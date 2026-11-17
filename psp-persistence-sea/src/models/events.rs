//! SeaORM entity model for the events table.
//!
//! This table stores all flow events with classification metadata.
//! Event body is stored as JSON for flexibility.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;

pub type DateTimeUtc = DateTime<Utc>;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,

    /// Foreign key to upi_txn.
    pub txn_id: i64,

    /// Sequence number within the transaction.
    pub seq: i32,

    // === Classification ===
    /// Source actor: 'User' | 'ThisSystem' | 'ExternalSystem'
    pub source: String,
    /// Target actor: 'User' | 'ThisSystem' | 'ExternalSystem'
    pub target: String,
    /// Method: 'Request' | 'Ack' | 'AsyncResponse' | 'Notify'
    pub method: String,

    // === Correlation ===
    /// For Ack/AsyncResponse: seq of the Request this responds to.
    pub in_response_to: Option<i32>,
    /// For Request: whether AsyncResponse is expected after Ack.
    pub expects_async: bool,

    // === Payload ===
    /// Event type name.
    pub event_type: String,
    /// Event body as JSON string.
    pub event_body: String,

    // === Status ===
    /// Event status: 'Pending' | 'Completed'
    pub status: String,

    // === Timestamps ===
    pub created_at: DateTimeUtc,
    pub completed_at: Option<DateTimeUtc>,
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
