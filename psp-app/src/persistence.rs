//! Storage mapper for the pay flow: how the stored bookmark (which carries the
//! transaction) becomes rows in the application's tables, and how rows become the bookmark
//! again. In the DSL this is what a `#[persist]` derive with column annotations would
//! generate; it is hand-written here so the shape of the generated code is visible.
//!
//! Two tables, chosen by the application:
//! * `upi_txn`  — the entity, one row per transaction, queryable by support and recon.
//!                The `state` column is the entity state name; nullable columns are the
//!                per-state data.
//! * `pay_flow` — the flow position: which `PayerPay` variant, plus the locals that
//!                variant carries beyond the entity.

use crate::PayFlow;
use driver_framework::{InstanceId, Persist, Stored};
use psp_flow::*;
use store_framework::sqlite::Sqlite;
use rusqlite::{params, Transaction};

pub type StoredPay = Stored<PayerPay, Txn<Terminal>>;

// ---------------------------------------------------------------------------------------
// Row shapes.
// ---------------------------------------------------------------------------------------

struct EntityRow {
    txn_id: i64,
    state: &'static str,
    payer: i64,
    payee: Option<i64>,
    amount: Option<i64>,
    mpin: Option<i64>,
    debit_ref: Option<i64>,
    credit_ref: Option<i64>,
    reason: Option<&'static str>,
}

struct FlowRow {
    at: &'static str,
    intent_payee: Option<i64>,
    intent_amount: Option<i64>,
    reason: Option<&'static str>,
    debit_ref: Option<i64>,
    credit_ref: Option<i64>,
}

impl EntityRow {
    fn initiated(id: TxnId, payer: PayerId) -> Self {
        EntityRow { txn_id: id.0 as i64, state: "Initiated", payer: payer.0 as i64, payee: None, amount: None, mpin: None, debit_ref: None, credit_ref: None, reason: None }
    }
    fn with_req(id: TxnId, state: &'static str, req: &PayRequest) -> Self {
        EntityRow {
            txn_id: id.0 as i64, state, payer: req.payer.0 as i64,
            payee: Some(req.payee.0 as i64), amount: Some(req.amount.0 as i64), mpin: Some(req.mpin.0 as i64),
            debit_ref: None, credit_ref: None, reason: None,
        }
    }
    fn with_state(mut self, state: &'static str) -> Self { self.state = state; self }
}

impl FlowRow {
    fn at(at: &'static str) -> Self {
        FlowRow { at, intent_payee: None, intent_amount: None, reason: None, debit_ref: None, credit_ref: None }
    }
}

fn reason_str(r: Reason) -> &'static str {
    match r {
        Reason::InvalidAmount => "InvalidAmount",
        Reason::UnknownPayee => "UnknownPayee",
        Reason::PayeeIsPayer => "PayeeIsPayer",
        Reason::InsufficientFunds => "InsufficientFunds",
        Reason::BankDeclined => "BankDeclined",
        Reason::RiskRejected => "RiskRejected",
    }
}

fn reason_parse(s: &str) -> rusqlite::Result<Reason> {
    Ok(match s {
        "InvalidAmount" => Reason::InvalidAmount,
        "UnknownPayee" => Reason::UnknownPayee,
        "PayeeIsPayer" => Reason::PayeeIsPayer,
        "InsufficientFunds" => Reason::InsufficientFunds,
        "BankDeclined" => Reason::BankDeclined,
        "RiskRejected" => Reason::RiskRejected,
        other => return Err(bad(&format!("reason {other}"))),
    })
}

fn bad(what: &str) -> rusqlite::Error {
    rusqlite::Error::InvalidColumnName(format!("corrupt row: {what}"))
}

fn need<T>(v: Option<T>, what: &str) -> rusqlite::Result<T> {
    v.ok_or_else(|| bad(what))
}

// ---------------------------------------------------------------------------------------
// Object -> rows.
// ---------------------------------------------------------------------------------------

