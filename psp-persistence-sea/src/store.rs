//! SeaORM implementation of FlowStore.

use crate::conversions::{
    context_to_model, entity_to_model, event_to_model, model_to_context, model_to_entity,
    model_to_event,
};
use crate::models::{events, flow_context, upi_txn};
use chrono::Utc;
use psp_flow::values::TxnId;
use psp_persistence::context::{FlowContext, Segment};
use psp_persistence::entity::EntitySnapshot;
use psp_persistence::event_data::{EventData, EventResponse, EventStatus};
use psp_persistence::store::FlowStore;
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
    QueryOrder,
};
use thiserror::Error;

/// Errors from the SeaORM store.
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Db(#[from] DbErr),
    #[error("Conversion error: {0}")]
    Conversion(#[from] crate::conversions::ConversionError),
    #[error("Event not found: txn={0}, seq={1}")]
    EventNotFound(u64, i32),
}

/// SeaORM implementation of FlowStore.
pub struct SeaFlowStore {
    db: DatabaseConnection,
}

impl SeaFlowStore {
    /// Create a new store with the given database connection.
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Get the underlying database connection.
    pub fn db(&self) -> &DatabaseConnection {
        &self.db
    }

    /// Create the database schema.
    ///
    /// This should be called once when setting up the database.
    pub async fn create_schema(&self) -> Result<(), StoreError> {
        use sea_orm::ConnectionTrait;

        // Create upi_txn table
        self.db
            .execute_unprepared(
                r#"
            CREATE TABLE IF NOT EXISTS upi_txn (
                id INTEGER PRIMARY KEY,
                state TEXT NOT NULL,
                payer INTEGER NOT NULL,
                payee INTEGER,
                amount INTEGER,
                mpin INTEGER,
                debit_ref INTEGER,
                credit_ref INTEGER,
                failure_reason TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            )
            .await?;

        // Create events table
        self.db
            .execute_unprepared(
                r#"
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                txn_id INTEGER NOT NULL,
                seq INTEGER NOT NULL,
                event_type TEXT NOT NULL,
                status TEXT NOT NULL,
                req_payer INTEGER,
                req_intent_payee INTEGER,
                req_intent_amount INTEGER,
                req_reason TEXT,
                req_status TEXT,
                req_pay_payer INTEGER,
                req_pay_payee INTEGER,
                req_pay_amount INTEGER,
                req_pay_mpin INTEGER,
                req_timeout_secs INTEGER,
                req_deadline_days INTEGER,
                resp_raw_payee INTEGER,
                resp_raw_amount INTEGER,
                resp_mpin INTEGER,
                resp_req_pay_result TEXT,
                resp_reject_reason TEXT,
                resp_callback_type TEXT,
                resp_debit_ref INTEGER,
                resp_credit_ref INTEGER,
                resp_callback_reason TEXT,
                resp_resolution_type TEXT,
                created_at TEXT NOT NULL,
                sent_at TEXT,
                received_at TEXT,
                processed_at TEXT,
                FOREIGN KEY (txn_id) REFERENCES upi_txn(id)
            )
            "#,
            )
            .await?;

        // Create flow_context table
        self.db
            .execute_unprepared(
                r#"
            CREATE TABLE IF NOT EXISTS flow_context (
                txn_id INTEGER PRIMARY KEY,
                FOREIGN KEY (txn_id) REFERENCES upi_txn(id)
            )
            "#,
            )
            .await?;

        // Create indexes
        self.db
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_events_txn_id ON events(txn_id)")
            .await?;
        self.db
            .execute_unprepared(
                "CREATE INDEX IF NOT EXISTS idx_events_txn_seq ON events(txn_id, seq)",
            )
            .await?;
        self.db
            .execute_unprepared("CREATE INDEX IF NOT EXISTS idx_upi_txn_state ON upi_txn(state)")
            .await?;

        Ok(())
    }
}

impl FlowStore for SeaFlowStore {
    type Error = StoreError;

    async fn save_entity(&self, id: TxnId, entity: &EntitySnapshot) -> Result<(), Self::Error> {
        let model = entity_to_model(id, entity);

        // Try to find existing record
        let existing = upi_txn::Entity::find_by_id(id.0 as i64)
            .one(&self.db)
            .await?;

        if existing.is_some() {
            // Update existing record
            let mut update_model = model;
            update_model.created_at = sea_orm::ActiveValue::NotSet; // Don't update created_at
            update_model.updated_at = Set(Utc::now());
            update_model.update(&self.db).await?;
        } else {
            // Insert new record
            model.insert(&self.db).await?;
        }

        Ok(())
    }

    async fn load_entity(&self, id: TxnId) -> Result<Option<EntitySnapshot>, Self::Error> {
        let model = upi_txn::Entity::find_by_id(id.0 as i64)
            .one(&self.db)
            .await?;

        match model {
            Some(m) => Ok(Some(model_to_entity(&m)?)),
            None => Ok(None),
        }
    }

    async fn insert_event(&self, id: TxnId, event: &EventData) -> Result<i32, Self::Error> {
        let model = event_to_model(id, event);
        model.insert(&self.db).await?;
        Ok(event.seq)
    }

    async fn update_event_status(
        &self,
        id: TxnId,
        seq: i32,
        status: EventStatus,
    ) -> Result<(), Self::Error> {
        let event = events::Entity::find()
            .filter(events::Column::TxnId.eq(id.0 as i64))
            .filter(events::Column::Seq.eq(seq))
            .one(&self.db)
            .await?
            .ok_or(StoreError::EventNotFound(id.0, seq))?;

        let now = Utc::now();
        let mut update: events::ActiveModel = event.into();
        update.status = Set(status.as_str().to_string());

        // Set appropriate timestamp
        match status {
            EventStatus::Awaiting => update.sent_at = Set(Some(now)),
            EventStatus::Received => update.received_at = Set(Some(now)),
            EventStatus::Processed => update.processed_at = Set(Some(now)),
            EventStatus::Initialized => {}
        }

        update.update(&self.db).await?;
        Ok(())
    }

    async fn update_event_response(
        &self,
        id: TxnId,
        seq: i32,
        response: &EventResponse,
    ) -> Result<(), Self::Error> {
        let event = events::Entity::find()
            .filter(events::Column::TxnId.eq(id.0 as i64))
            .filter(events::Column::Seq.eq(seq))
            .one(&self.db)
            .await?
            .ok_or(StoreError::EventNotFound(id.0, seq))?;

        let now = Utc::now();
        let mut update: events::ActiveModel = event.into();

        // Set status to Received
        update.status = Set(EventStatus::Received.as_str().to_string());
        update.received_at = Set(Some(now));

        // Set response fields
        if let Some(raw) = response.raw_intent {
            update.resp_raw_payee = Set(Some(raw.payee.0 as i64));
            update.resp_raw_amount = Set(Some(raw.amount.0 as i64));
        }
        if let Some(mpin) = response.mpin {
            update.resp_mpin = Set(Some(mpin.0 as i64));
        }
        if let Some(result) = response.req_pay_result {
            update.resp_req_pay_result = Set(Some(result.as_str().to_string()));
        }
        if let Some(reason) = response.reject_reason {
            update.resp_reject_reason =
                Set(Some(crate::conversions::reason_to_str(reason).to_string()));
        }
        if let Some(cb_type) = response.callback_type {
            update.resp_callback_type = Set(Some(cb_type.as_str().to_string()));
        }
        if let Some(debit_ref) = response.debit_ref {
            update.resp_debit_ref = Set(Some(debit_ref.0 as i64));
        }
        if let Some(credit_ref) = response.credit_ref {
            update.resp_credit_ref = Set(Some(credit_ref.0 as i64));
        }
        if let Some(reason) = response.callback_reason {
            update.resp_callback_reason =
                Set(Some(crate::conversions::reason_to_str(reason).to_string()));
        }
        if let Some(res_type) = response.resolution_type {
            update.resp_resolution_type = Set(Some(res_type.as_str().to_string()));
        }

        update.update(&self.db).await?;
        Ok(())
    }

    async fn load_events(&self, id: TxnId) -> Result<Vec<EventData>, Self::Error> {
        let models = events::Entity::find()
            .filter(events::Column::TxnId.eq(id.0 as i64))
            .order_by_asc(events::Column::Seq)
            .all(&self.db)
            .await?;

        models
            .iter()
            .map(|m| model_to_event(m).map_err(StoreError::from))
            .collect()
    }

    async fn current_event(&self, id: TxnId) -> Result<Option<EventData>, Self::Error> {
        let model = events::Entity::find()
            .filter(events::Column::TxnId.eq(id.0 as i64))
            .filter(events::Column::Status.ne(EventStatus::Processed.as_str()))
            .order_by_asc(events::Column::Seq)
            .one(&self.db)
            .await?;

        match model {
            Some(m) => Ok(Some(model_to_event(&m)?)),
            None => Ok(None),
        }
    }

    async fn save_context(&self, id: TxnId, ctx: &FlowContext) -> Result<(), Self::Error> {
        let model = context_to_model(id, ctx);

        // Try to find existing record
        let existing = flow_context::Entity::find_by_id(id.0 as i64)
            .one(&self.db)
            .await?;

        if existing.is_some() {
            model.update(&self.db).await?;
        } else {
            model.insert(&self.db).await?;
        }

        Ok(())
    }

    async fn load_context(&self, id: TxnId) -> Result<FlowContext, Self::Error> {
        let model = flow_context::Entity::find_by_id(id.0 as i64)
            .one(&self.db)
            .await?;

        Ok(match model {
            Some(m) => model_to_context(&m),
            None => FlowContext::default(),
        })
    }

    async fn find_pending(&self) -> Result<Vec<TxnId>, Self::Error> {
        let terminal_states = ["Success", "Failed", "Reversed", "Abandoned"];

        let models = upi_txn::Entity::find()
            .filter(
                terminal_states
                    .iter()
                    .fold(sea_orm::Condition::all(), |cond, state| {
                        cond.add(upi_txn::Column::State.ne(*state))
                    }),
            )
            .all(&self.db)
            .await?;

        Ok(models.iter().map(|m| TxnId(m.id as u64)).collect())
    }

    async fn find_by_state(&self, state: Segment) -> Result<Vec<TxnId>, Self::Error> {
        let models = upi_txn::Entity::find()
            .filter(upi_txn::Column::State.eq(state.as_str()))
            .all(&self.db)
            .await?;

        Ok(models.iter().map(|m| TxnId(m.id as u64)).collect())
    }
}

#[cfg(all(test, feature = "sqlite"))]
mod tests {
    use super::*;
    use psp_flow::values::*;
    use psp_persistence::entity::InitiatedEntity;
    use psp_persistence::event_data::{EventRequest, EventType};
    use sea_orm::Database;

    async fn setup_test_db() -> SeaFlowStore {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let store = SeaFlowStore::new(db);
        store.create_schema().await.unwrap();
        store
    }

    #[tokio::test]
    async fn test_save_load_entity() {
        let store = setup_test_db().await;
        let id = TxnId(1);
        let entity = EntitySnapshot::Initiated(InitiatedEntity {
            id,
            payer: PayerId(42),
        });

        store.save_entity(id, &entity).await.unwrap();
        let loaded = store.load_entity(id).await.unwrap();

        assert_eq!(loaded, Some(entity));
    }

    #[tokio::test]
    async fn test_insert_load_event() {
        let store = setup_test_db().await;
        let id = TxnId(1);

        // First save the entity (foreign key constraint)
        let entity = EntitySnapshot::Initiated(InitiatedEntity {
            id,
            payer: PayerId(42),
        });
        store.save_entity(id, &entity).await.unwrap();

        let event = EventData::new(
            0,
            EventType::GetPaymentIntent,
            EventRequest::get_payment_intent(PayerId(42)),
        );
        store.insert_event(id, &event).await.unwrap();

        let events = store.load_events(id).await.unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[0].event_type, EventType::GetPaymentIntent);
    }

    #[tokio::test]
    async fn test_find_pending() {
        let store = setup_test_db().await;

        // Create a pending transaction
        let id1 = TxnId(1);
        let entity1 = EntitySnapshot::Initiated(InitiatedEntity {
            id: id1,
            payer: PayerId(42),
        });
        store.save_entity(id1, &entity1).await.unwrap();

        // Create a terminal transaction
        let id2 = TxnId(2);
        let entity2 = EntitySnapshot::Abandoned(psp_persistence::entity::AbandonedEntity {
            id: id2,
            payer: PayerId(43),
        });
        store.save_entity(id2, &entity2).await.unwrap();

        let pending = store.find_pending().await.unwrap();
        assert_eq!(pending, vec![id1]);
    }
}
