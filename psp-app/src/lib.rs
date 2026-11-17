//! The application: binds the flow machine in `psp_flow` to the persistence layer.
//!
//! This crate provides the integration between the flow logic and the SeaORM-based
//! persistence layer.

#![forbid(unsafe_code)]

// Re-export the persistence layer
// Note: Per-event request/response types (GetPaymentIntentRequest, etc.) are available
// via psp_persistence or psp_flow::persist when needed for storage operations.
pub use psp_persistence::{
    decompose, reconstruct, EntitySnapshot, MessageRecord, MessageStatus, EventType, FlowContext,
    FlowStore, ReconstructError, Segment,
};

// Re-export the SeaORM store
pub use psp_persistence_sea::SeaFlowStore;

// Re-export flow types
pub use psp_flow::{AnyEvent, AnyResponse, PayerPay, Terminal, Txn};
pub use psp_flow::values::TxnId;

// Re-export driver framework types
pub use driver_framework::{Flow, Next, Refused};

use psp_flow::Mismatch;

/// The pay flow marker type.
pub struct PayFlow;

impl Flow for PayFlow {
    const NAME: &'static str = "payer_pay";
    type State = PayerPay;
    type Done = Txn<Terminal>;
    type Event = AnyEvent;
    type Response = AnyResponse;

    fn event(s: &PayerPay) -> AnyEvent {
        psp_flow::event(s)
    }

    fn step(s: PayerPay, r: AnyResponse) -> Result<Next<PayerPay, Txn<Terminal>>, Refused> {
        psp_flow::deliver(s, r).map_err(|Mismatch::ResponseMismatch| Refused::Mismatch)
    }
}
