//! Generic event data structures for flow persistence.
//!
//! These types are domain-agnostic and can be used with any flow.
//! Event classification:
//! - Source: User | ThisSystem | ExternalSystem
//! - Target: User | ThisSystem | ExternalSystem
//! - Method: Request | Ack | AsyncResponse | Notify
//!
//! Correlation:
//! - Ack and AsyncResponse have `in_response_to` pointing to the Request's seq
//! - Request has `expects_async` indicating if AsyncResponse is expected after Ack

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// The status of an event in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventStatus {
    /// Event created but not yet sent/processed.
    Pending,
    /// Event sent/received, processing complete.
    Completed,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventStatus::Pending => "Pending",
            EventStatus::Completed => "Completed",
        }
    }

    pub fn from_str(s: &str) -> Option<EventStatus> {
        match s {
            "Pending" => Some(EventStatus::Pending),
            "Completed" => Some(EventStatus::Completed),
            _ => None,
        }
    }
}

/// An actor in the event flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Actor {
    /// End user (customer, client app)
    User,
    /// This system (our application)
    ThisSystem,
    /// External system (switch, bank, third-party service)
    ExternalSystem,
}

impl Actor {
    pub fn as_str(&self) -> &'static str {
        match self {
            Actor::User => "User",
            Actor::ThisSystem => "ThisSystem",
            Actor::ExternalSystem => "ExternalSystem",
        }
    }

    pub fn from_str(s: &str) -> Option<Actor> {
        match s {
            "User" => Some(Actor::User),
            "ThisSystem" => Some(Actor::ThisSystem),
            "ExternalSystem" => Some(Actor::ExternalSystem),
            _ => None,
        }
    }
}

/// The method/nature of the event interaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Method {
    /// Request: Initiates an interaction (expects Ack, optionally AsyncResponse)
    Request,
    /// Ack: Synchronous acknowledgment to a Request
    Ack,
    /// AsyncResponse: Asynchronous response to a Request
    AsyncResponse,
    /// Notify: One-way notification (no response expected)
    Notify,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Request => "Request",
            Method::Ack => "Ack",
            Method::AsyncResponse => "AsyncResponse",
            Method::Notify => "Notify",
        }
    }

    pub fn from_str(s: &str) -> Option<Method> {
        match s {
            "Request" => Some(Method::Request),
            "Ack" => Some(Method::Ack),
            "AsyncResponse" => Some(Method::AsyncResponse),
            "Notify" => Some(Method::Notify),
            _ => None,
        }
    }
}

/// A generic event data record for persistence.
///
/// This structure stores event metadata with classification
/// (source, target, method) along with the event body as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventData {
    /// Sequence number within the transaction (0-indexed).
    pub seq: i32,

    // === Classification ===
    /// Source actor: who sent this event.
    pub source: Actor,
    /// Target actor: who receives this event.
    pub target: Actor,
    /// Method: the nature of this event interaction.
    pub method: Method,

    // === Correlation ===
    /// For Ack/AsyncResponse: the seq of the Request this responds to.
    /// None for Request and Notify.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_response_to: Option<i32>,
    /// For Request: whether an AsyncResponse is expected after Ack.
    /// Ignored for other methods.
    #[serde(default)]
    pub expects_async: bool,

    // === Payload ===
    /// The specific event type name.
    pub event_type: String,
    /// Event body (serialized domain event as JSON).
    pub event_body: JsonValue,

    // === Status ===
    /// Current lifecycle status.
    pub status: EventStatus,
}

impl EventData {
    /// Create a Request event (initiates interaction).
    ///
    /// - `expects_async`: true if AsyncResponse is expected after Ack
    pub fn request(
        seq: i32,
        source: Actor,
        target: Actor,
        expects_async: bool,
        event_type: impl Into<String>,
        event_body: JsonValue,
    ) -> Self {
        Self {
            seq,
            source,
            target,
            method: Method::Request,
            in_response_to: None,
            expects_async,
            event_type: event_type.into(),
            event_body,
            status: EventStatus::Pending,
        }
    }

    /// Create an Ack event (synchronous acknowledgment).
    ///
    /// - `in_response_to`: seq of the Request being acknowledged
    pub fn ack(
        seq: i32,
        source: Actor,
        target: Actor,
        in_response_to: i32,
        event_type: impl Into<String>,
        event_body: JsonValue,
    ) -> Self {
        Self {
            seq,
            source,
            target,
            method: Method::Ack,
            in_response_to: Some(in_response_to),
            expects_async: false,
            event_type: event_type.into(),
            event_body,
            status: EventStatus::Pending,
        }
    }

    /// Create an AsyncResponse event (delayed response).
    ///
    /// - `in_response_to`: seq of the Request being responded to
    pub fn async_response(
        seq: i32,
        source: Actor,
        target: Actor,
        in_response_to: i32,
        event_type: impl Into<String>,
        event_body: JsonValue,
    ) -> Self {
        Self {
            seq,
            source,
            target,
            method: Method::AsyncResponse,
            in_response_to: Some(in_response_to),
            expects_async: false,
            event_type: event_type.into(),
            event_body,
            status: EventStatus::Pending,
        }
    }

    /// Create a Notify event (one-way notification).
    pub fn notify(
        seq: i32,
        source: Actor,
        target: Actor,
        event_type: impl Into<String>,
        event_body: JsonValue,
    ) -> Self {
        Self {
            seq,
            source,
            target,
            method: Method::Notify,
            in_response_to: None,
            expects_async: false,
            event_type: event_type.into(),
            event_body,
            status: EventStatus::Pending,
        }
    }

    // === Convenience constructors for common patterns ===

    /// Request from ThisSystem to User (ack only).
    pub fn request_to_user(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(seq, Actor::ThisSystem, Actor::User, false, event_type, body)
    }

    /// Request from ThisSystem to User (expects async response).
    pub fn request_to_user_async(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(seq, Actor::ThisSystem, Actor::User, true, event_type, body)
    }

    /// Request from ThisSystem to ExternalSystem (ack only).
    pub fn request_to_system(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(seq, Actor::ThisSystem, Actor::ExternalSystem, false, event_type, body)
    }

    /// Request from ThisSystem to ExternalSystem (expects async response).
    pub fn request_to_system_async(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(seq, Actor::ThisSystem, Actor::ExternalSystem, true, event_type, body)
    }

    /// AsyncResponse from ExternalSystem to ThisSystem.
    pub fn async_response_from_system(
        seq: i32,
        in_response_to: i32,
        event_type: impl Into<String>,
        body: JsonValue,
    ) -> Self {
        Self::async_response(seq, Actor::ExternalSystem, Actor::ThisSystem, in_response_to, event_type, body)
    }

    /// AsyncResponse from User to ThisSystem.
    pub fn async_response_from_user(
        seq: i32,
        in_response_to: i32,
        event_type: impl Into<String>,
        body: JsonValue,
    ) -> Self {
        Self::async_response(seq, Actor::User, Actor::ThisSystem, in_response_to, event_type, body)
    }

    /// Mark the event as completed.
    pub fn mark_completed(&mut self) {
        self.status = EventStatus::Completed;
    }

    /// Check if this event is pending.
    pub fn is_pending(&self) -> bool {
        self.status == EventStatus::Pending
    }

    /// Check if this is a Request that expects an async response.
    pub fn expects_async_response(&self) -> bool {
        self.method == Method::Request && self.expects_async
    }
}
