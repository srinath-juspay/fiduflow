//! Event request/response data for lifecycle tracking.
//!
//! Events are stored with their request data (what we asked for) and response data
//! (what we received). The status tracks where in the lifecycle the event is.

use psp_flow::values::*;

/// The type of event being tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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
    /// Get the event type name as a string (for database storage).
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

    /// Parse an event type from a string.
    pub fn from_str(s: &str) -> Option<EventType> {
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

/// The status of an event in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventStatus {
    /// Event created but not yet sent.
    Initialized,
    /// Event sent, waiting for response.
    Awaiting,
    /// Response received but not yet processed.
    Received,
    /// Event fully processed, response consumed by the flow.
    Processed,
}

impl EventStatus {
    /// Get the status name as a string (for database storage).
    pub fn as_str(&self) -> &'static str {
        match self {
            EventStatus::Initialized => "Initialized",
            EventStatus::Awaiting => "Awaiting",
            EventStatus::Received => "Received",
            EventStatus::Processed => "Processed",
        }
    }

    /// Parse a status from a string.
    pub fn from_str(s: &str) -> Option<EventStatus> {
        match s {
            "Initialized" => Some(EventStatus::Initialized),
            "Awaiting" => Some(EventStatus::Awaiting),
            "Received" => Some(EventStatus::Received),
            "Processed" => Some(EventStatus::Processed),
            _ => None,
        }
    }
}

/// The request data: what we asked for in an event.
///
/// Fields are optional because different event types use different subsets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventRequest {
    pub event_type: Option<EventType>,
    pub payer: Option<PayerId>,
    pub intent: Option<ValidIntent>,
    pub reason: Option<Reason>,
    pub status: Option<Status>,
    pub req: Option<PayRequest>,
    pub timeout_secs: Option<u64>,
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

/// The type of ReqPay result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReqPayResult {
    Ack,
    Rejected,
}

impl ReqPayResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ReqPayResult::Ack => "Ack",
            ReqPayResult::Rejected => "Rejected",
        }
    }

    pub fn from_str(s: &str) -> Option<ReqPayResult> {
        match s {
            "Ack" => Some(ReqPayResult::Ack),
            "Rejected" => Some(ReqPayResult::Rejected),
            _ => None,
        }
    }
}

/// The type of callback received.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallbackType {
    Success,
    Failed,
    Deemed,
    Timeout,
}

impl CallbackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CallbackType::Success => "Success",
            CallbackType::Failed => "Failed",
            CallbackType::Deemed => "Deemed",
            CallbackType::Timeout => "Timeout",
        }
    }

    pub fn from_str(s: &str) -> Option<CallbackType> {
        match s {
            "Success" => Some(CallbackType::Success),
            "Failed" => Some(CallbackType::Failed),
            "Deemed" => Some(CallbackType::Deemed),
            "Timeout" => Some(CallbackType::Timeout),
            _ => None,
        }
    }
}

/// The type of deemed resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolutionType {
    DeemedSuccess,
    DeemedFailed,
    DeadlineExpired,
}

impl ResolutionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ResolutionType::DeemedSuccess => "DeemedSuccess",
            ResolutionType::DeemedFailed => "DeemedFailed",
            ResolutionType::DeadlineExpired => "DeadlineExpired",
        }
    }

    pub fn from_str(s: &str) -> Option<ResolutionType> {
        match s {
            "DeemedSuccess" => Some(ResolutionType::DeemedSuccess),
            "DeemedFailed" => Some(ResolutionType::DeemedFailed),
            "DeadlineExpired" => Some(ResolutionType::DeadlineExpired),
            _ => None,
        }
    }
}

/// The response data: what we received from an event.
///
/// Fields are optional because different event types produce different responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EventResponse {
    /// Raw intent returned from GetPaymentIntent.
    pub raw_intent: Option<RawIntent>,
    /// Mpin returned from CollectMpin.
    pub mpin: Option<Mpin>,
    /// Result type from ReqPay.
    pub req_pay_result: Option<ReqPayResult>,
    /// Rejection reason from ReqPay (when rejected).
    pub reject_reason: Option<Reason>,
    /// Callback type from AwaitCallback.
    pub callback_type: Option<CallbackType>,
    /// Debit ref from callback (on success).
    pub debit_ref: Option<Ref>,
    /// Credit ref from callback or resolution (on success).
    pub credit_ref: Option<Ref>,
    /// Failure reason from callback (on failure).
    pub callback_reason: Option<Reason>,
    /// Resolution type from AwaitDeemedResolution.
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

/// A complete event data record combining request, response, and lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventData {
    /// Sequence number within the transaction (0-indexed).
    pub seq: i32,
    /// The type of event.
    pub event_type: EventType,
    /// Current lifecycle status.
    pub status: EventStatus,
    /// What we asked for.
    pub request: EventRequest,
    /// What we received (if any).
    pub response: Option<EventResponse>,
}

impl EventData {
    /// Create a new event data record.
    pub fn new(seq: i32, event_type: EventType, request: EventRequest) -> Self {
        Self {
            seq,
            event_type,
            status: EventStatus::Awaiting,
            request,
            response: None,
        }
    }

    /// Check if this event is the current (non-processed) event.
    pub fn is_current(&self) -> bool {
        self.status != EventStatus::Processed
    }
}
