//! Decompose a PayerPay bookmark into persistence-friendly snapshots.
//!
//! This function takes a flow bookmark and extracts:
//! - EntitySnapshot: the entity state data
//! - FlowContext: cross-segment locals
//! - MessageRecord: the current message being awaited (with classification + JSON body)
//!
//! The decomposition is pure and can be formally verified.

use crate::context::FlowContext;
use crate::entity::*;
use crate::message_data::*;
use psp_flow::machine::*;
use psp_flow::values::*;

/// Decompose a PayerPay bookmark into its persistence components.
///
/// Returns (EntitySnapshot, FlowContext, MessageRecord).
/// The `seq` parameter is the sequence number to assign to the message.
pub fn decompose(bookmark: &PayerPay, seq: i32) -> (EntitySnapshot, FlowContext, MessageRecord) {
    let ctx = FlowContext::default();

    match bookmark {
        PayerPay::AtGetPaymentIntent(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let cmd = UserCommand::GetPaymentIntent { payer: w.txn.data.payer };
            // Request to user, expects async response (user provides intent)
            let event = MessageRecord::request_to_user_async(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtShowError(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let cmd = UserCommand::ShowError { reason: w.reason };
            // Request to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtCollectMpin(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let cmd = UserCommand::CollectMpin {
                payer: w.txn.data.payer,
                intent: w.intent,
            };
            // Request to user, expects async response (user provides MPIN)
            let event = MessageRecord::request_to_user_async(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtReqPay(w) => {
            let entity = EntitySnapshot::Authenticated(AuthenticatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
                intent: w.txn.data.intent,
                mpin: w.txn.data.mpin,
            });
            let cmd = SystemCommand::ReqPay { req: w.req };
            // Request to external system, expects async response (payment callback)
            let event = MessageRecord::request_to_system_async(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyRejected(w) => {
            let entity = EntitySnapshot::Authenticated(AuthenticatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
                intent: w.txn.data.intent,
                mpin: w.txn.data.mpin,
            });
            let cmd = UserCommand::Notify { status: Status::Failed(w.reason) };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtAwaitCallback(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            // Waiting for AsyncResponse from ExternalSystem (payment callback)
            // This is a placeholder for the expected incoming response
            let event = MessageRecord::async_response(
                seq,
                Actor::ExternalSystem,
                Actor::ThisSystem,
                seq - 1, // References the previous ReqPay request
                "AwaitCallback",
                serde_json::json!({"timeout_secs": CALLBACK_TIMEOUT_SECS}),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifySuccess(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Successful };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyFailed(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Failed(w.reason) };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyProcessing(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Processing };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtAwaitDeemed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            // Waiting for AsyncResponse from ExternalSystem (deemed resolution)
            let event = MessageRecord::async_response(
                seq,
                Actor::ExternalSystem,
                Actor::ThisSystem,
                seq - 1, // References the previous request
                "AwaitDeemedResolution",
                serde_json::json!({"deadline_days": DEEMED_DEADLINE_DAYS}),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyConfirmed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Confirmed };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyDeemedReversed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Reversed };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyReversalStart(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::ReversalInProgress };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtAwaitForcedReversal(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            // Waiting for AsyncResponse from ExternalSystem (forced reversal)
            let event = MessageRecord::async_response(
                seq,
                Actor::ExternalSystem,
                Actor::ThisSystem,
                seq - 1, // References the previous request
                "AwaitForcedReversal",
                serde_json::json!({"deadline_days": FORCED_REVERSAL_DEADLINE_DAYS}),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyReversed(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Reversed };
            // Notify to user, expects ack only
            let event = MessageRecord::request_to_user(
                seq,
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, event)
        }
    }
}

/// Get the wait point name for a PayerPay bookmark.
pub fn wait_point_name(bookmark: &PayerPay) -> &'static str {
    match bookmark {
        PayerPay::AtGetPaymentIntent(_) => "AtGetPaymentIntent",
        PayerPay::AtShowError(_) => "AtShowError",
        PayerPay::AtCollectMpin(_) => "AtCollectMpin",
        PayerPay::AtReqPay(_) => "AtReqPay",
        PayerPay::AtNotifyRejected(_) => "AtNotifyRejected",
        PayerPay::AtAwaitCallback(_) => "AtAwaitCallback",
        PayerPay::AtNotifySuccess(_) => "AtNotifySuccess",
        PayerPay::AtNotifyFailed(_) => "AtNotifyFailed",
        PayerPay::AtNotifyProcessing(_) => "AtNotifyProcessing",
        PayerPay::AtAwaitDeemed(_) => "AtAwaitDeemed",
        PayerPay::AtNotifyConfirmed(_) => "AtNotifyConfirmed",
        PayerPay::AtNotifyDeemedReversed(_) => "AtNotifyDeemedReversed",
        PayerPay::AtNotifyReversalStart(_) => "AtNotifyReversalStart",
        PayerPay::AtAwaitForcedReversal(_) => "AtAwaitForcedReversal",
        PayerPay::AtNotifyReversed(_) => "AtNotifyReversed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use psp_flow::entity::Txn;

    #[test]
    fn test_decompose_at_get_payment_intent() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let bookmark = PayerPay::AtGetPaymentIntent(AtGetPaymentIntent { txn });

        let (entity, ctx, event) = decompose(&bookmark, 0);

        assert!(matches!(
            entity,
            EntitySnapshot::Initiated(e) if e.id == TxnId(1) && e.payer == PayerId(42)
        ));
        assert_eq!(ctx, FlowContext::default());
        assert_eq!(event.seq, 0);
        assert_eq!(event.source, Actor::ThisSystem);
        assert_eq!(event.target, Actor::User);
        assert_eq!(event.method, Method::Request);
        assert!(event.expects_async); // Expects user to provide intent
        assert_eq!(event.message_type, "GetPaymentIntent");
    }

    #[test]
    fn test_decompose_at_collect_mpin() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let intent = ValidIntent {
            payee: Vpa(100),
            amount: Amount(5000),
        };
        let bookmark = PayerPay::AtCollectMpin(AtCollectMpin { txn, intent });

        let (entity, _ctx, event) = decompose(&bookmark, 1);

        assert!(matches!(entity, EntitySnapshot::Initiated(_)));
        assert_eq!(event.seq, 1);
        assert_eq!(event.message_type, "CollectMpin");
        assert_eq!(event.source, Actor::ThisSystem);
        assert_eq!(event.target, Actor::User);
        assert_eq!(event.method, Method::Request);
        assert!(event.expects_async); // Expects user to provide MPIN
    }

    #[test]
    fn test_decompose_at_req_pay() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let intent = ValidIntent { payee: Vpa(100), amount: Amount(5000) };
        let auth_txn = txn.authenticate(intent, Mpin(1234));
        let req = auth_txn.build_req_pay();
        let bookmark = PayerPay::AtReqPay(AtReqPay { txn: auth_txn, req });

        let (entity, _ctx, event) = decompose(&bookmark, 2);

        assert!(matches!(entity, EntitySnapshot::Authenticated(_)));
        assert_eq!(event.message_type, "ReqPay");
        assert_eq!(event.source, Actor::ThisSystem);
        assert_eq!(event.target, Actor::ExternalSystem);
        assert_eq!(event.method, Method::Request);
        assert!(event.expects_async); // Expects payment callback
    }

    #[test]
    fn test_decompose_at_await_callback() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let intent = ValidIntent { payee: Vpa(100), amount: Amount(5000) };
        let auth_txn = txn.authenticate(intent, Mpin(1234));
        let req = auth_txn.build_req_pay();
        let sub_txn = auth_txn.submit(req);
        let bookmark = PayerPay::AtAwaitCallback(AtAwaitCallback { txn: sub_txn });

        let (entity, _ctx, event) = decompose(&bookmark, 3);

        assert!(matches!(entity, EntitySnapshot::Submitted(_)));
        assert_eq!(event.message_type, "AwaitCallback");
        assert_eq!(event.source, Actor::ExternalSystem);
        assert_eq!(event.target, Actor::ThisSystem);
        assert_eq!(event.method, Method::AsyncResponse);
        assert_eq!(event.in_response_to, Some(2)); // References seq 2 (ReqPay)
    }
}
