//! Event data types re-exported from psp_flow and driver_framework.
//!
//! - Per-event request/response types from `psp_flow::persist`
//! - Generic `EventData`/`EventStatus` from `driver_framework::event`

// Re-export per-event typed request/response structures from psp_flow
pub use psp_flow::persist::{
    // Event type enum
    EventType,
    // GetPaymentIntent
    GetPaymentIntentRequest, GetPaymentIntentResponse,
    // ShowError
    ShowErrorRequest, ShowErrorResponse,
    // CollectMpin
    CollectMpinRequest, CollectMpinResponse,
    // Notify
    NotifyRequest, NotifyResponse,
    // ReqPay
    ReqPayRequest, ReqPayResponse, ReqPayResult,
    // AwaitCallback
    AwaitCallbackRequest, AwaitCallbackResponse, CallbackResult,
    // AwaitDeemedResolution
    AwaitDeemedResolutionRequest, AwaitDeemedResolutionResponse, DeemedResolutionResult,
    // AwaitForcedReversal
    AwaitForcedReversalRequest, AwaitForcedReversalResponse,
};

// Re-export generic event storage structures from driver_framework
pub use driver_framework::{EventData, EventStatus};
