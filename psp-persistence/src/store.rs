//! Storage trait for flow persistence.
//!
//! This trait defines the async interface that database implementations must provide.
//! It is deliberately backend-agnostic to allow different implementations (SeaORM, SQLx, etc).

use crate::context::{FlowContext, Segment};
use crate::entity::EntitySnapshot;
use crate::message_data::{MessageRecord, MessageStatus};
use psp_flow::values::TxnId;
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
    fn save_entity(
        &self,
        id: TxnId,
        entity: &EntitySnapshot,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load an entity snapshot by transaction ID.
    fn load_entity(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<Option<EntitySnapshot>, Self::Error>> + Send;

    // ---- Message operations ----

    /// Insert a new message record.
    ///
    /// The store assigns a GUID to the message and returns it.
    /// Messages are ordered by their `created_at` timestamp.
    fn insert_message(
        &self,
        txn_id: TxnId,
        message: MessageRecord,
    ) -> impl Future<Output = Result<MessageRecord, Self::Error>> + Send;

    /// Update the status of a message by its ID.
    fn update_message_status(
        &self,
        message_id: &str,
        status: MessageStatus,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load all messages for a transaction, ordered by created_at.
    fn load_messages(
        &self,
        txn_id: TxnId,
    ) -> impl Future<Output = Result<Vec<MessageRecord>, Self::Error>> + Send;

    /// Load the pending message for a transaction.
    fn pending_message(
        &self,
        txn_id: TxnId,
    ) -> impl Future<Output = Result<Option<MessageRecord>, Self::Error>> + Send;

    // ---- Context operations ----

    /// Save the flow context.
    fn save_context(
        &self,
        id: TxnId,
        ctx: &FlowContext,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load the flow context.
    fn load_context(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<FlowContext, Self::Error>> + Send;

    // ---- Recovery operations ----

    /// Find all transactions with pending (non-terminal) state.
    fn find_pending(&self) -> impl Future<Output = Result<Vec<TxnId>, Self::Error>> + Send;

    // ---- Query operations ----

    /// Find all transactions currently in a specific segment.
    fn find_by_state(
        &self,
        state: Segment,
    ) -> impl Future<Output = Result<Vec<TxnId>, Self::Error>> + Send;
}

/// Extension trait for common store operations.
pub trait FlowStoreExt: FlowStore {
    /// Load an entity and its messages, reconstructing the full state.
    fn load_full(
        &self,
        id: TxnId,
    ) -> impl Future<Output = Result<Option<(EntitySnapshot, FlowContext, Vec<MessageRecord>)>, Self::Error>> + Send
    {
        async move {
            let Some(entity) = self.load_entity(id).await? else {
                return Ok(None);
            };
            let ctx = self.load_context(id).await?;
            let messages = self.load_messages(id).await?;
            Ok(Some((entity, ctx, messages)))
        }
    }

    /// Save an entity, context, and new message atomically.
    fn save_full(
        &self,
        id: TxnId,
        entity: &EntitySnapshot,
        ctx: &FlowContext,
        message: MessageRecord,
    ) -> impl Future<Output = Result<MessageRecord, Self::Error>> + Send {
        async move {
            self.save_entity(id, entity).await?;
            self.save_context(id, ctx).await?;
            self.insert_message(id, message).await
        }
    }
}

// Blanket implementation for all FlowStore implementations
impl<T: FlowStore> FlowStoreExt for T {}
