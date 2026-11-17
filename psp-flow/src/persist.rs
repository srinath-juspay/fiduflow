//! Typed persistence structures for the PSP flow.
//!
//! This module provides:
//! - `FlowContext`: Cross-segment locals (e.g., is_high_value)
//! - `FlowEvent`: Categorized events (source, target, method)
//!
//! Event classification (re-exported from driver-framework):
//! - Source/Target: User | ThisSystem | ExternalSystem
//! - Method: Command | Request | Notification | SyncAck | AsyncResponse | AsyncCallback

use crate::values::*;
use serde::{Deserialize, Serialize};

// Re-export classification types from driver-framework for consistent usage
pub use driver_framework::{Actor, EventData, EventStatus, Method};

// ============================================================================
// Flow Context (cross-segment locals)
// ============================================================================

/// Cross-segment local variables for the PSP flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowContext {
    /// High-value transaction flag.
    /// Set in Authenticated segment when `amount > HIGH_VALUE_THRESHOLD`.
    #[serde(default)]
    pub is_high_value: bool,
}

impl FlowContext {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_high_value(is_high_value: bool) -> Self {
        Self { is_high_value }
    }
}

/// Threshold for high-value transactions (in smallest currency unit).
pub const HIGH_VALUE_THRESHOLD: u64 = 100_000;

// ============================================================================
// Flow Event (top-level categorized enum)
// ============================================================================

/// All flow events - categorized by source, target, method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FlowEvent {
    // ======= OUTGOING (ThisSystem → ...) =======

    /// Command to user (expects sync ack, may have async response)
    UserCommand(UserCommand),
    /// Command to external system (expects sync ack, may have async callback)
    SystemCommand(SystemCommand),

    // ======= INCOMING (... → ThisSystem, they initiate) =======

    /// User initiates request (not a response to our command)
    UserRequest(UserRequest),
    /// External system sends notification (unsolicited)
    SystemNotification(SystemNotification),

    // ======= INCOMING (... → ThisSystem, response to our command) =======

    /// Sync ack from user (command accepted/rejected)
    UserSyncAck(UserSyncAck),
    /// Async response from user (actual data/completion)
    UserAsyncResponse(UserAsyncResponse),
    /// Sync ack from external system (command accepted/rejected)
    SystemSyncAck(SystemSyncAck),
    /// Async callback from external system (result of async processing)
    SystemAsyncCallback(SystemAsyncCallback),
}

impl FlowEvent {
    /// Get the source actor of this event.
    pub fn source(&self) -> Actor {
        match self {
            FlowEvent::UserCommand(_) | FlowEvent::SystemCommand(_) => Actor::ThisSystem,
            FlowEvent::UserRequest(_)
            | FlowEvent::UserSyncAck(_)
            | FlowEvent::UserAsyncResponse(_) => Actor::User,
            FlowEvent::SystemNotification(_)
            | FlowEvent::SystemSyncAck(_)
            | FlowEvent::SystemAsyncCallback(_) => Actor::ExternalSystem,
        }
    }

    /// Get the target actor of this event.
    pub fn target(&self) -> Actor {
        match self {
            FlowEvent::UserCommand(_) => Actor::User,
            FlowEvent::SystemCommand(_) => Actor::ExternalSystem,
            _ => Actor::ThisSystem,
        }
    }

    /// Get the method/nature of this event.
    pub fn method(&self) -> Method {
        match self {
            FlowEvent::UserCommand(_) | FlowEvent::SystemCommand(_) => Method::Command,
            FlowEvent::UserRequest(_) => Method::Request,
            FlowEvent::SystemNotification(_) => Method::Notification,
            FlowEvent::UserSyncAck(_) | FlowEvent::SystemSyncAck(_) => Method::SyncAck,
            FlowEvent::UserAsyncResponse(_) => Method::AsyncResponse,
            FlowEvent::SystemAsyncCallback(_) => Method::AsyncCallback,
        }
    }

    /// Get the event type name as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            FlowEvent::UserCommand(c) => c.event_type(),
            FlowEvent::SystemCommand(c) => c.event_type(),
            FlowEvent::UserRequest(r) => r.event_type(),
            FlowEvent::SystemNotification(n) => n.event_type(),
            FlowEvent::UserSyncAck(a) => a.event_type(),
            FlowEvent::UserAsyncResponse(r) => r.event_type(),
            FlowEvent::SystemSyncAck(a) => a.event_type(),
            FlowEvent::SystemAsyncCallback(c) => c.event_type(),
        }
    }
}

// ============================================================================
// OUTGOING: Commands we send
// ============================================================================

/// Commands we send to the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserCommand {
    /// Ask user for payment intent (async response expected)
    GetPaymentIntent { payer: PayerId },
    /// Ask user for MPIN (async response expected)
    CollectMpin { payer: PayerId, intent: ValidIntent },
    /// Show error to user (sync ack expected)
    ShowError { reason: Reason },
    /// Notify user of status (sync ack expected)
    Notify { status: Status },
}

impl UserCommand {
    pub fn event_type(&self) -> &'static str {
        match self {
            UserCommand::GetPaymentIntent { .. } => "GetPaymentIntent",
            UserCommand::CollectMpin { .. } => "CollectMpin",
            UserCommand::ShowError { .. } => "ShowError",
            UserCommand::Notify { .. } => "Notify",
        }
    }
}

/// Commands we send to external systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemCommand {
    /// Send payment request to switch (sync ack + async callback expected)
    ReqPay { req: PayRequest },
}

impl SystemCommand {
    pub fn event_type(&self) -> &'static str {
        match self {
            SystemCommand::ReqPay { .. } => "ReqPay",
        }
    }
}

