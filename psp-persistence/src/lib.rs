//! Pure persistence bridge layer for the PSP flow.
//!
//! This crate provides:
//! - Typed entity snapshots per state
//! - Cross-segment context (FlowContext from psp_flow, ContextData from driver_framework)
//! - Event data structures (re-exported from psp_flow and driver_framework)
//! - Pure decompose/reconstruct functions (verifiable)
//! - Storage trait for database implementations

pub mod context;
pub mod decompose;
pub mod entity;
pub mod event_data;
pub mod reconstruct;
pub mod store;

pub use context::{ContextData, FlowContext, Segment};
pub use decompose::decompose;
pub use entity::EntitySnapshot;
pub use event_data::{
    CallbackType, EventData, EventRequest, EventResponse, EventStatus, EventType, ReqPayResult,
    ResolutionType,
};
pub use reconstruct::{reconstruct, ReconstructError};
pub use store::FlowStore;
