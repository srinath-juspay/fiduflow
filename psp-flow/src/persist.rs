//! Typed persistence structures for the PSP flow.
//!
//! This module provides:
//! - `FlowContext`: Cross-segment locals (e.g., is_high_value)
//! - Per-event request/response types that serialize directly to JSON
//!
//! Each event type has its own request and response struct with exactly
//! the fields it needs. These serialize via serde to JSON for storage.

use crate::values::*;
use serde::{Deserialize, Serialize};

// ============================================================================
// Flow Context (cross-segment locals)
// ============================================================================

/// Cross-segment local variables for the PSP flow.
///
/// These values are computed in one segment and used in later segments
/// for decision-making (e.g., SLA handling, timeout policies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FlowContext {
    /// High-value transaction flag.
    ///
    /// Set in Authenticated segment when `amount > HIGH_VALUE_THRESHOLD`.
    /// Used in Submitted/Deemed segments for different SLA handling
    /// (longer timeouts, priority processing).
    #[serde(default)]
    pub is_high_value: bool,
}

impl FlowContext {
    /// Create a new empty context (all defaults).
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a context with the high-value flag set.
    pub fn with_high_value(is_high_value: bool) -> Self {
        Self { is_high_value }
    }
}

/// Threshold for high-value transactions (in smallest currency unit).
pub const HIGH_VALUE_THRESHOLD: u64 = 100_000; // e.g., ₹1,00,000

// ============================================================================
// Event Type
// ============================================================================

/// The type of event being tracked.
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
    /// Get the event type name as a string.
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

    /// Parse event type from string.
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

// ============================================================================
// GetPaymentIntent Event
// ============================================================================

/// Request for GetPaymentIntent: ask user for payment intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetPaymentIntentRequest {
    pub payer: PayerId,
}

/// Response from GetPaymentIntent: raw intent from user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GetPaymentIntentResponse {
    pub intent: RawIntent,
}

// ============================================================================
// ShowError Event
// ============================================================================

/// Request for ShowError: display error to user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowErrorRequest {
    pub reason: Reason,
}

/// Response from ShowError: acknowledgment (unit response).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ShowErrorResponse;

// ============================================================================
// CollectMpin Event
// ============================================================================

/// Request for CollectMpin: ask user for MPIN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectMpinRequest {
    pub payer: PayerId,
    pub intent: ValidIntent,
}

/// Response from CollectMpin: user's MPIN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CollectMpinResponse {
    pub mpin: Mpin,
}

// ============================================================================
// Notify Event
// ============================================================================

/// Request for Notify: inform user of status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotifyRequest {
    pub status: Status,
}

/// Response from Notify: acknowledgment (unit response).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NotifyResponse;

// ============================================================================
// ReqPay Event
// ============================================================================

/// Request for ReqPay: send payment request to switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReqPayRequest {
    pub req: PayRequest,
}

/// Response from ReqPay: ack or rejection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReqPayResponse {
    pub result: ReqPayResult,
    /// Rejection reason (only set when result is Rejected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
}

/// The result of a ReqPay request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReqPayResult {
    Ack,
    Rejected,
}

impl ReqPayResponse {
    pub fn ack() -> Self {
        Self { result: ReqPayResult::Ack, reason: None }
    }

    pub fn rejected(reason: Reason) -> Self {
        Self { result: ReqPayResult::Rejected, reason: Some(reason) }
    }
}

// ============================================================================
// AwaitCallback Event
// ============================================================================

/// Request for AwaitCallback: wait for callback from switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwaitCallbackRequest {
    pub timeout_secs: u64,
}

/// Response from AwaitCallback: callback result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwaitCallbackResponse {
    pub result: CallbackResult,
}

/// The result of waiting for a callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallbackResult {
    Success { debit_ref: Ref, credit_ref: Ref },
    Failed { reason: Reason },
    Deemed,
    Timeout,
}

impl AwaitCallbackResponse {
    pub fn success(debit_ref: Ref, credit_ref: Ref) -> Self {
        Self { result: CallbackResult::Success { debit_ref, credit_ref } }
    }

    pub fn failed(reason: Reason) -> Self {
        Self { result: CallbackResult::Failed { reason } }
    }

    pub fn deemed() -> Self {
        Self { result: CallbackResult::Deemed }
    }

    pub fn timeout() -> Self {
        Self { result: CallbackResult::Timeout }
    }
}

// ============================================================================
// AwaitDeemedResolution Event
// ============================================================================

/// Request for AwaitDeemedResolution: wait for deemed transaction resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwaitDeemedResolutionRequest {
    pub deadline_days: u64,
}

/// Response from AwaitDeemedResolution: resolution result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwaitDeemedResolutionResponse {
    pub result: DeemedResolutionResult,
}

/// The result of deemed resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeemedResolutionResult {
    Success { credit_ref: Ref },
    Failed,
    DeadlineExpired,
}

impl AwaitDeemedResolutionResponse {
    pub fn success(credit_ref: Ref) -> Self {
        Self { result: DeemedResolutionResult::Success { credit_ref } }
    }

    pub fn failed() -> Self {
        Self { result: DeemedResolutionResult::Failed }
    }

    pub fn deadline_expired() -> Self {
        Self { result: DeemedResolutionResult::DeadlineExpired }
    }
}

// ============================================================================
// AwaitForcedReversal Event
// ============================================================================

/// Request for AwaitForcedReversal: wait for forced reversal to complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AwaitForcedReversalRequest {
    pub deadline_days: u64,
}

/// Response from AwaitForcedReversal: acknowledgment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AwaitForcedReversalResponse;
