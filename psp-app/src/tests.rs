//! Integration tests for the generic Driver with PayFlow.

#[cfg(all(test, feature = "sqlite"))]
mod driver_tests {
    use crate::{
        DeliverResult, Driver, EntitySnapshot, FlowConfig, MessageStatus, PayFlow, PayerPay, PayerId,
        PayFlowStore, TxnId,
    };
    use psp_flow::values::{Amount, RawIntent, Vpa};
    use psp_flow::AnyResponse;
    use psp_persistence::Actor;
    use psp_persistence_sea::SeaFlowStore;
    use sea_orm::Database;

    // Default config for tests
    fn cfg() -> FlowConfig { FlowConfig::default() }

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
        let result = driver
            .deliver(txn_id, AnyResponse::Init(payer), &cfg())
            .await
            .unwrap();

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
        driver
            .deliver(txn_id, AnyResponse::Init(payer), &cfg())
            .await
            .unwrap();

        // Deliver a valid intent
        let raw_intent = RawIntent {
            payee: Vpa(100),
            amount: Amount(5000),
        };
        let result = driver
            .deliver(txn_id, AnyResponse::Intent(raw_intent), &cfg())
            .await
            .unwrap();

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
        driver
            .deliver(txn_id, AnyResponse::Init(payer), &cfg())
            .await
            .unwrap();

        // Deliver invalid intent (payee same as payer) to get to ShowError
        let raw_intent = RawIntent {
            payee: Vpa(42), // Same as payer
            amount: Amount(5000),
        };
        driver
            .deliver(txn_id, AnyResponse::Intent(raw_intent), &cfg())
            .await
            .unwrap();

        // Acknowledge the error (which abandons the flow)
        let result = driver
            .deliver(txn_id, AnyResponse::Unit, &cfg())
            .await
            .unwrap();

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
        let result = driver.deliver(TxnId(999), AnyResponse::Unit, &cfg()).await;

        assert!(matches!(
            result,
            Err(driver_framework::DriverError::NotFound)
        ));
    }
}

/// Recovery tests: simulate crashes at various points and verify correct recovery.
///
/// These tests verify that:
/// 1. `find_pending()` correctly identifies in-flight transactions after "restart"
/// 2. `load()` correctly reconstructs the flow state from storage
/// 3. After recovery, `deliver()` continues the flow correctly
///
/// We simulate a "crash" by creating a new Driver with the same store,
/// which mirrors what happens when an application restarts.
#[cfg(all(test, feature = "sqlite"))]
mod recovery_tests {
    use crate::{
        DeliverResult, Driver, EntitySnapshot, FlowConfig, PayFlow, PayFlowStore, PayerPay, PayerId, TxnId,
    };
    use psp_flow::values::{Amount, Mpin, RawIntent, Vpa};
    use psp_flow::AnyResponse;
    use psp_persistence_sea::SeaFlowStore;
    use sea_orm::Database;

    // Default config for tests
    fn cfg() -> FlowConfig { FlowConfig::default() }

    // For recovery tests, we use a file-based database so we can
    // create multiple connections to simulate application restarts.

    async fn setup_file_db() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let db_path = format!("/tmp/fiduflow_test_{}.db", timestamp);

        // Create initial schema
        let db = Database::connect(format!("sqlite:{}?mode=rwc", db_path))
            .await
            .unwrap();
        let sea_store = SeaFlowStore::new(db);
        let store = PayFlowStore::new(sea_store);
        store.create_schema().await.unwrap();
        // Drop connection
        drop(store);

