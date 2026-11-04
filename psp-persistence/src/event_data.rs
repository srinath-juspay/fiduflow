//! Event data types re-exported from psp_flow and driver_framework.
//!
//! - Typed `EventRequest`/`EventResponse` from `psp_flow::persist`
//! - Generic `EventData`/`EventStatus` from `driver_framework::event`

// Re-export typed domain event structures from psp_flow
pub use psp_flow::persist::{
    CallbackType, EventRequest, EventResponse, EventType, ReqPayResult, ResolutionType,
};

// Re-export generic event storage structures from driver_framework
pub use driver_framework::{EventData, EventStatus};