fn rows_waiting(s: &PayerPay) -> (EntityRow, FlowRow) {
    use PayerPay::*;
    match s {
        AtGetPaymentIntent(psp_flow::AtGetPaymentIntent { txn }) => (EntityRow::initiated(txn.id, txn.data.payer), FlowRow::at("AtGetPaymentIntent")),
        AtShowError(psp_flow::AtShowError { txn, reason }) => (
            EntityRow::initiated(txn.id, txn.data.payer),
            FlowRow { reason: Some(reason_str(*reason)), ..FlowRow::at("AtShowError") },
        ),
        AtCollectMpin(psp_flow::AtCollectMpin { txn, intent }) => (
            EntityRow::initiated(txn.id, txn.data.payer),
            FlowRow { intent_payee: Some(intent.payee.0 as i64), intent_amount: Some(intent.amount.0 as i64), ..FlowRow::at("AtCollectMpin") },
        ),
        AtReqPay(psp_flow::AtReqPay { txn, req }) => (EntityRow::with_req(txn.id, "Authenticated", req), FlowRow::at("AtReqPay")),
        AtNotifyRejected(psp_flow::AtNotifyRejected { txn, req, reason }) => (
            EntityRow::with_req(txn.id, "Authenticated", req),
            FlowRow { reason: Some(reason_str(*reason)), ..FlowRow::at("AtNotifyRejected") },
        ),
        AtAwaitCallback(psp_flow::AtAwaitCallback { txn }) => (EntityRow::with_req(txn.id, "Submitted", &txn.data.req), FlowRow::at("AtAwaitCallback")),
        AtNotifySuccess(psp_flow::AtNotifySuccess { txn, debit_ref, credit_ref }) => (
            EntityRow::with_req(txn.id, "Submitted", &txn.data.req),
            FlowRow { debit_ref: Some(debit_ref.0 as i64), credit_ref: Some(credit_ref.0 as i64), ..FlowRow::at("AtNotifySuccess") },
        ),
        AtNotifyFailed(psp_flow::AtNotifyFailed { txn, reason }) => (
            EntityRow::with_req(txn.id, "Submitted", &txn.data.req),
            FlowRow { reason: Some(reason_str(*reason)), ..FlowRow::at("AtNotifyFailed") },
        ),
        AtNotifyProcessing(psp_flow::AtNotifyProcessing { txn }) => (EntityRow::with_req(txn.id, "Submitted", &txn.data.req), FlowRow::at("AtNotifyProcessing")),
        AtAwaitDeemed(psp_flow::AtAwaitDeemed { txn }) => (EntityRow::with_req(txn.id, "Deemed", &txn.data.req), FlowRow::at("AtAwaitDeemed")),
        AtNotifyConfirmed(psp_flow::AtNotifyConfirmed { txn, credit_ref }) => (
            EntityRow::with_req(txn.id, "Deemed", &txn.data.req),
            FlowRow { credit_ref: Some(credit_ref.0 as i64), ..FlowRow::at("AtNotifyConfirmed") },
        ),
        AtNotifyDeemedReversed(psp_flow::AtNotifyDeemedReversed { txn }) => (EntityRow::with_req(txn.id, "Deemed", &txn.data.req), FlowRow::at("AtNotifyDeemedReversed")),
        AtNotifyReversalStart(psp_flow::AtNotifyReversalStart { txn }) => (EntityRow::with_req(txn.id, "ReversalPending", &txn.data.req), FlowRow::at("AtNotifyReversalStart")),
        AtAwaitForcedReversal(psp_flow::AtAwaitForcedReversal { txn }) => (EntityRow::with_req(txn.id, "ReversalPending", &txn.data.req), FlowRow::at("AtAwaitForcedReversal")),
        AtNotifyReversed(psp_flow::AtNotifyReversed { txn }) => (EntityRow::with_req(txn.id, "ReversalPending", &txn.data.req), FlowRow::at("AtNotifyReversed")),
    }
}

fn rows_finished(t: &Txn<Terminal>) -> (EntityRow, FlowRow) {
    let e = match &t.data {
        Terminal::Success(s) => EntityRow {
            debit_ref: s.debit_ref.map(|r| r.0 as i64),
            credit_ref: Some(s.credit_ref.0 as i64),
            ..EntityRow::with_req(t.id, "Success", &s.req)
        },
        Terminal::Failed(f) => EntityRow { reason: Some(reason_str(f.reason)), ..EntityRow::with_req(t.id, "Failed", &f.req) },
        Terminal::Reversed(r) => EntityRow::with_req(t.id, "Reversed", &r.req),
        Terminal::Abandoned(a) => EntityRow::initiated(t.id, a.payer).with_state("Abandoned"),
    };
    (e, FlowRow::at("Finished"))
}

// ---------------------------------------------------------------------------------------
// Rows -> object.
// ---------------------------------------------------------------------------------------

struct EntityDb { txn_id: i64, state: String, payer: i64, payee: Option<i64>, amount: Option<i64>, mpin: Option<i64>, debit_ref: Option<i64>, credit_ref: Option<i64>, reason: Option<String> }
struct FlowDb { at: String, intent_payee: Option<i64>, intent_amount: Option<i64>, reason: Option<String>, debit_ref: Option<i64>, credit_ref: Option<i64> }

