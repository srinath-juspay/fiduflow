//! SeaORM entity model for the events table.
//!
//! This table stores the event lifecycle for each transaction.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    /// Auto-generated primary key.
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Transaction ID (foreign key to upi_txn).
    pub txn_id: i64,

    /// Sequence number within the transaction (0-indexed).
    pub seq: i32,

    /// Event type (GetPaymentIntent, ShowError, etc.).
    pub event_type: String,

    /// Event status (Initialized, Awaiting, Received, Processed).
    pub status: String,

    // ---- Request fields ----
    /// Payer ID in the request.
    pub req_payer: Option<i64>,

    /// Payee VPA in the request (for intent).
    pub req_intent_payee: Option<i64>,

    /// Amount in the request (for intent).
    pub req_intent_amount: Option<i64>,

    /// Reason in the request.
    pub req_reason: Option<String>,

    /// Status in the request (for Notify).
    pub req_status: Option<String>,

    /// PayRequest payer in the request.
    pub req_pay_payer: Option<i64>,

    /// PayRequest payee in the request.
    pub req_pay_payee: Option<i64>,

    /// PayRequest amount in the request.
    pub req_pay_amount: Option<i64>,

    /// PayRequest mpin in the request.
    pub req_pay_mpin: Option<i64>,

    /// Timeout in seconds (for AwaitCallback).
    pub req_timeout_secs: Option<i64>,

    /// Deadline in days (for AwaitDeemedResolution, AwaitForcedReversal).
    pub req_deadline_days: Option<i64>,

    // ---- Response fields ----
    /// Raw intent payee from response.
    pub resp_raw_payee: Option<i64>,

    /// Raw intent amount from response.
    pub resp_raw_amount: Option<i64>,

    /// MPIN from response.
    pub resp_mpin: Option<i64>,

    /// ReqPay result type (Ack, Rejected).
    pub resp_req_pay_result: Option<String>,

    /// Rejection reason from ReqPay response.
    pub resp_reject_reason: Option<String>,

    /// Callback type (Success, Failed, Deemed, Timeout).
    pub resp_callback_type: Option<String>,

    /// Debit reference from callback.
    pub resp_debit_ref: Option<i64>,

    /// Credit reference from callback or resolution.
    pub resp_credit_ref: Option<i64>,

    /// Callback failure reason.
    pub resp_callback_reason: Option<String>,

    /// Resolution type (DeemedSuccess, DeemedFailed, DeadlineExpired).
    pub resp_resolution_type: Option<String>,

    // ---- Timestamps ----
    /// When the event was created.
    pub created_at: DateTimeUtc,

    /// When the event was sent.
    pub sent_at: Option<DateTimeUtc>,

    /// When the response was received.
    pub received_at: Option<DateTimeUtc>,

    /// When the event was processed.
    pub processed_at: Option<DateTimeUtc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::upi_txn::Entity",
        from = "Column::TxnId",
        to = "super::upi_txn::Column::Id"
    )]
    UpiTxn,
}

impl Related<super::upi_txn::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::UpiTxn.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
