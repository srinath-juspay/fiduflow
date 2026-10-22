//! The application: binds the generated-shaped machine in `psp_flow` to the framework and
//! models the tables it is saved to. This is the only crate that knows about payments.

#![forbid(unsafe_code)]

pub mod persistence;

pub use driver_framework::{Driver, DriverError, Flow, Next, Outcome};
pub use persistence::{entity_state, StoredPay};
pub use store_framework::sqlite::Sqlite;

use psp_flow::{AnyEvent, AnyResponse, PayerPay, Terminal, Txn};

/// The pay flow, as the framework sees it. What `#[flow]` would generate.
pub struct PayFlow;

impl Flow for PayFlow {
    const NAME: &'static str = "payer_pay";
    type State = PayerPay;
    type Done = Txn<Terminal>;
    type Event = AnyEvent;
    type Response = AnyResponse;

    fn event(s: &PayerPay) -> AnyEvent { psp_flow::event(s) }

    fn step(s: PayerPay, r: AnyResponse) -> Option<Next<PayerPay, Txn<Terminal>>> {
        match psp_flow::deliver(s, r).ok()? {
            psp_flow::Next::Wait(s1) => Some(Next::Wait(s1)),
            psp_flow::Next::Done(d) => Some(Next::Done(d)),
        }
    }
}

/// The pay flow on SQLite.
pub type PayDriver = Driver<PayFlow, Sqlite>;

pub fn open(path: &str) -> rusqlite::Result<PayDriver> {
    Driver::new(Sqlite::open(path)?)
}

pub fn open_in_memory() -> rusqlite::Result<PayDriver> {
    Driver::new(Sqlite::open_in_memory()?)
}
