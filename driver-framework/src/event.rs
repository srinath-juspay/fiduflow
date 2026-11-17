//! Generic event data structures for flow persistence.
//!
//! These types are domain-agnostic and can be used with any flow.
//! Event classification:
//! - Direction: Out (we send) | In (we receive)
//! - Target/Source: User | System
//! - Initiated by: Us | Them
//! - Timing: Command | Request | Notification | SyncAck | AsyncResponse | AsyncCallback

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

/// Direction of the event relative to our system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Direction {
    /// We send (outgoing)
    Out,
    /// We receive (incoming)
    In,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Out => "Out",
            Direction::In => "In",
        }
    }

    pub fn from_str(s: &str) -> Option<Direction> {
        match s {
            "Out" => Some(Direction::Out),
            "In" => Some(Direction::In),
            _ => None,
        }
    }
}

/// Target (for outgoing) or Source (for incoming).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TargetSource {
    User,
    System,
}

impl TargetSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetSource::User => "User",
            TargetSource::System => "System",
        }
    }

    pub fn from_str(s: &str) -> Option<TargetSource> {
        match s {
            "User" => Some(TargetSource::User),
            "System" => Some(TargetSource::System),
            _ => None,
        }
    }
}

/// Who initiated this event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InitiatedBy {
    /// We initiated
    Us,
    /// They initiated
    Them,
}

impl InitiatedBy {
    pub fn as_str(&self) -> &'static str {
        match self {
            InitiatedBy::Us => "Us",
            InitiatedBy::Them => "Them",
        }
    }

    pub fn from_str(s: &str) -> Option<InitiatedBy> {
        match s {
            "Us" => Some(InitiatedBy::Us),
            "Them" => Some(InitiatedBy::Them),
            _ => None,
        }
    }
}

/// Timing/nature of the event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timing {
    /// Outgoing command (we send, expect response)
    Command,
    /// Incoming request (they initiate, not response to our command)
    Request,
    /// Incoming notification (they notify, unsolicited)
    Notification,
    /// Incoming sync ack (immediate response to our command)
    SyncAck,
    /// Incoming async response (delayed response to our command)
    AsyncResponse,
    /// Incoming async callback (async result from system)
    AsyncCallback,
}

impl Timing {
    pub fn as_str(&self) -> &'static str {
        match self {
            Timing::Command => "Command",
            Timing::Request => "Request",
            Timing::Notification => "Notification",
            Timing::SyncAck => "SyncAck",
            Timing::AsyncResponse => "AsyncResponse",
            Timing::AsyncCallback => "AsyncCallback",
        }
    }

    pub fn from_str(s: &str) -> Option<Timing> {
        match s {
            "Command" => Some(Timing::Command),
            "Request" => Some(Timing::Request),
            "Notification" => Some(Timing::Notification),
            "SyncAck" => Some(Timing::SyncAck),
            "AsyncResponse" => Some(Timing::AsyncResponse),
            "AsyncCallback" => Some(Timing::AsyncCallback),
            _ => None,
        }
    }
}

/// A generic event data record for persistence.
///
/// This structure stores event metadata with classification
/// (direction, target/source, initiated_by, timing) along with
/// the event body as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventData {
    /// Sequence number within the transaction (0-indexed).
    pub seq: i32,

    // === Classification ===
    /// Direction: Out (we send) or In (we receive).
    pub direction: Direction,
    /// Target (if Out) or Source (if In): User or System.
    pub target_source: TargetSource,
    /// Who initiated: Us or Them.
    pub initiated_by: InitiatedBy,
    /// Timing/nature of the event.
    pub timing: Timing,

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
        direction: Direction,
        target_source: TargetSource,
        initiated_by: InitiatedBy,
        timing: Timing,
        event_type: impl Into<String>,
        event_body: JsonValue,
    ) -> Self {
        Self {
            seq,
            direction,
            target_source,
            initiated_by,
            timing,
            event_type: event_type.into(),
            event_body,
            status: EventStatus::Pending,
        }
    }

    /// Create an outgoing user command event.
    pub fn user_command(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::Out,
            TargetSource::User,
            InitiatedBy::Us,
            Timing::Command,
            event_type,
            body,
        )
    }

    /// Create an outgoing system command event.
    pub fn system_command(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::Out,
            TargetSource::System,
            InitiatedBy::Us,
            Timing::Command,
            event_type,
            body,
        )
    }

    /// Create an incoming user request event.
    pub fn user_request(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::In,
            TargetSource::User,
            InitiatedBy::Them,
            Timing::Request,
            event_type,
            body,
        )
    }

    /// Create an incoming user sync ack event.
    pub fn user_sync_ack(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::In,
            TargetSource::User,
            InitiatedBy::Them,
            Timing::SyncAck,
            event_type,
            body,
        )
    }

    /// Create an incoming user async response event.
    pub fn user_async_response(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::In,
            TargetSource::User,
            InitiatedBy::Them,
            Timing::AsyncResponse,
            event_type,
            body,
        )
    }

    /// Create an incoming system sync ack event.
    pub fn system_sync_ack(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::In,
            TargetSource::System,
            InitiatedBy::Them,
            Timing::SyncAck,
            event_type,
            body,
        )
    }

    /// Create an incoming system async callback event.
    pub fn system_async_callback(seq: i32, event_type: impl Into<String>, body: JsonValue) -> Self {
        Self::new(
            seq,
            Direction::In,
            TargetSource::System,
            InitiatedBy::Them,
            Timing::AsyncCallback,
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
