//! The application: binds the generated-shaped machine in `psp_flow` to the framework and
//! models the tables it is saved to. This is the only crate that knows about payments.

#![forbid(unsafe_code)]

pub mod pay_flow;

pub use pay_flow::{entity_state, PayFlow, StoredPay};
pub use psp_driver_framework::{Driver, DriverError, Outcome};
pub use psp_store_framework::sqlite::Sqlite;

/// The pay flow on SQLite.
pub type PayDriver = Driver<PayFlow, Sqlite>;

pub fn open(path: &str) -> rusqlite::Result<PayDriver> {
    Driver::new(Sqlite::open(path)?)
}

pub fn open_in_memory() -> rusqlite::Result<PayDriver> {
    Driver::new(Sqlite::open_in_memory()?)
}
