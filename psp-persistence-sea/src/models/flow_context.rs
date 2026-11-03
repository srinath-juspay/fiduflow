//! SeaORM entity model for the flow_context table.
//!
//! This table stores cross-segment locals for each transaction.
//! Currently minimal, but designed for extension.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "flow_context")]
pub struct Model {
    /// Transaction ID (primary key, foreign key to upi_txn).
    #[sea_orm(primary_key, auto_increment = false)]
    pub txn_id: i64,

    // ---- Cross-segment locals ----
    // Add fields here as the context grows.
    // Example:
    // pub high_value_transaction: Option<bool>,
    // pub risk_score: Option<i32>,
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
