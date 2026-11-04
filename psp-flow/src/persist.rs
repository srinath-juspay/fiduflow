//! Typed persistence structures for event request/response data.
//!
//! These types capture the domain-specific data needed to persist and reconstruct
//! flow events. They are serializable via serde for storage as JSON.

use crate::values::*;
use serde::{Deserialize, Serialize};

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
}

/// The type of ReqPay result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReqPayResult {
    Ack,
    Rejected,
}

/// The type of callback received.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallbackType {
    Success,
    Failed,
    Deemed,
    Timeout,
}

/// The type of deemed resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionType {
    DeemedSuccess,
    DeemedFailed,
    DeadlineExpired,
}

/// The request data: what we asked for in an event.
///
/// Fields are optional because different event types use different subsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EventRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_type: Option<EventType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payer: Option<PayerId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub intent: Option<ValidIntent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<Reason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req: Option<PayRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline_days: Option<u64>,
}

impl EventRequest {
    /// Create a request for GetPaymentIntent.
    pub fn get_payment_intent(payer: PayerId) -> Self {
        Self {
            event_type: Some(EventType::GetPaymentIntent),
            payer: Some(payer),
            ..Default::default()
        }
    }

    /// Create a request for ShowError.
    pub fn show_error(reason: Reason) -> Self {
        Self {
            event_type: Some(EventType::ShowError),
            reason: Some(reason),
            ..Default::default()
        }
    }

    /// Create a request for CollectMpin.
    pub fn collect_mpin(payer: PayerId, intent: ValidIntent) -> Self {
        Self {
            event_type: Some(EventType::CollectMpin),
            payer: Some(payer),
            intent: Some(intent),
            ..Default::default()
        }
    }

    /// Create a request for Notify.
    pub fn notify(status: Status) -> Self {
        Self {
            event_type: Some(EventType::Notify),
            status: Some(status),
            ..Default::default()
        }
    }

    /// Create a request for ReqPay.
    pub fn req_pay(req: PayRequest) -> Self {
        Self {
            event_type: Some(EventType::ReqPay),
            req: Some(req),
            ..Default::default()
        }
    }

    /// Create a request for AwaitCallback.
    pub fn await_callback(timeout_secs: u64) -> Self {
        Self {
            event_type: Some(EventType::AwaitCallback),
            timeout_secs: Some(timeout_secs),
            ..Default::default()
        }
    }

    /// Create a request for AwaitDeemedResolution.
    pub fn await_deemed_resolution(deadline_days: u64) -> Self {
        Self {
            event_type: Some(EventType::AwaitDeemedResolution),
            deadline_days: Some(deadline_days),
            ..Default::default()
        }
    }

    /// Create a request for AwaitForcedReversal.
    pub fn await_forced_reversal(deadline_days: u64) -> Self {
        Self {
            event_type: Some(EventType::AwaitForcedReversal),
            deadline_days: Some(deadline_days),
            ..Default::default()
        }
    }
}

/// The response data: what we received from an event.
///
/// Fields are optional because different event types produce different responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EventResponse {
    /// Raw intent returned from GetPaymentIntent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_intent: Option<RawIntent>,
    /// Mpin returned from CollectMpin.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mpin: Option<Mpin>,
    /// Result type from ReqPay.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub req_pay_result: Option<ReqPayResult>,
    /// Rejection reason from ReqPay (when rejected).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reject_reason: Option<Reason>,
    /// Callback type from AwaitCallback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_type: Option<CallbackType>,
    /// Debit ref from callback (on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debit_ref: Option<Ref>,
    /// Credit ref from callback or resolution (on success).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credit_ref: Option<Ref>,
    /// Failure reason from callback (on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub callback_reason: Option<Reason>,
    /// Resolution type from AwaitDeemedResolution.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution_type: Option<ResolutionType>,
}

impl EventResponse {
    /// Create a response for a raw intent.
    pub fn intent(raw_intent: RawIntent) -> Self {
        Self {
            raw_intent: Some(raw_intent),
            ..Default::default()
        }
    }

    /// Create a response for mpin.
    pub fn mpin(mpin: Mpin) -> Self {
        Self {
            mpin: Some(mpin),
            ..Default::default()
        }
    }

    /// Create a response for a unit event (ShowError, Notify).
    pub fn unit() -> Self {
        Self::default()
    }

    /// Create a response for ReqPay Ack.
    pub fn req_pay_ack() -> Self {
        Self {
            req_pay_result: Some(ReqPayResult::Ack),
            ..Default::default()
        }
    }

    /// Create a response for ReqPay Rejected.
    pub fn req_pay_rejected(reason: Reason) -> Self {
        Self {
            req_pay_result: Some(ReqPayResult::Rejected),
            reject_reason: Some(reason),
            ..Default::default()
        }
    }

    /// Create a response for Callback Success.
    pub fn callback_success(debit_ref: Ref, credit_ref: Ref) -> Self {
        Self {
            callback_type: Some(CallbackType::Success),
            debit_ref: Some(debit_ref),
            credit_ref: Some(credit_ref),
            ..Default::default()
        }
    }

    /// Create a response for Callback Failed.
    pub fn callback_failed(reason: Reason) -> Self {
        Self {
            callback_type: Some(CallbackType::Failed),
            callback_reason: Some(reason),
            ..Default::default()
        }
    }

    /// Create a response for Callback Deemed.
    pub fn callback_deemed() -> Self {
        Self {
            callback_type: Some(CallbackType::Deemed),
            ..Default::default()
        }
    }

    /// Create a response for Callback Timeout.
    pub fn callback_timeout() -> Self {
        Self {
            callback_type: Some(CallbackType::Timeout),
            ..Default::default()
        }
    }

    /// Create a response for DeemedSuccess resolution.
    pub fn resolution_success(credit_ref: Ref) -> Self {
        Self {
            resolution_type: Some(ResolutionType::DeemedSuccess),
            credit_ref: Some(credit_ref),
            ..Default::default()
        }
    }

    /// Create a response for DeemedFailed resolution.
    pub fn resolution_failed() -> Self {
        Self {
            resolution_type: Some(ResolutionType::DeemedFailed),
            ..Default::default()
        }
    }

    /// Create a response for DeadlineExpired resolution.
    pub fn resolution_deadline_expired() -> Self {
        Self {
            resolution_type: Some(ResolutionType::DeadlineExpired),
            ..Default::default()
        }
    }
}
