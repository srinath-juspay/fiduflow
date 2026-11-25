//! Persistence traits for flows.
//!
//! This module provides:
//! - `Persist`: Trait for decomposing/reconstructing flow state
//! - `FlowStore`: Trait for storing flow state in a database
//! - `Driver`: Generic driver that coordinates flow execution and persistence

use crate::machine::{Flow, Next};
use crate::message::{MessageRecord, MessageStatus};
use core::fmt::Debug;
use std::error::Error;
use std::future::Future;

/// Error type for reconstruction failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructError {
    /// No pending message found in the message list.
    NoPendingMessage,
    /// The entity state and message type combination is invalid.
    InvalidStateMessageCombination {
        state: &'static str,
        message_type: String,
    },
    /// Missing required data from message history.
    MissingMessageData(&'static str),
    /// The entity snapshot is a terminal state.
    TerminalState,
    /// Failed to deserialize JSON.
    JsonError(String),
}

/// Trait for flows that can be persisted.
///
/// This trait defines how to decompose a flow state into storage-friendly
/// components and how to reconstruct the state from those components.
pub trait Persist: Flow {
    /// The entity snapshot type (represents the domain entity state).
    type Entity: Debug;
    /// The context type (cross-segment locals).
    type Context: Debug + Default;

    /// Decompose a flow state into storage components.
    ///
    /// Returns (entity, context, outgoing_message).
    fn decompose(state: &Self::State) -> (Self::Entity, Self::Context, MessageRecord);

    /// Reconstruct a flow state from stored components.
    fn reconstruct(
        entity: &Self::Entity,
        ctx: &Self::Context,
        messages: &[MessageRecord],
    ) -> Result<Self::State, ReconstructError>;

    /// Convert a terminal (Done) value to an entity snapshot.
    fn terminal_entity(done: &Self::Done) -> Self::Entity;

    /// Check if an entity represents a terminal state.
    fn is_terminal(entity: &Self::Entity) -> bool;
}

/// Storage trait for flow persistence.
///
/// This trait defines the async interface that database implementations must provide.
/// It is generic over the `Persist` trait to support different flow types.
pub trait FlowStore<P: Persist>: Send + Sync {
    /// The error type for storage operations.
    type Error: Error + Send + Sync + 'static;

    // ---- Entity operations ----

    /// Save an entity snapshot.
    fn save_entity(
        &self,
        id: P::Id,
        entity: &P::Entity,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load an entity snapshot by ID.
    fn load_entity(
        &self,
        id: P::Id,
    ) -> impl Future<Output = Result<Option<P::Entity>, Self::Error>> + Send;

    // ---- Message operations ----

    /// Insert a new message record.
    /// The store assigns a GUID to the message and returns it.
    fn insert_message(
        &self,
        id: P::Id,
        message: MessageRecord,
    ) -> impl Future<Output = Result<MessageRecord, Self::Error>> + Send;

    /// Update the status of a message by its ID.
    fn update_message_status(
        &self,
        message_id: &str,
        status: MessageStatus,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load all messages for a flow instance, ordered by created_at.
    fn load_messages(
        &self,
        id: P::Id,
    ) -> impl Future<Output = Result<Vec<MessageRecord>, Self::Error>> + Send;

    /// Load the pending message for a flow instance.
    fn pending_message(
        &self,
        id: P::Id,
    ) -> impl Future<Output = Result<Option<MessageRecord>, Self::Error>> + Send;

    // ---- Context operations ----

    /// Save the flow context.
    fn save_context(
        &self,
        id: P::Id,
        ctx: &P::Context,
    ) -> impl Future<Output = Result<(), Self::Error>> + Send;

    /// Load the flow context.
    fn load_context(
        &self,
        id: P::Id,
    ) -> impl Future<Output = Result<P::Context, Self::Error>> + Send;

    // ---- Recovery operations ----

    /// Find all flow instances with pending (non-terminal) state.
    fn find_pending(&self) -> impl Future<Output = Result<Vec<P::Id>, Self::Error>> + Send;
}

/// Result of delivering a response to a flow.
#[derive(Debug)]
pub enum DeliverResult<E> {
    /// Flow moved to a new wait point.
    Wait {
        /// The outgoing message to send.
        outgoing: MessageRecord,
    },
    /// Flow completed with a terminal state.
    Done {
        /// The terminal entity snapshot.
        terminal: E,
    },
}

/// Errors that can occur during driver operations.
#[derive(Debug)]
pub enum DriverError<E: Error> {
    /// Store operation failed.
    Store(E),
    /// Failed to reconstruct flow state.
    Reconstruct(ReconstructError),
    /// Flow instance not found.
    NotFound,
    /// No pending message for this instance.
    NoPendingMessage,
    /// Response was refused by the flow (type mismatch).
    Refused,
}

impl<E: Error> std::fmt::Display for DriverError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverError::Store(e) => write!(f, "Store error: {}", e),
            DriverError::Reconstruct(e) => write!(f, "Reconstruct error: {:?}", e),
            DriverError::NotFound => write!(f, "Flow instance not found"),
            DriverError::NoPendingMessage => write!(f, "No pending message"),
            DriverError::Refused => write!(f, "Response refused (type mismatch)"),
        }
    }
}

impl<E: Error + 'static> Error for DriverError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DriverError::Store(e) => Some(e),
            _ => None,
        }
    }
}

