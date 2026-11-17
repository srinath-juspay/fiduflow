//! The application: binds the flow machine in `psp_flow` to the persistence layer.
//!
//! This crate provides:
//! - `PayFlow`: Flow + Persist trait implementation for PayerPay
//! - `PayFlowStore`: Adapter to use SeaFlowStore with the generic Driver
//! - Re-exports of persistence and flow types
//!
//! The generic `Driver` from `driver_framework` is used for flow execution.

#![forbid(unsafe_code)]

mod persist_impl;
mod store_adapter;
#[cfg(test)]
mod tests;

pub use store_adapter::PayFlowStore;

// Re-export from driver-framework
pub use driver_framework::{
    DeliverResult, Driver, DriverError, Flow, FlowStore, Next, Persist, ReconstructError, Refused,
};

// Re-export the persistence layer types
pub use psp_persistence::{
    decompose, reconstruct, EntitySnapshot, FlowContext, MessageRecord, MessageStatus, Segment,
};

// Re-export the SeaORM store
pub use psp_persistence_sea::SeaFlowStore;

// Re-export flow types
pub use psp_flow::values::{PayerId, TxnId};
pub use psp_flow::{AnyEvent, AnyResponse, PayerPay, Terminal, Txn};

use psp_flow::Mismatch;

/// The pay flow marker type.
pub struct PayFlow;

impl Flow for PayFlow {
    const NAME: &'static str = "payer_pay";
    type Id = TxnId;
    type State = PayerPay;
    type Done = Txn<Terminal>;
    type Event = AnyEvent;
    type Response = AnyResponse;

    fn initial(id: TxnId) -> PayerPay {
        psp_flow::machine::initial(id)
    }

    fn event(s: &PayerPay) -> AnyEvent {
        psp_flow::event(s)
    }

    fn step(s: PayerPay, r: AnyResponse) -> Result<Next<PayerPay, Txn<Terminal>>, Refused> {
        psp_flow::deliver(s, r).map_err(|Mismatch::ResponseMismatch| Refused::Mismatch)
    }
}