impl EntityDb {
    fn id(&self) -> TxnId { TxnId(self.txn_id as u64) }
    fn payer(&self) -> PayerId { PayerId(self.payer as u64) }
    fn req(&self) -> rusqlite::Result<PayRequest> {
        Ok(PayRequest {
            payer: self.payer(),
            payee: Vpa(need(self.payee, "payee")? as u64),
            amount: Amount(need(self.amount, "amount")? as u64),
            mpin: Mpin(need(self.mpin, "mpin")? as u32),
        })
    }
    fn initiated(&self) -> Txn<Initiated> { Txn { id: self.id(), data: Initiated { payer: self.payer() } } }
    fn authenticated(&self) -> rusqlite::Result<Txn<Authenticated>> {
        let r = self.req()?;
        Ok(Txn { id: self.id(), data: Authenticated { payer: r.payer, intent: ValidIntent { payee: r.payee, amount: r.amount }, mpin: r.mpin } })
    }
    fn submitted(&self) -> rusqlite::Result<Txn<Submitted>> { Ok(Txn { id: self.id(), data: Submitted { req: self.req()? } }) }
    fn deemed(&self) -> rusqlite::Result<Txn<Deemed>> { Ok(Txn { id: self.id(), data: Deemed { req: self.req()? } }) }
    fn reversal_pending(&self) -> rusqlite::Result<Txn<ReversalPending>> { Ok(Txn { id: self.id(), data: ReversalPending { req: self.req()? } }) }
    fn reason(&self) -> rusqlite::Result<Reason> { reason_parse(need(self.reason.as_deref(), "reason")?) }
}

impl FlowDb {
    fn reason(&self) -> rusqlite::Result<Reason> { reason_parse(need(self.reason.as_deref(), "flow reason")?) }
    fn credit_ref(&self) -> rusqlite::Result<Ref> { Ok(Ref(need(self.credit_ref, "credit_ref")? as u64)) }
    fn debit_ref(&self) -> rusqlite::Result<Ref> { Ok(Ref(need(self.debit_ref, "debit_ref")? as u64)) }
}

fn rebuild(e: &EntityDb, f: &FlowDb) -> rusqlite::Result<StoredPay> {
    use PayerPay::*;
    let s = match f.at.as_str() {
        "AtGetPaymentIntent" => AtGetPaymentIntent(psp_flow::AtGetPaymentIntent { txn: e.initiated() }),
        "AtShowError" => AtShowError(psp_flow::AtShowError { txn: e.initiated(), reason: f.reason()? }),
        "AtCollectMpin" => AtCollectMpin(psp_flow::AtCollectMpin {
            txn: e.initiated(),
            intent: ValidIntent { payee: Vpa(need(f.intent_payee, "intent_payee")? as u64), amount: Amount(need(f.intent_amount, "intent_amount")? as u64) },
        }),
        "AtReqPay" => { let txn = e.authenticated()?; let req = txn.build_req_pay(); AtReqPay(psp_flow::AtReqPay { txn, req }) }
        "AtNotifyRejected" => { let txn = e.authenticated()?; let req = txn.build_req_pay(); AtNotifyRejected(psp_flow::AtNotifyRejected { txn, req, reason: f.reason()? }) }
        "AtAwaitCallback" => AtAwaitCallback(psp_flow::AtAwaitCallback { txn: e.submitted()? }),
        "AtNotifySuccess" => AtNotifySuccess(psp_flow::AtNotifySuccess { txn: e.submitted()?, debit_ref: f.debit_ref()?, credit_ref: f.credit_ref()? }),
        "AtNotifyFailed" => AtNotifyFailed(psp_flow::AtNotifyFailed { txn: e.submitted()?, reason: f.reason()? }),
        "AtNotifyProcessing" => AtNotifyProcessing(psp_flow::AtNotifyProcessing { txn: e.submitted()? }),
        "AtAwaitDeemed" => AtAwaitDeemed(psp_flow::AtAwaitDeemed { txn: e.deemed()? }),
        "AtNotifyConfirmed" => AtNotifyConfirmed(psp_flow::AtNotifyConfirmed { txn: e.deemed()?, credit_ref: f.credit_ref()? }),
        "AtNotifyDeemedReversed" => AtNotifyDeemedReversed(psp_flow::AtNotifyDeemedReversed { txn: e.deemed()? }),
        "AtNotifyReversalStart" => AtNotifyReversalStart(psp_flow::AtNotifyReversalStart { txn: e.reversal_pending()? }),
        "AtAwaitForcedReversal" => AtAwaitForcedReversal(psp_flow::AtAwaitForcedReversal { txn: e.reversal_pending()? }),
        "AtNotifyReversed" => AtNotifyReversed(psp_flow::AtNotifyReversed { txn: e.reversal_pending()? }),
        "Finished" => {
            let data = match e.state.as_str() {
                "Success" => Terminal::Success(Success { req: e.req()?, debit_ref: e.debit_ref.map(|r| Ref(r as u64)), credit_ref: Ref(need(e.credit_ref, "credit_ref")? as u64) }),
                "Failed" => Terminal::Failed(Failed { req: e.req()?, reason: e.reason()? }),
                "Reversed" => Terminal::Reversed(Reversed { req: e.req()? }),
                "Abandoned" => Terminal::Abandoned(Abandoned { payer: e.payer() }),
                other => return Err(bad(&format!("terminal state {other}"))),
            };
            return Ok(Stored::Finished(Txn { id: e.id(), data }));
        }
        other => return Err(bad(&format!("flow position {other}"))),
    };
    Ok(Stored::Waiting(s))
}