/// Generic flow driver.
///
/// Manages the lifecycle of flow instances:
/// - Starting new flows
/// - Delivering responses
/// - Persisting state
/// - Reconstructing from storage
pub struct Driver<P: Persist, S: FlowStore<P>> {
    store: S,
    _marker: core::marker::PhantomData<P>,
}

impl<P: Persist, S: FlowStore<P>> Driver<P, S> {
    /// Create a new driver with the given store.
    pub fn new(store: S) -> Self {
        Self {
            store,
            _marker: core::marker::PhantomData,
        }
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Start a new flow instance.
    ///
    /// Creates the initial state and returns the first outgoing message.
    pub async fn start(&self, id: P::Id) -> Result<MessageRecord, DriverError<S::Error>> {
        // Create initial state
        let initial = P::initial(id);

        // Decompose into storage components
        let (entity, ctx, message) = P::decompose(&initial);

        // Save to store
        self.store
            .save_entity(id, &entity)
            .await
            .map_err(DriverError::Store)?;
        self.store
            .save_context(id, &ctx)
            .await
            .map_err(DriverError::Store)?;
        let saved_message = self
            .store
            .insert_message(id, message)
            .await
            .map_err(DriverError::Store)?;

        Ok(saved_message)
    }

    /// Deliver a response to a pending message.
    ///
    /// This implements the hybrid event-sourcing pattern:
    /// 1. Load current state and reconstruct the bookmark
    /// 2. Step the flow with the response (using the given config)
    /// 3. Mark the old message as completed
    /// 4. Save the new state and outgoing message
    ///
    /// The config can change between deliver calls, allowing runtime policy changes.
    pub async fn deliver(
        &self,
        id: P::Id,
        response: P::Response,
        config: &P::Config,
    ) -> Result<DeliverResult<P::Entity>, DriverError<S::Error>> {
        // Load current state
        let entity = self
            .store
            .load_entity(id)
            .await
            .map_err(DriverError::Store)?
            .ok_or(DriverError::NotFound)?;

        let ctx = self
            .store
            .load_context(id)
            .await
            .map_err(DriverError::Store)?;

        let messages = self
            .store
            .load_messages(id)
            .await
            .map_err(DriverError::Store)?;

        // Get the pending message
        let pending = self
            .store
            .pending_message(id)
            .await
            .map_err(DriverError::Store)?
            .ok_or(DriverError::NoPendingMessage)?;

        let pending_id = pending.id.clone().expect("pending message should have id");

        // Reconstruct the bookmark
        let bookmark =
            P::reconstruct(&entity, &ctx, &messages).map_err(DriverError::Reconstruct)?;

        // Step the flow with the given config
        let next = P::step(bookmark, response, config).map_err(|_| DriverError::Refused)?;

        // Mark the pending message as completed
        self.store
            .update_message_status(&pending_id, MessageStatus::Completed)
            .await
            .map_err(DriverError::Store)?;

        match next {
            Next::Wait(new_bookmark) => {
                // Decompose the new state
                let (new_entity, new_ctx, new_message) = P::decompose(&new_bookmark);

                // Save the new state
                self.store
                    .save_entity(id, &new_entity)
                    .await
                    .map_err(DriverError::Store)?;
                self.store
                    .save_context(id, &new_ctx)
                    .await
                    .map_err(DriverError::Store)?;
                let saved_message = self
                    .store
                    .insert_message(id, new_message)
                    .await
                    .map_err(DriverError::Store)?;

                Ok(DeliverResult::Wait {
                    outgoing: saved_message,
                })
            }
            Next::Done(terminal) => {
                // Convert terminal to entity
                let terminal_entity = P::terminal_entity(&terminal);

                // Save the terminal state
                self.store
                    .save_entity(id, &terminal_entity)
                    .await
                    .map_err(DriverError::Store)?;

                Ok(DeliverResult::Done {
                    terminal: terminal_entity,
                })
            }
        }
    }

    /// Get the pending message for a flow instance.
    pub async fn pending(
        &self,
        id: P::Id,
    ) -> Result<Option<MessageRecord>, DriverError<S::Error>> {
        self.store
            .pending_message(id)
            .await
            .map_err(DriverError::Store)
    }

    /// Reconstruct the flow state from stored data.
    pub async fn load(&self, id: P::Id) -> Result<Option<P::State>, DriverError<S::Error>> {
        let entity = match self
            .store
            .load_entity(id)
            .await
            .map_err(DriverError::Store)?
        {
            Some(e) => e,
            None => return Ok(None),
        };

        // Terminal states don't have a bookmark
        if P::is_terminal(&entity) {
            return Ok(None);
        }

        let ctx = self
            .store
            .load_context(id)
            .await
            .map_err(DriverError::Store)?;

        let messages = self
            .store
            .load_messages(id)
            .await
            .map_err(DriverError::Store)?;

        let bookmark =
            P::reconstruct(&entity, &ctx, &messages).map_err(DriverError::Reconstruct)?;
        Ok(Some(bookmark))
    }

    /// Check if a flow instance exists.
    pub async fn exists(&self, id: P::Id) -> Result<bool, DriverError<S::Error>> {
        let entity = self
            .store
            .load_entity(id)
            .await
            .map_err(DriverError::Store)?;
        Ok(entity.is_some())
    }

    /// Find all flow instances with pending (non-terminal) state.
    pub async fn find_pending(&self) -> Result<Vec<P::Id>, DriverError<S::Error>> {
        self.store.find_pending().await.map_err(DriverError::Store)
    }
}