        db_path
    }

    async fn connect_driver(db_path: &str) -> Driver<PayFlow, PayFlowStore> {
        let db = Database::connect(format!("sqlite:{}", db_path))
            .await
            .unwrap();
        let sea_store = SeaFlowStore::new(db);
        let store = PayFlowStore::new(sea_store);
        Driver::new(store)
    }

    async fn cleanup_db(db_path: &str) {
        let _ = std::fs::remove_file(db_path);
    }

    // ---- Recovery Tests ----

    #[tokio::test]
    async fn test_recover_at_init() {
        let db_path = setup_file_db().await;

        // Phase 1: Start a flow, then "crash"
        {
            let driver = connect_driver(&db_path).await;
            driver.start(TxnId(1)).await.unwrap();
            // Simulate crash: driver goes out of scope
        }

        // Phase 2: "Restart" and recover
        {
            let driver = connect_driver(&db_path).await;

            // Find pending transactions
            let pending = driver.find_pending().await.unwrap();
            assert_eq!(pending, vec![TxnId(1)]);

            // Load and verify state
            let bookmark = driver.load(TxnId(1)).await.unwrap();
            assert!(bookmark.is_some());
            assert!(matches!(bookmark.unwrap(), PayerPay::AtInit(_)));

            // Continue the flow
            let result = driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(42)), &cfg())
                .await
                .unwrap();
            match result {
                DeliverResult::Wait { outgoing } => {
                    assert_eq!(outgoing.message_type, "GetPaymentIntent");
                }
                _ => panic!("Expected Wait"),
            }
        }

        cleanup_db(&db_path).await;
    }

    #[tokio::test]
    async fn test_recover_at_get_payment_intent() {
        let db_path = setup_file_db().await;

        // Phase 1: Start flow, deliver init, then "crash"
        {
            let driver = connect_driver(&db_path).await;
            driver.start(TxnId(1)).await.unwrap();
            driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(42)), &cfg())
                .await
                .unwrap();
            // Crash at GetPaymentIntent
        }

        // Phase 2: Recover and continue
        {
            let driver = connect_driver(&db_path).await;

            // Find pending
            let pending = driver.find_pending().await.unwrap();
            assert_eq!(pending, vec![TxnId(1)]);

            // Load and verify state
            let bookmark = driver.load(TxnId(1)).await.unwrap();
            assert!(bookmark.is_some());
            assert!(matches!(
                bookmark.unwrap(),
                PayerPay::AtGetPaymentIntent(_)
            ));

            // Continue with intent
            let intent = RawIntent {
                payee: Vpa(100),
                amount: Amount(5000),
            };
            let result = driver
                .deliver(TxnId(1), AnyResponse::Intent(intent), &cfg())
                .await
                .unwrap();
            match result {
                DeliverResult::Wait { outgoing } => {
                    assert_eq!(outgoing.message_type, "CollectMpin");
                }
                _ => panic!("Expected Wait"),
            }
        }

        cleanup_db(&db_path).await;
    }

    #[tokio::test]
    async fn test_recover_at_collect_mpin() {
        let db_path = setup_file_db().await;

        // Phase 1: Progress to CollectMpin, then "crash"
        {
            let driver = connect_driver(&db_path).await;
            driver.start(TxnId(1)).await.unwrap();
            driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(42)), &cfg())
                .await
                .unwrap();
            let intent = RawIntent {
                payee: Vpa(100),
                amount: Amount(5000),
            };
            driver
                .deliver(TxnId(1), AnyResponse::Intent(intent), &cfg())
                .await
                .unwrap();
            // Crash at CollectMpin
        }

        // Phase 2: Recover and continue
        {
            let driver = connect_driver(&db_path).await;

            // Verify state
            let bookmark = driver.load(TxnId(1)).await.unwrap();
            assert!(bookmark.is_some());
            match bookmark.unwrap() {
                PayerPay::AtCollectMpin(w) => {
                    // Verify the intent was preserved
                    assert_eq!(w.intent.payee.0, 100);
                    assert_eq!(w.intent.amount.0, 5000);
                }
                other => panic!("Expected AtCollectMpin, got {:?}", other),
            }

            // Continue with MPIN
            let result = driver
                .deliver(TxnId(1), AnyResponse::Mpin(Mpin(1234)), &cfg())
                .await
                .unwrap();
            match result {
                DeliverResult::Wait { outgoing } => {
                    assert_eq!(outgoing.message_type, "ReqPay");
                }
                _ => panic!("Expected Wait"),
            }
        }

        cleanup_db(&db_path).await;
    }

    #[tokio::test]
    async fn test_recover_at_show_error() {
        let db_path = setup_file_db().await;

        // Phase 1: Progress to ShowError (invalid intent), then "crash"
        {
            let driver = connect_driver(&db_path).await;
            driver.start(TxnId(1)).await.unwrap();
            driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(42)), &cfg())
                .await
                .unwrap();
            // Invalid intent: payee same as payer
            let intent = RawIntent {
                payee: Vpa(42),
                amount: Amount(5000),
            };
            driver
                .deliver(TxnId(1), AnyResponse::Intent(intent), &cfg())
                .await
                .unwrap();
            // Crash at ShowError
        }

        // Phase 2: Recover and complete
        {
            let driver = connect_driver(&db_path).await;

            // Verify state
            let bookmark = driver.load(TxnId(1)).await.unwrap();
            assert!(bookmark.is_some());
            assert!(matches!(bookmark.unwrap(), PayerPay::AtShowError(_)));

            // Acknowledge error to complete flow
            let result = driver
                .deliver(TxnId(1), AnyResponse::Unit, &cfg())
                .await
                .unwrap();
            match result {
                DeliverResult::Done { terminal } => {
                    assert!(matches!(terminal, EntitySnapshot::Abandoned(_)));
                }
                _ => panic!("Expected Done"),
            }

            // Flow should no longer be pending
            let pending = driver.find_pending().await.unwrap();
            assert!(pending.is_empty());
        }

        cleanup_db(&db_path).await;
    }

    #[tokio::test]
    async fn test_recover_multiple_transactions() {
        let db_path = setup_file_db().await;

        // Phase 1: Start multiple transactions at different states
        {
            let driver = connect_driver(&db_path).await;

            // Txn 1: At Init
            driver.start(TxnId(1)).await.unwrap();

            // Txn 2: At GetPaymentIntent
            driver.start(TxnId(2)).await.unwrap();
            driver
                .deliver(TxnId(2), AnyResponse::Init(PayerId(100)), &cfg())
                .await
                .unwrap();

            // Txn 3: At CollectMpin
            driver.start(TxnId(3)).await.unwrap();
            driver
                .deliver(TxnId(3), AnyResponse::Init(PayerId(200)), &cfg())
                .await
                .unwrap();
            let intent = RawIntent {
                payee: Vpa(300),
                amount: Amount(10000),
            };
            driver
                .deliver(TxnId(3), AnyResponse::Intent(intent), &cfg())
                .await
                .unwrap();

            // Crash with 3 transactions in flight
        }

        // Phase 2: Recover all transactions
        {
            let driver = connect_driver(&db_path).await;

            // Find all pending
            let mut pending = driver.find_pending().await.unwrap();
            pending.sort_by_key(|id| id.0);
            assert_eq!(pending, vec![TxnId(1), TxnId(2), TxnId(3)]);

            // Verify each state
            let b1 = driver.load(TxnId(1)).await.unwrap().unwrap();
            assert!(matches!(b1, PayerPay::AtInit(_)));

            let b2 = driver.load(TxnId(2)).await.unwrap().unwrap();
            assert!(matches!(b2, PayerPay::AtGetPaymentIntent(_)));

            let b3 = driver.load(TxnId(3)).await.unwrap().unwrap();
            assert!(matches!(b3, PayerPay::AtCollectMpin(_)));

            // Continue each transaction
            driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(1)), &cfg())
                .await
                .unwrap();
            driver
                .deliver(TxnId(2), AnyResponse::Intent(RawIntent {
                    payee: Vpa(101),
                    amount: Amount(1000),
                }), &cfg())
                .await
                .unwrap();
            driver
                .deliver(TxnId(3), AnyResponse::Mpin(Mpin(9999)), &cfg())
                .await
                .unwrap();

            // All still pending but advanced
            let b1 = driver.load(TxnId(1)).await.unwrap().unwrap();
            assert!(matches!(b1, PayerPay::AtGetPaymentIntent(_)));

            let b2 = driver.load(TxnId(2)).await.unwrap().unwrap();
            assert!(matches!(b2, PayerPay::AtCollectMpin(_)));

            let b3 = driver.load(TxnId(3)).await.unwrap().unwrap();
            assert!(matches!(b3, PayerPay::AtReqPay(_)));
        }

        cleanup_db(&db_path).await;
    }

    #[tokio::test]
    async fn test_terminal_state_not_in_pending() {
        let db_path = setup_file_db().await;

        // Phase 1: Complete a flow to terminal state
        {
            let driver = connect_driver(&db_path).await;

            // Start and complete with error (abandon)
            driver.start(TxnId(1)).await.unwrap();
            driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(42)), &cfg())
                .await
                .unwrap();
            let intent = RawIntent {
                payee: Vpa(42), // Same as payer - triggers error
                amount: Amount(5000),
            };
            driver
                .deliver(TxnId(1), AnyResponse::Intent(intent), &cfg())
                .await
                .unwrap();
            driver
                .deliver(TxnId(1), AnyResponse::Unit, &cfg())
                .await
                .unwrap();

            // Start another flow but don't complete it
            driver.start(TxnId(2)).await.unwrap();
        }

        // Phase 2: Verify only non-terminal transaction is pending
        {
            let driver = connect_driver(&db_path).await;

            let pending = driver.find_pending().await.unwrap();
            assert_eq!(pending, vec![TxnId(2)]);

            // load() returns None for terminal states
            let b1 = driver.load(TxnId(1)).await.unwrap();
            assert!(b1.is_none());

            // But the entity still exists
            assert!(driver.exists(TxnId(1)).await.unwrap());
        }

        cleanup_db(&db_path).await;
    }

    #[tokio::test]
    async fn test_message_history_preserved_across_restart() {
        let db_path = setup_file_db().await;

        // Phase 1: Progress through several states
        {
            let driver = connect_driver(&db_path).await;
            driver.start(TxnId(1)).await.unwrap();
            driver
                .deliver(TxnId(1), AnyResponse::Init(PayerId(42)), &cfg())
                .await
                .unwrap();
            let intent = RawIntent {
                payee: Vpa(100),
                amount: Amount(5000),
            };
            driver
                .deliver(TxnId(1), AnyResponse::Intent(intent), &cfg())
                .await
                .unwrap();
            // Crash at CollectMpin
        }

        // Phase 2: Verify message history is complete
        {
            let driver = connect_driver(&db_path).await;

            // Access the store to check messages directly
            use psp_persistence::FlowStore;
            let messages = driver.store().inner().load_messages(TxnId(1)).await.unwrap();

            // Should have: Init (completed), GetPaymentIntent (completed), CollectMpin (pending)
            assert_eq!(messages.len(), 3);

            // First two should be completed
            use crate::MessageStatus;
            assert_eq!(messages[0].status, MessageStatus::Completed);
            assert_eq!(messages[0].message_type, "Init");

            assert_eq!(messages[1].status, MessageStatus::Completed);
            assert_eq!(messages[1].message_type, "GetPaymentIntent");

            assert_eq!(messages[2].status, MessageStatus::Pending);
            assert_eq!(messages[2].message_type, "CollectMpin");
        }

        cleanup_db(&db_path).await;
    }
}
