//! Integration tests for the generic Driver with PayFlow.

#[cfg(all(test, feature = "sqlite"))]
mod driver_tests {
    use crate::{
        DeliverResult, Driver, EntitySnapshot, MessageStatus, PayFlow,
        PayerPay, PayerId, PayFlowStore, TxnId,
    };
    use psp_flow::values::{Amount, RawIntent, Vpa};
    use psp_flow::AnyResponse;
    use psp_persistence::Actor;
    use psp_persistence_sea::SeaFlowStore;
    use sea_orm::Database;

    async fn setup_driver() -> Driver<PayFlow, PayFlowStore> {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let sea_store = SeaFlowStore::new(db);
        let store = PayFlowStore::new(sea_store);
        store.create_schema().await.unwrap();
        Driver::new(store)
    }

    #[tokio::test]
    async fn test_start_flow() {
        let driver = setup_driver().await;
        let txn_id = TxnId(1);

        // Start a new flow (at AtInit state)
        let message = driver.start(txn_id).await.unwrap();

        // Verify the outgoing message is Init request
        assert!(message.id.is_some());
        assert_eq!(message.message_type, "Init");
        assert_eq!(message.source, Actor::ThisSystem);
        assert_eq!(message.target, Actor::ThisSystem);
        assert_eq!(message.status, MessageStatus::Pending);

        // Verify the flow exists
        assert!(driver.exists(txn_id).await.unwrap());
    }

    #[tokio::test]
    async fn test_init_then_get_intent() {
        let driver = setup_driver().await;
        let txn_id = TxnId(1);
        let payer = PayerId(42);

        // Start flow
        driver.start(txn_id).await.unwrap();

        // Deliver Init response (payer ID)
        let result = driver.deliver(txn_id, AnyResponse::Init(payer)).await.unwrap();

        // Should move to GetPaymentIntent
        match result {
            DeliverResult::Wait { outgoing } => {
                assert_eq!(outgoing.message_type, "GetPaymentIntent");
                assert_eq!(outgoing.source, Actor::ThisSystem);
                assert_eq!(outgoing.target, Actor::User);
            }
            DeliverResult::Done { .. } => panic!("Expected Wait, got Done"),
        }
    }

    #[tokio::test]
    async fn test_full_init_to_collect_mpin() {
        let driver = setup_driver().await;
        let txn_id = TxnId(1);
        let payer = PayerId(42);

        // Start flow
        driver.start(txn_id).await.unwrap();

        // Init with payer
        driver.deliver(txn_id, AnyResponse::Init(payer)).await.unwrap();

        // Deliver a valid intent
        let raw_intent = RawIntent {
            payee: Vpa(100),
            amount: Amount(5000),
        };
        let result = driver.deliver(txn_id, AnyResponse::Intent(raw_intent)).await.unwrap();

        // Should move to CollectMpin
        match result {
            DeliverResult::Wait { outgoing } => {
                assert_eq!(outgoing.message_type, "CollectMpin");
                assert_eq!(outgoing.source, Actor::ThisSystem);
                assert_eq!(outgoing.target, Actor::User);
            }
            DeliverResult::Done { .. } => panic!("Expected Wait, got Done"),
        }
    }

    #[tokio::test]
    async fn test_abandon_flow() {
        let driver = setup_driver().await;
        let txn_id = TxnId(1);
        let payer = PayerId(42);

        // Start flow and init
        driver.start(txn_id).await.unwrap();
        driver.deliver(txn_id, AnyResponse::Init(payer)).await.unwrap();

        // Deliver invalid intent (payee same as payer) to get to ShowError
        let raw_intent = RawIntent {
            payee: Vpa(42), // Same as payer
            amount: Amount(5000),
        };
        driver.deliver(txn_id, AnyResponse::Intent(raw_intent)).await.unwrap();

        // Acknowledge the error (which abandons the flow)
        let result = driver.deliver(txn_id, AnyResponse::Unit).await.unwrap();

        // Should complete with Abandoned state
        match result {
            DeliverResult::Done { terminal } => {
                assert!(matches!(terminal, EntitySnapshot::Abandoned(_)));
            }
            DeliverResult::Wait { .. } => panic!("Expected Done, got Wait"),
        }

        // No pending message anymore
        let pending = driver.pending(txn_id).await.unwrap();
        assert!(pending.is_none());
    }

    #[tokio::test]
    async fn test_load_reconstructs_bookmark() {
        let driver = setup_driver().await;
        let txn_id = TxnId(1);

        // Start flow
        driver.start(txn_id).await.unwrap();

        // Load and verify
        let bookmark = driver.load(txn_id).await.unwrap();
        assert!(bookmark.is_some());
        assert!(matches!(bookmark.unwrap(), PayerPay::AtInit(_)));
    }

    #[tokio::test]
    async fn test_find_pending() {
        let driver = setup_driver().await;

        // Start two flows
        driver.start(TxnId(1)).await.unwrap();
        driver.start(TxnId(2)).await.unwrap();

        // Both should be pending
        let pending = driver.find_pending().await.unwrap();
        assert_eq!(pending.len(), 2);
        assert!(pending.contains(&TxnId(1)));
        assert!(pending.contains(&TxnId(2)));
    }

    #[tokio::test]
    async fn test_not_found_error() {
        let driver = setup_driver().await;

        // Try to deliver to non-existent transaction
        let result = driver.deliver(TxnId(999), AnyResponse::Unit).await;

        assert!(matches!(result, Err(driver_framework::DriverError::NotFound)));
    }
}
