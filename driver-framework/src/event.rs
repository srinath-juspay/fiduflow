//! Generic event data structures for flow persistence.
//!
//! These types are domain-agnostic and can be used with any flow.
//! Event classification:
//! - Source: User | ThisSystem | ExternalSystem
//! - Target: User | ThisSystem | ExternalSystem
//! - Method: Command | Request | Notification | SyncAck | AsyncResponse | AsyncCallback

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
    /// Command (initiates an interaction, expects response)
    Command,
    /// Request (they initiate, not a response to our command)
    Request,
    /// Notification (unsolicited, informational)
    Notification,
    /// SyncAck (immediate acknowledgment to a command)
    SyncAck,
    /// AsyncResponse (delayed response with data)
    AsyncResponse,
    /// AsyncCallback (async result from external processing)
    AsyncCallback,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Command => "Command",
            Method::Request => "Request",
            Method::Notification => "Notification",
            Method::SyncAck => "SyncAck",
            Method::AsyncResponse => "AsyncResponse",
            Method::AsyncCallback => "AsyncCallback",
        }
    }

    pub fn from_str(s: &str) -> Option<Method> {
        match s {
            "Command" => Some(Method::Command),
            "Request" => Some(Method::Request),
            "Notification" => Some(Method::Notification),
            "SyncAck" => Some(Method::SyncAck),
            "AsyncResponse" => Some(Method::AsyncResponse),
            "AsyncCallback" => Some(Method::AsyncCallback),
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
    /// Create a new event data record.
    pub fn new(
        seq: i32,
        source: Actor,
        target: Actor,
        method: Method,
        event_type: impl Into<String>,
        event_body: JsonValue,
    ) -> Self {
        Self {
            seq,
            source,
            target,
            method,
            event_type: event_type.into(),
            event_body,
            status: EventStatus::Pending,
        }
    }

    /// Create an outgoing user command event (ThisSystem → User).
    pub fn user_command(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::ThisSystem,
            Actor::User,
            Method::Command,
            event_type,
            body,
        )
    }

    /// Create an outgoing system command event (ThisSystem → ExternalSystem).
    pub fn system_command(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::ThisSystem,
            Actor::ExternalSystem,
            Method::Command,
            event_type,
            body,
        )
    }

    /// Create an incoming user request event (User → ThisSystem).
    pub fn user_request(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::User,
            Actor::ThisSystem,
            Method::Request,
            event_type,
            body,
        )
    }

    /// Create an incoming user sync ack event (User → ThisSystem).
    pub fn user_sync_ack(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::User,
            Actor::ThisSystem,
            Method::SyncAck,
            event_type,
            body,
        )
    }

    /// Create an incoming user async response event (User → ThisSystem).
    pub fn user_async_response(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::User,
            Actor::ThisSystem,
            Method::AsyncResponse,
            event_type,
            body,
        )
    }

    /// Create an incoming system sync ack event (ExternalSystem → ThisSystem).
    pub fn system_sync_ack(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::ExternalSystem,
            Actor::ThisSystem,
            Method::SyncAck,
            event_type,
            body,
        )
    }

    /// Create an incoming system async callback event (ExternalSystem → ThisSystem).
    pub fn system_async_callback(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::ExternalSystem,
            Actor::ThisSystem,
            Method::AsyncCallback,
            event_type,
            body,
        )
    }

    /// Create a system notification event (ExternalSystem → ThisSystem).
    pub fn system_notification(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Actor::ExternalSystem,
            Actor::ThisSystem,
            Method::Notification,
            event_type,
            body,
        )
    }

    /// Mark the event as completed.
    pub fn mark_completed(&mut self) {
        self.status = EventStatus::Completed;
    }

    /// Check if this event is pending.
    pub fn is_pending(&self) -> bool {
        self.status == EventStatus::Pending
    }
}
