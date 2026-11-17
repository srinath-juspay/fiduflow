//! Adapter to make SeaFlowStore work with the generic Driver.

use crate::{EntitySnapshot, FlowContext, PayFlow, TxnId};
use driver_framework::{FlowStore, MessageRecord, MessageStatus};
use psp_persistence_sea::{SeaFlowStore, StoreError};

/// Wrapper around SeaFlowStore that implements FlowStore<PayFlow>.
pub struct PayFlowStore {
    inner: SeaFlowStore,
}

impl PayFlowStore {
    /// Create a new PayFlowStore wrapping a SeaFlowStore.
    pub fn new(store: SeaFlowStore) -> Self {
        Self { inner: store }
    }

    /// Get a reference to the inner SeaFlowStore.
    pub fn inner(&self) -> &SeaFlowStore {
        &self.inner
    }

    /// Create the database schema.
    pub async fn create_schema(&self) -> Result<(), StoreError> {
        self.inner.create_schema().await
    }
}

impl FlowStore<PayFlow> for PayFlowStore {
    type Error = StoreError;

    async fn save_entity(&self, id: TxnId, entity: &EntitySnapshot) -> Result<(), Self::Error> {
        // Use the domain-specific FlowStore trait from psp-persistence
        psp_persistence::FlowStore::save_entity(&self.inner, id, entity).await
    }

    async fn load_entity(&self, id: TxnId) -> Result<Option<EntitySnapshot>, Self::Error> {
        psp_persistence::FlowStore::load_entity(&self.inner, id).await
    }

    async fn insert_message(
        &self,
        id: TxnId,
        message: MessageRecord,
    ) -> Result<MessageRecord, Self::Error> {
        psp_persistence::FlowStore::insert_message(&self.inner, id, message).await
    }

    async fn update_message_status(
        &self,
        message_id: &str,
        status: MessageStatus,
    ) -> Result<(), Self::Error> {
        psp_persistence::FlowStore::update_message_status(&self.inner, message_id, status).await
    }

    async fn load_messages(&self, id: TxnId) -> Result<Vec<MessageRecord>, Self::Error> {
        psp_persistence::FlowStore::load_messages(&self.inner, id).await
    }

    async fn pending_message(&self, id: TxnId) -> Result<Option<MessageRecord>, Self::Error> {
        psp_persistence::FlowStore::pending_message(&self.inner, id).await
    }

    async fn save_context(&self, id: TxnId, ctx: &FlowContext) -> Result<(), Self::Error> {
        psp_persistence::FlowStore::save_context(&self.inner, id, ctx).await
    }

    async fn load_context(&self, id: TxnId) -> Result<FlowContext, Self::Error> {
        psp_persistence::FlowStore::load_context(&self.inner, id).await
    }

    async fn find_pending(&self) -> Result<Vec<TxnId>, Self::Error> {
        psp_persistence::FlowStore::find_pending(&self.inner).await
    }
}
