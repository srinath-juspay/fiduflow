//! SeaORM entity model for the events table.
//!
//! This table stores the event lifecycle for each transaction.
//! Request and response data are stored as JSON blobs.

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

    /// Request data as JSON.
    pub request: String,

    /// Response data as JSON (nullable).
    pub response: Option<String>,

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
