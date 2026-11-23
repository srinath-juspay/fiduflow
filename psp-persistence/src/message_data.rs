//! Message data types re-exported from psp_flow and driver_framework.
//!
//! - Categorized `FlowEvent` types from `psp_flow::persist`
//! - Generic `MessageRecord` with classification from `driver_framework::message`

// Re-export categorized event types from psp_flow
pub use psp_flow::persist::{
    // Top-level event enum
    FlowEvent,
    // Classification enums (also in driver_framework)
    Actor, Method,
    // Outgoing commands
    UserCommand, SystemCommand,
    // Incoming user-initiated
    UserRequest, SystemNotification,
    // Incoming responses
    UserSyncAck, UserAsyncResponse, SystemSyncAck, SystemAsyncCallback,
};

// Re-export generic message storage structures from driver_framework
pub use driver_framework::{MessageRecord, MessageStatus};
