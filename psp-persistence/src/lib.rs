//! Pure persistence bridge layer for the PSP flow.
//!
//! This crate provides:
//! - Typed entity snapshots per state
//! - Cross-segment context (FlowContext from psp_flow, ContextData from driver_framework)
//! - Categorized event data (FlowEvent from psp_flow, EventData from driver_framework)
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
    // Generic storage types
    EventData, EventStatus,
    // Classification enums
    Direction, InitiatedBy, TargetSource, Timing,
    // Categorized event types
    FlowEvent,
    UserCommand, SystemCommand,
    UserRequest, SystemNotification,
    UserSyncAck, UserAsyncResponse, SystemSyncAck, SystemAsyncCallback,
    // Legacy
    EventType,
};
pub use reconstruct::{reconstruct, ReconstructError};
pub use store::FlowStore;
