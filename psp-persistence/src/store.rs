//! Storage trait for flow persistence.
//!
//! This trait defines the async interface that database implementations must provide.
//! It is deliberately backend-agnostic to allow different implementations (SeaORM, SQLx, etc).

use crate::context::{FlowContext, Segment};
use crate::entity::EntitySnapshot;
use crate::event_data::{EventData, EventResponse, EventStatus};
use psp_flow::values::TxnId;
use serde_json::Value as JsonValue;
use std::error::Error;
use std::future::Future;

/// The main storage trait for flow persistence.
///
/// Implementations must be Send + Sync to allow concurrent access.
/// Each method returns a Future for async operation.
pub trait FlowStore: Send + Sync {
    /// The error type for storage operations.
    type Error: Error + Send + Sync + 'static;

    // ---- Entity operations ----

    /// Save an entity snapshot.
    ///
    /// This should upsert (insert or update) the entity.
    fn save_entity(
        &self,
        id: TxnId,
        entity: &EntitySnapshot,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load an entity snapshot by transaction ID.
    ///
    /// Returns None if the entity doesn't exist.
    fn load_entity(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<Option<EntitySnapshot>, Self::Error>> + Send;

    // ---- Event operations ----

    /// Insert a new event record.
    ///
    /// Returns the sequence number assigned to the event.
    fn insert_event(
        &self,
        id: TxnId,
        event: &EventData,
    ) -> impl Future<Output = Result<i32, Self::Error>> + Send;

    /// Update the status of an event.
    fn update_event_status(
        &self,
        id: TxnId,
        seq: i32,
        status: EventStatus,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Update the response of an event (as JSON).
    ///
    /// This also sets the status to Received.
    fn update_event_response(
        &self,
        id: TxnId,
        seq: i32,
        response: JsonValue,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load all events for a transaction.
    ///
    /// Events should be ordered by sequence number.
    fn load_events(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<Vec<EventData>, Self::Error>> + Send;

    /// Load the current (non-processed) event for a transaction.
    ///
    /// Returns None if all events are processed or no events exist.
    fn current_event(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<Option<EventData>, Self::Error>> + Send;

    // ---- Context operations ----

    /// Save the flow context.
    fn save_context(
        &self,
        id: TxnId,
        ctx: &FlowContext,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load the flow context.
    ///
    /// Returns the default context if none exists.
    fn load_context(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<FlowContext, Self::Error>> + Send;

    // ---- Recovery operations ----

    /// Find all transactions with pending (non-terminal) state.
    ///
    /// Used for crash recovery to resume in-flight transactions.
    fn find_pending(&self) -> impl Future<Output = Result<Vec<TxnId>, Self::Error>> + Send;

    // ---- Query operations ----

    /// Find all transactions currently in a specific segment.
    ///
    /// Used for monitoring and operational queries.
    fn find_by_state(
        &self,
        state: Segment,
    ) -> impl Future<Output = Result<Vec<TxnId>, Self::Error>> + Send;
}

/// Extension trait for common store operations.
pub trait FlowStoreExt: FlowStore {
    /// Load an entity and its events, reconstructing the full state.
    fn load_full(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<Option<(EntitySnapshot, FlowContext, Vec<EventData>)>, Self::Error>> + Send
    {
        async move {
            let Some(entity) = self.load_entity(id).await? else {
                return Ok(None);
            };
            let ctx = self.load_context(id).await?;
            let events = self.load_events(id).await?;
            Ok(Some((entity, ctx, events)))
        }
    }

    /// Save an entity, context, and new event atomically.
    ///
    /// Note: Actual atomicity depends on the implementation.
    fn save_full(
        &self,
        id: TxnId,
        entity: &EntitySnapshot,
        ctx: &FlowContext,
        event: &EventData,
    ) -> impl Future<Output = Result<i32, Self::Error>> + Send {
        async move {
            self.save_entity(id, entity).await?;
            self.save_context(id, ctx).await?;
            self.insert_event(id, event).await
        }
    }

    /// Update event response with typed EventResponse (serializes to JSON).
    fn update_event_response_typed(
        &self,
        id: TxnId,
        seq: i32,
        response: &EventResponse,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send {
        async move {
            let json = serde_json::to_value(response).map_err(|_| {
                // This shouldn't happen with valid EventResponse
                panic!("Failed to serialize EventResponse")
            })?;
            self.update_event_response(id, seq, json).await
        }
    }
}

// Blanket implementation for all FlowStore implementations
impl<T: FlowStore> FlowStoreExt for T {}
