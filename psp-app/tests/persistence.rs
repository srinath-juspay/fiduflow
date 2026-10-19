//! The persistence contract, tested through the application's tables.

use psp_app::{entity_state, open, open_in_memory, DriverError, Outcome, PayDriver, PayFlow, Sqlite, StoredPay};
use psp_flow::*;
use driver_framework::{Backend, Persist, Stored};

fn intent() -> AnyResponse { AnyResponse::Intent(RawIntent { payee: Vpa(42), amount: Amount(50_000) }) }

fn state(d: &PayDriver, id: i64) -> String { entity_state(d.backend().connection(), id).unwrap() }

fn to_await_callback(d: &mut PayDriver, txn: u64) -> i64 {
    let (id, _) = d.start(start(TxnId(txn), PayerId(7))).unwrap();
    d.deliver(id, intent()).unwrap();
    d.deliver(id, AnyResponse::Mpin(Mpin(1234))).unwrap();
    match d.deliver(id, AnyResponse::ReqPay(ReqPayResponse::Ack)).unwrap() {
        Outcome::Wait(AnyEvent::AwaitCallback(_)) => id,
        other => panic!("expected AwaitCallback, got {other:?}"),
    }
}

#[test]
fn entity_state_column_tracks_the_lifecycle() {
    let mut d = open_in_memory().unwrap();
    let (id, ev) = d.start(start(TxnId(1), PayerId(7))).unwrap();
    assert!(matches!(ev, AnyEvent::GetPaymentIntent(_)));
    assert_eq!(state(&d, id), "Initiated");
    d.deliver(id, intent()).unwrap();
    assert_eq!(state(&d, id), "Initiated");
    d.deliver(id, AnyResponse::Mpin(Mpin(1234))).unwrap();
    assert_eq!(state(&d, id), "Authenticated");
    d.deliver(id, AnyResponse::ReqPay(ReqPayResponse::Ack)).unwrap();
    assert_eq!(state(&d, id), "Submitted");
    d.deliver(id, AnyResponse::Callback(Callback::Success { debit_ref: Ref(1), credit_ref: Ref(2) })).unwrap();
    assert_eq!(state(&d, id), "Submitted");
    match d.deliver(id, AnyResponse::Unit).unwrap() {
        Outcome::Done(t) => assert!(matches!(t.data, Terminal::Success(Success { credit_ref: Ref(2), .. }))),
        Outcome::Wait(e) => panic!("still waiting on {e:?}"),
    }
    assert_eq!(state(&d, id), "Success");
    assert_eq!(d.journal(id).unwrap().len(), 5);
}

#[test]
fn crash_recovery_resumes_from_the_tables() {
    let path = std::env::temp_dir().join(format!("psp-test-{}.sqlite", std::process::id()));
    let p = path.to_str().unwrap();
    let id = {
        let mut d = open(p).unwrap();
        to_await_callback(&mut d, 2)
    }; // dropped: nothing in memory survives
    let mut d = open(p).unwrap();
    let pending = d.pending().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].0, id);
    assert!(matches!(pending[0].1, AnyEvent::AwaitCallback(AwaitCallback { timeout_secs: 30 })));

    d.deliver(id, AnyResponse::Callback(Callback::Timeout)).unwrap();
    d.deliver(id, AnyResponse::Unit).unwrap();
    assert_eq!(state(&d, id), "Deemed");
    d.deliver(id, AnyResponse::Resolution(Resolution::DeadlineExpired)).unwrap();
    assert_eq!(state(&d, id), "ReversalPending");
    d.deliver(id, AnyResponse::Unit).unwrap();
    d.deliver(id, AnyResponse::Unit).unwrap();
    match d.deliver(id, AnyResponse::Unit).unwrap() {
        Outcome::Done(t) => assert!(matches!(t.data, Terminal::Reversed(_))),
        Outcome::Wait(e) => panic!("still waiting on {e:?}"),
    }
    assert_eq!(state(&d, id), "Reversed");
    assert!(d.pending().unwrap().is_empty());
    let _ = std::fs::remove_file(&path);
}

#[test]
fn violation_rolls_back_and_changes_nothing() {
    let mut d = open_in_memory().unwrap();
    let id = to_await_callback(&mut d, 3);
    let journal_before = d.journal(id).unwrap();
    let err = d.deliver(id, AnyResponse::Mpin(Mpin(9))).unwrap_err();
    assert!(matches!(err, DriverError::Violation(Mismatch::ResponseMismatch)));
    assert_eq!(state(&d, id), "Submitted");
    assert_eq!(d.journal(id).unwrap(), journal_before);
    assert!(matches!(d.deliver(id, AnyResponse::Callback(Callback::Deemed)).unwrap(), Outcome::Wait(AnyEvent::Notify(Notify { status: Status::Processing }))));
}