// ---------------------------------------------------------------------------------------
// The Persist impl: the user's tables.
// ---------------------------------------------------------------------------------------

impl Persist<Sqlite> for PayFlow {
    fn schema() -> &'static str {
        "
        CREATE TABLE IF NOT EXISTS upi_txn (
            instance    INTEGER PRIMARY KEY,
            txn_id      INTEGER NOT NULL,
            state       TEXT    NOT NULL,
            payer       INTEGER NOT NULL,
            payee       INTEGER,
            amount      INTEGER,
            mpin        INTEGER,
            debit_ref   INTEGER,
            credit_ref  INTEGER,
            reason      TEXT
        );
        CREATE TABLE IF NOT EXISTS pay_flow (
            instance      INTEGER PRIMARY KEY,
            at            TEXT NOT NULL,
            intent_payee  INTEGER,
            intent_amount INTEGER,
            reason        TEXT,
            debit_ref     INTEGER,
            credit_ref    INTEGER
        );
        "
    }

    fn save(obj: &StoredPay, tx: &mut Transaction<'_>, instance: InstanceId) -> rusqlite::Result<()> {
        let (e, f) = match obj {
            Stored::Waiting(s) => rows_waiting(s),
            Stored::Finished(t) => rows_finished(t),
        };
        tx.execute(
            "INSERT INTO upi_txn (instance, txn_id, state, payer, payee, amount, mpin, debit_ref, credit_ref, reason)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(instance) DO UPDATE SET txn_id = excluded.txn_id, state = excluded.state, payer = excluded.payer,
               payee = excluded.payee, amount = excluded.amount, mpin = excluded.mpin,
               debit_ref = excluded.debit_ref, credit_ref = excluded.credit_ref, reason = excluded.reason",
            params![instance, e.txn_id, e.state, e.payer, e.payee, e.amount, e.mpin, e.debit_ref, e.credit_ref, e.reason],
        )?;
        tx.execute(
            "INSERT INTO pay_flow (instance, at, intent_payee, intent_amount, reason, debit_ref, credit_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(instance) DO UPDATE SET at = excluded.at, intent_payee = excluded.intent_payee,
               intent_amount = excluded.intent_amount, reason = excluded.reason,
               debit_ref = excluded.debit_ref, credit_ref = excluded.credit_ref",
            params![instance, f.at, f.intent_payee, f.intent_amount, f.reason, f.debit_ref, f.credit_ref],
        )?;
        Ok(())
    }

    fn load(tx: &mut Transaction<'_>, instance: InstanceId) -> rusqlite::Result<StoredPay> {
        let e = tx.query_row(
            "SELECT txn_id, state, payer, payee, amount, mpin, debit_ref, credit_ref, reason FROM upi_txn WHERE instance = ?1",
            params![instance],
            |r| Ok(EntityDb { txn_id: r.get(0)?, state: r.get(1)?, payer: r.get(2)?, payee: r.get(3)?, amount: r.get(4)?, mpin: r.get(5)?, debit_ref: r.get(6)?, credit_ref: r.get(7)?, reason: r.get(8)? }),
        )?;
        let f = tx.query_row(
            "SELECT at, intent_payee, intent_amount, reason, debit_ref, credit_ref FROM pay_flow WHERE instance = ?1",
            params![instance],
            |r| Ok(FlowDb { at: r.get(0)?, intent_payee: r.get(1)?, intent_amount: r.get(2)?, reason: r.get(3)?, debit_ref: r.get(4)?, credit_ref: r.get(5)? }),
        )?;
        rebuild(&e, &f)
    }
}

/// The entity's state column, as support would read it.
pub fn entity_state(conn: &rusqlite::Connection, instance: InstanceId) -> rusqlite::Result<String> {
    conn.query_row("SELECT state FROM upi_txn WHERE instance = ?1", params![instance], |r| r.get(0))
}
