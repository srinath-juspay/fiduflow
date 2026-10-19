//! Storage. Knows about databases and nothing about flows or domains.
//!
//! `Backend` is implemented once per database. It owns transactions and the framework's
//! own tables (the instance registry and the journal). The driver only ever talks to this
//! trait; how an application's own tables look is the application's business, expressed
//! through `driver_framework::Persist`.
//!
//! Contract every backend must honour: dropping a `Tx` without `commit` rolls it back.

#![forbid(unsafe_code)]

#[cfg(feature = "sqlite")]
pub mod sqlite;

use core::fmt::Debug;

pub type InstanceId = i64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Waiting,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstanceInfo {
    pub seq: i64,
    pub status: Status,
}

/// One delivered response: the bookmark it was delivered to, what that bookmark was
/// waiting on, and the response. Text for now; the DSL would use a real serialisation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalEntry {
    pub at: String,
    pub event: String,
    pub response: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalRow {
    pub seq: i64,
    pub entry: JournalEntry,
}

pub trait Backend {
    /// A transaction borrowed from the backend. Rolls back if dropped uncommitted.
    type Tx<'a>
    where
        Self: 'a;
    type Error: Debug;

    fn begin(&mut self) -> Result<Self::Tx<'_>, Self::Error>;
    fn commit(tx: Self::Tx<'_>) -> Result<(), Self::Error>;

    /// Run application DDL (idempotent `CREATE TABLE IF NOT EXISTS ...`). Called once at open.
    fn execute_ddl(&mut self, ddl: &str) -> Result<(), Self::Error>;

    // The framework's own tables.
    fn insert_instance(tx: &mut Self::Tx<'_>, flow: &str) -> Result<InstanceId, Self::Error>;
    fn instance(tx: &mut Self::Tx<'_>, id: InstanceId) -> Result<Option<InstanceInfo>, Self::Error>;
    fn advance(tx: &mut Self::Tx<'_>, id: InstanceId, seq: i64, status: Status) -> Result<(), Self::Error>;
    fn append_journal(tx: &mut Self::Tx<'_>, id: InstanceId, seq: i64, entry: &JournalEntry) -> Result<(), Self::Error>;
    fn waiting_instances(tx: &mut Self::Tx<'_>, flow: &str) -> Result<Vec<InstanceId>, Self::Error>;
    fn journal(tx: &mut Self::Tx<'_>, id: InstanceId) -> Result<Vec<JournalRow>, Self::Error>;
}
