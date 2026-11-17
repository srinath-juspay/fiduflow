//! Event data types re-exported from psp_flow and driver_framework.
//!
//! - Categorized `FlowEvent` types from `psp_flow::persist`
//! - Generic `EventData` with classification from `driver_framework::event`

// Re-export categorized event types from psp_flow
pub use psp_flow::persist::{
    // Top-level event enum
    FlowEvent,
    // Classification enums (also in driver_framework)
    Direction, InitiatedBy, TargetSource, Timing,
    // Outgoing commands
    UserCommand, SystemCommand,
    // Incoming user-initiated
    UserRequest, SystemNotification,
    // Incoming responses
    UserSyncAck, UserAsyncResponse, SystemSyncAck, SystemAsyncCallback,
    // Legacy EventType for compatibility
    EventType,
};

// Re-export generic event storage structures from driver_framework
pub use driver_framework::{EventData, EventStatus};