// ============================================================================
// INCOMING: User-initiated (not response to our command)
// ============================================================================

/// Requests initiated by the user (not in response to our command).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserRequest {
    /// User initiates a payment
    InitiatePayment { payer: PayerId },
    /// User cancels the transaction
    Cancel { reason: Reason },
    /// User requests status check
    StatusCheck,
}

impl UserRequest {
    pub fn event_type(&self) -> &'static str {
        match self {
            UserRequest::InitiatePayment { .. } => "InitiatePayment",
            UserRequest::Cancel { .. } => "Cancel",
            UserRequest::StatusCheck => "StatusCheck",
        }
    }
}

/// Unsolicited notifications from external systems.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemNotification {
    /// Fraud alert from risk system
    FraudAlert,
    /// Account freeze notification
    AccountFreeze,
}

impl SystemNotification {
    pub fn event_type(&self) -> &'static str {
        match self {
            SystemNotification::FraudAlert => "FraudAlert",
            SystemNotification::AccountFreeze => "AccountFreeze",
        }
    }
}

// ============================================================================
// INCOMING: Responses to our commands
// ============================================================================

/// Sync ack from user (immediate response to our command).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserSyncAck {
    /// Command accepted by user/client
    Accepted,
    /// Command rejected by user/client
    Rejected { reason: Reason },
}

impl UserSyncAck {
    pub fn event_type(&self) -> &'static str {
        match self {
            UserSyncAck::Accepted => "UserAccepted",
            UserSyncAck::Rejected { .. } => "UserRejected",
        }
    }
}

/// Async response from user (delayed response with data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UserAsyncResponse {
    /// User provided payment intent
    PaymentIntent { intent: RawIntent },
    /// User provided MPIN
    Mpin { mpin: Mpin },
    /// User acknowledged error display
    ErrorAck,
    /// User acknowledged notification
    NotifyAck,
}

impl UserAsyncResponse {
    pub fn event_type(&self) -> &'static str {
        match self {
            UserAsyncResponse::PaymentIntent { .. } => "PaymentIntent",
            UserAsyncResponse::Mpin { .. } => "Mpin",
            UserAsyncResponse::ErrorAck => "ErrorAck",
            UserAsyncResponse::NotifyAck => "NotifyAck",
        }
    }
}

/// Sync ack from system (immediate response to our command).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemSyncAck {
    /// ReqPay accepted by switch
    ReqPayAccepted,
    /// ReqPay rejected by switch
    ReqPayRejected { reason: Reason },
}

impl SystemSyncAck {
    pub fn event_type(&self) -> &'static str {
        match self {
            SystemSyncAck::ReqPayAccepted => "ReqPayAccepted",
            SystemSyncAck::ReqPayRejected { .. } => "ReqPayRejected",
        }
    }
}

/// Async callback from system (result of async processing).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SystemAsyncCallback {
    /// Payment succeeded
    PaymentSuccess { debit_ref: Ref, credit_ref: Ref },
    /// Payment failed
    PaymentFailed { reason: Reason },
    /// Payment deemed (uncertain state)
    PaymentDeemed,
    /// Callback timeout
    PaymentTimeout,
    /// Deemed resolution: success
    DeemedSuccess { credit_ref: Ref },
    /// Deemed resolution: failed
    DeemedFailed,
    /// Deemed resolution: deadline expired
    DeemedExpired,
    /// Forced reversal complete
    ReversalComplete,
}

impl SystemAsyncCallback {
    pub fn event_type(&self) -> &'static str {
        match self {
            SystemAsyncCallback::PaymentSuccess { .. } => "PaymentSuccess",
            SystemAsyncCallback::PaymentFailed { .. } => "PaymentFailed",
            SystemAsyncCallback::PaymentDeemed => "PaymentDeemed",
            SystemAsyncCallback::PaymentTimeout => "PaymentTimeout",
            SystemAsyncCallback::DeemedSuccess { .. } => "DeemedSuccess",
            SystemAsyncCallback::DeemedFailed => "DeemedFailed",
            SystemAsyncCallback::DeemedExpired => "DeemedExpired",
            SystemAsyncCallback::ReversalComplete => "ReversalComplete",
        }
    }
}

// ============================================================================
// Legacy type aliases for compatibility (deprecated)
// ============================================================================

/// Event type enum (for backward compatibility with storage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    GetPaymentIntent,
    ShowError,
    CollectMpin,
    Notify,
    ReqPay,
    AwaitCallback,
    AwaitDeemedResolution,
    AwaitForcedReversal,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::GetPaymentIntent => "GetPaymentIntent",
            EventType::ShowError => "ShowError",
            EventType::CollectMpin => "CollectMpin",
            EventType::Notify => "Notify",
            EventType::ReqPay => "ReqPay",
            EventType::AwaitCallback => "AwaitCallback",
            EventType::AwaitDeemedResolution => "AwaitDeemedResolution",
            EventType::AwaitForcedReversal => "AwaitForcedReversal",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "GetPaymentIntent" => Some(EventType::GetPaymentIntent),
            "ShowError" => Some(EventType::ShowError),
            "CollectMpin" => Some(EventType::CollectMpin),
            "Notify" => Some(EventType::Notify),
            "ReqPay" => Some(EventType::ReqPay),
            "AwaitCallback" => Some(EventType::AwaitCallback),
            "AwaitDeemedResolution" => Some(EventType::AwaitDeemedResolution),
            "AwaitForcedReversal" => Some(EventType::AwaitForcedReversal),
            _ => None,
        }
    }
}
