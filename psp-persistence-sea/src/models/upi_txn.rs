//! SeaORM entity model for the upi_txn table.
//!
//! This table stores the entity snapshot for each transaction.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "upi_txn")]
pub struct Model {
    /// Transaction ID (primary key).
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: i64,

    /// Entity state name (Initiated, Authenticated, Submitted, etc.).
    pub state: String,

    /// Payer ID (always present).
    pub payer: i64,

    /// Payee VPA (present from Authenticated onwards).
    pub payee: Option<i64>,

    /// Amount in paise (present from Authenticated onwards).
    pub amount: Option<i64>,

    /// MPIN (present from Authenticated onwards).
    pub mpin: Option<i64>,

    /// Debit reference (present on Success with debit).
    pub debit_ref: Option<i64>,

    /// Credit reference (present on Success).
    pub credit_ref: Option<i64>,

    /// Failure reason (present on Failed).
    pub failure_reason: Option<String>,

    /// When the record was created.
    pub created_at: DateTimeUtc,

    /// When the record was last updated.
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::events::Entity")]
    Events,
    #[sea_orm(has_one = "super::flow_context::Entity")]
    FlowContext,
}

impl Related<super::events::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl Related<super::flow_context::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FlowContext.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
