//! Generic event data structures for flow persistence.
//!
//! These types are domain-agnostic and can be used with any flow.
//! Request and response bodies are stored as JSON values.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// The status of an event in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
    /// Get the status name as a string.
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

/// A generic event data record for persistence.
///
/// This structure stores event metadata along with request and response
/// bodies as JSON values. The actual typed data lives in the domain layer
/// and is serialized/deserialized when storing/loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventData {
    /// Sequence number within the transaction (0-indexed).
    pub seq: i32,
    /// The type of event (domain-specific string).
    pub event_type: String,
    /// Current lifecycle status.
    pub status: EventStatus,
    /// Request body (serialized domain request as JSON).
    pub request_body: JsonValue,
    /// Response body, if received (serialized domain response as JSON).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_body: Option<JsonValue>,
}

impl EventData {
    /// Create a new event data record.
    pub fn new(seq: i32, event_type: impl Into<String>, request_body: JsonValue) -> Self {
        Self {
            seq,
            event_type: event_type.into(),
            status: EventStatus::Awaiting,
            request_body,
            response_body: None,
        }
    }

    /// Check if this event is the current (non-processed) event.
    pub fn is_current(&self) -> bool {
        self.status != EventStatus::Processed
    }

    /// Set the response body and update status to Received.
    pub fn set_response(&mut self, response_body: JsonValue) {
        self.response_body = Some(response_body);
        self.status = EventStatus::Received;
    }

    /// Mark the event as processed.
    pub fn mark_processed(&mut self) {
        self.status = EventStatus::Processed;
    }
}