#[test]
fn finished_instances_refuse_more_responses() {
    let mut d = open_in_memory().unwrap();
    let (id, _) = d.start(start(TxnId(4), PayerId(7))).unwrap();
    d.deliver(id, AnyResponse::Intent(RawIntent { payee: Vpa(7), amount: Amount(1) })).unwrap(); // payee == payer
    assert!(matches!(d.deliver(id, AnyResponse::Unit).unwrap(), Outcome::Done(Txn { data: Terminal::Abandoned(_), .. })));
    assert!(matches!(d.deliver(id, AnyResponse::Unit).unwrap_err(), DriverError::AlreadyFinished(_)));
    assert!(matches!(d.deliver(999, AnyResponse::Unit).unwrap_err(), DriverError::UnknownInstance(999)));
}

/// Every waiting point and every terminal state survives a save/load round trip through
/// the application's tables. This is the test a `#[persist]` derive would generate.
#[test]
fn every_state_round_trips_through_the_tables() {
    use PayerPay::*;
    let id = TxnId(5);
    let init = Txn::new(id, PayerId(7));
    let intent = ValidIntent { payee: Vpa(42), amount: Amount(50_000) };
    let auth = Txn::new(id, PayerId(7)).authenticate(intent, Mpin(1234));
    let req = auth.build_req_pay();
    let sub = auth.submit(req);
    let deemed = sub.deem();
    let rp = deemed.escalate_reversal();
    let states: Vec<StoredPay> = vec![
        Stored::Waiting(AtGetPaymentIntent(psp_flow::AtGetPaymentIntent { txn: init })),
        Stored::Waiting(AtShowError(psp_flow::AtShowError { txn: init, reason: Reason::UnknownPayee })),
        Stored::Waiting(AtCollectMpin(psp_flow::AtCollectMpin { txn: init, intent })),
        Stored::Waiting(AtReqPay(psp_flow::AtReqPay { txn: auth, req })),
        Stored::Waiting(AtNotifyRejected(psp_flow::AtNotifyRejected { txn: auth, req, reason: Reason::RiskRejected })),
        Stored::Waiting(AtAwaitCallback(psp_flow::AtAwaitCallback { txn: sub })),
        Stored::Waiting(AtNotifySuccess(psp_flow::AtNotifySuccess { txn: sub, debit_ref: Ref(1), credit_ref: Ref(2) })),
        Stored::Waiting(AtNotifyFailed(psp_flow::AtNotifyFailed { txn: sub, reason: Reason::BankDeclined })),
        Stored::Waiting(AtNotifyProcessing(psp_flow::AtNotifyProcessing { txn: sub })),
        Stored::Waiting(AtAwaitDeemed(psp_flow::AtAwaitDeemed { txn: deemed })),
        Stored::Waiting(AtNotifyConfirmed(psp_flow::AtNotifyConfirmed { txn: deemed, credit_ref: Ref(3) })),
        Stored::Waiting(AtNotifyDeemedReversed(psp_flow::AtNotifyDeemedReversed { txn: deemed })),
        Stored::Waiting(AtNotifyReversalStart(psp_flow::AtNotifyReversalStart { txn: rp })),
        Stored::Waiting(AtAwaitForcedReversal(psp_flow::AtAwaitForcedReversal { txn: rp })),
        Stored::Waiting(AtNotifyReversed(psp_flow::AtNotifyReversed { txn: rp })),
        Stored::Finished(sub.succeed(Ref(1), Ref(2)).terminal()),
        Stored::Finished(deemed.confirm(Ref(9)).terminal()),
        Stored::Finished(sub.fail(Reason::InsufficientFunds).terminal()),
        Stored::Finished(rp.reversed().terminal()),
        Stored::Finished(init.abandon().terminal()),
    ];
    let mut backend = Sqlite::open_in_memory().unwrap();
    backend.execute_ddl(<PayFlow as Persist<Sqlite>>::schema()).unwrap();
    for (i, s) in states.iter().enumerate() {
        let mut tx = backend.begin().unwrap();
        <PayFlow as Persist<Sqlite>>::save(s, &mut tx, i as i64).unwrap();
        let back = <PayFlow as Persist<Sqlite>>::load(&mut tx, i as i64).unwrap();
        Sqlite::commit(tx).unwrap();
        assert_eq!(&back, s, "round trip of state #{i}");
    }
}
