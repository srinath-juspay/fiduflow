//! Generic message record structures for flow persistence.
//!
//! These types are domain-agnostic and can be used with any flow.
//! Message classification:
//! - Source: User | ThisSystem | ExternalSystem
//! - Target: User | ThisSystem | ExternalSystem
//! - Method: Request | Ack | AsyncResponse | Notify
//!
//! Correlation:
//! - Ack and AsyncResponse have `in_response_to` pointing to the Request's id
//! - Request has `expects_async` indicating if AsyncResponse is expected after Ack
//!
//! ID and ordering:
//! - `id` is a GUID assigned by the store on insert
//! - Ordering is by `created_at` timestamp (managed by store)

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// The status of a message in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MessageStatus {
    /// Message created but not yet sent/processed.
    Pending,
    /// Message sent/received, processing complete.
    Completed,
}

impl MessageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            MessageStatus::Pending => "Pending",
            MessageStatus::Completed => "Completed",
        }
    }

    pub fn from_str(s: &str) -> Option<MessageStatus> {
        match s {
            "Pending" => Some(MessageStatus::Pending),
            "Completed" => Some(MessageStatus::Completed),
            _ => None,
        }
    }
}

/// An actor in the message flow.
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

/// The method/nature of the message interaction.
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

/// A generic message record for persistence.
///
/// This structure stores message metadata with classification
/// (source, target, method) along with the message body as JSON.
///
/// The `id` field is assigned by the store on insert. When creating
/// a new message, use the constructors which set `id` to `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageRecord {
    /// Unique identifier (GUID). Assigned by store on insert.
    /// None when creating a new message, Some after stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    // === Classification ===
    /// Source actor: who sent this message.
    pub source: Actor,
    /// Target actor: who receives this message.
    pub target: Actor,
    /// Method: the nature of this message interaction.
    pub method: Method,

    // === Correlation ===
    /// For Ack/AsyncResponse: the id of the Request this responds to.
    /// None for Request and Notify.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_response_to: Option<String>,
    /// For Request: whether an AsyncResponse is expected after Ack.
    /// Ignored for other methods.
    #[serde(default)]
    pub expects_async: bool,

    // === Payload ===
    /// The specific message type name.
    pub message_type: String,
    /// Message body (serialized domain event as JSON).
    pub message_body: JsonValue,

    // === Status ===
    /// Current lifecycle status.
    pub status: MessageStatus,
}

impl MessageRecord {
    /// Create a Request message (initiates interaction).
    ///
    /// - `expects_async`: true if AsyncResponse is expected after Ack
    pub fn request(
        source: Actor,
        target: Actor,
        expects_async: bool,
        message_type: impl Into<String>,
        message_body: JsonValue,
    ) -> Self {
        Self {
            id: None,
            source,
            target,
            method: Method::Request,
            in_response_to: None,
            expects_async,
            message_type: message_type.into(),
            message_body,
            status: MessageStatus::Pending,
        }
    }

    /// Create an Ack message (synchronous acknowledgment).
    ///
    /// - `in_response_to`: id of the Request being acknowledged
    pub fn ack(
        source: Actor,
        target: Actor,
        in_response_to: impl Into<String>,
        message_type: impl Into<String>,
        message_body: JsonValue,
    ) -> Self {
        Self {
            id: None,
            source,
            target,
            method: Method::Ack,
            in_response_to: Some(in_response_to.into()),
            expects_async: false,
            message_type: message_type.into(),
            message_body,
            status: MessageStatus::Pending,
        }
    }

    /// Create an AsyncResponse message (delayed response).
    ///
    /// - `in_response_to`: id of the Request being responded to
    pub fn async_response(
        source: Actor,
        target: Actor,
        in_response_to: impl Into<String>,
        message_type: impl Into<String>,
        message_body: JsonValue,
    ) -> Self {
        Self {
            id: None,
            source,
            target,
            method: Method::AsyncResponse,
            in_response_to: Some(in_response_to.into()),
            expects_async: false,
            message_type: message_type.into(),
            message_body,
            status: MessageStatus::Pending,
        }
    }

    /// Create a Notify message (one-way notification).
    pub fn notify(
        source: Actor,
        target: Actor,
        message_type: impl Into<String>,
        message_body: JsonValue,
    ) -> Self {
        Self {
            id: None,
            source,
            target,
            method: Method::Notify,
            in_response_to: None,
            expects_async: false,
            message_type: message_type.into(),
            message_body,
            status: MessageStatus::Pending,
        }
    }

    // === Convenience constructors for common patterns ===

    /// Request from ThisSystem to User (ack only).
    pub fn request_to_user(message_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(Actor::ThisSystem, Actor::User, false, message_type, body)
    }

    /// Request from ThisSystem to User (expects async response).
    pub fn request_to_user_async(message_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(Actor::ThisSystem, Actor::User, true, message_type, body)
    }

    /// Request from ThisSystem to ExternalSystem (ack only).
    pub fn request_to_system(message_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(Actor::ThisSystem, Actor::ExternalSystem, false, message_type, body)
    }

    /// Request from ThisSystem to ExternalSystem (expects async response).
    pub fn request_to_system_async(message_type: impl Into<String>, body: JsonValue) -> Self {
        Self::request(Actor::ThisSystem, Actor::ExternalSystem, true, message_type, body)
    }

    /// AsyncResponse from ExternalSystem to ThisSystem.
    pub fn async_response_from_system(
        in_response_to: impl Into<String>,
        message_type: impl Into<String>,
        body: JsonValue,
    ) -> Self {
        Self::async_response(Actor::ExternalSystem, Actor::ThisSystem, in_response_to, message_type, body)
    }

    /// AsyncResponse from User to ThisSystem.
    pub fn async_response_from_user(
        in_response_to: impl Into<String>,
        message_type: impl Into<String>,
        body: JsonValue,
    ) -> Self {
        Self::async_response(Actor::User, Actor::ThisSystem, in_response_to, message_type, body)
    }

    /// Set the id (called by store after insert).
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Mark the message as completed.
    pub fn mark_completed(&mut self) {
        self.status = MessageStatus::Completed;
    }

    /// Check if this message is pending.
    pub fn is_pending(&self) -> bool {
        self.status == MessageStatus::Pending
    }

    /// Check if this is a Request that expects an async response.
    pub fn expects_async_response(&self) -> bool {
        self.method == Method::Request && self.expects_async
    }

    /// Get the id, panics if not set.
    pub fn id(&self) -> &str {
        self.id.as_ref().expect("message id not set")
    }
}
