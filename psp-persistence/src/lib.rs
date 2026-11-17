//! Pure persistence bridge layer for the PSP flow.
//!
//! This crate provides:
//! - Typed entity snapshots per state
//! - Cross-segment context (FlowContext from psp_flow, ContextData from driver_framework)
//! - Message records (FlowEvent from psp_flow, MessageRecord from driver_framework)
//! - Pure decompose/reconstruct functions (verifiable)
//! - Storage trait for database implementations

pub mod context;
pub mod decompose;
pub mod entity;
pub mod message_data;
pub mod reconstruct;
pub mod store;

pub use context::{ContextData, FlowContext, Segment};
pub use decompose::decompose;
pub use entity::EntitySnapshot;
pub use message_data::{
    // Generic storage types
    MessageRecord, MessageStatus,
    // Classification enums
    Actor, Method,
    // Categorized event types
    FlowEvent,
    UserCommand, SystemCommand,
    UserRequest, SystemNotification,
    UserSyncAck, UserAsyncResponse, SystemSyncAck, SystemAsyncCallback,
    // Legacy
    EventType,
};
pub use reconstruct::reconstruct;
// Re-export from driver-framework
pub use driver_framework::ReconstructError;
pub use store::FlowStore;
