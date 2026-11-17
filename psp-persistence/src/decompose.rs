//! Decompose a PayerPay bookmark into persistence-friendly snapshots.
//!
//! This function takes a flow bookmark and extracts:
//! - EntitySnapshot: the entity state data
//! - FlowContext: cross-segment locals
//! - MessageRecord: the current message being awaited (with classification + JSON body)
//!
//! The decomposition is pure and can be formally verified.
//!
//! Note: The returned MessageRecord has `id` as None (assigned by store on insert)
//! and `in_response_to` as None for await states (filled in by driver).

use crate::context::FlowContext;
use crate::entity::*;
use crate::message_data::*;
use psp_flow::machine::*;
use psp_flow::values::*;

/// Decompose a PayerPay bookmark into its persistence components.
///
/// Returns (EntitySnapshot, FlowContext, MessageRecord).
/// The MessageRecord has `id` = None (assigned by store) and
/// `in_response_to` = None for await states (filled in by driver).
pub fn decompose(bookmark: &PayerPay) -> (EntitySnapshot, FlowContext, MessageRecord) {
    let ctx = FlowContext::default();

    match bookmark {
        PayerPay::AtGetPaymentIntent(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let cmd = UserCommand::GetPaymentIntent { payer: w.txn.data.payer };
            // Request to user, expects async response (user provides intent)
            let message = MessageRecord::request_to_user_async(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtShowError(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let cmd = UserCommand::ShowError { reason: w.reason };
            // Request to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
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
            let message = MessageRecord::request_to_user_async(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
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
            let message = MessageRecord::request_to_system_async(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
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
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtAwaitCallback(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            // Waiting for AsyncResponse from ExternalSystem (payment callback)
            // This is a placeholder for the expected incoming response.
            // `in_response_to` will be filled in by the driver (references the ReqPay request).
            let message = MessageRecord {
                id: None,
                source: Actor::ExternalSystem,
                target: Actor::ThisSystem,
                method: Method::AsyncResponse,
                in_response_to: None, // Filled in by driver
                expects_async: false,
                message_type: "AwaitCallback".to_string(),
                message_body: serde_json::json!({"timeout_secs": CALLBACK_TIMEOUT_SECS}),
                status: MessageStatus::Pending,
            };
            (entity, ctx, message)
        }

        PayerPay::AtNotifySuccess(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Successful };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtNotifyFailed(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Failed(w.reason) };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtNotifyProcessing(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Processing };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtAwaitDeemed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            // Waiting for AsyncResponse from ExternalSystem (deemed resolution)
            // `in_response_to` will be filled in by the driver.
            let message = MessageRecord {
                id: None,
                source: Actor::ExternalSystem,
                target: Actor::ThisSystem,
                method: Method::AsyncResponse,
                in_response_to: None, // Filled in by driver
                expects_async: false,
                message_type: "AwaitDeemedResolution".to_string(),
                message_body: serde_json::json!({"deadline_days": DEEMED_DEADLINE_DAYS}),
                status: MessageStatus::Pending,
            };
            (entity, ctx, message)
        }

        PayerPay::AtNotifyConfirmed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Confirmed };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtNotifyDeemedReversed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Reversed };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtNotifyReversalStart(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::ReversalInProgress };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
        }

        PayerPay::AtAwaitForcedReversal(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            // Waiting for AsyncResponse from ExternalSystem (forced reversal)
            // `in_response_to` will be filled in by the driver.
            let message = MessageRecord {
                id: None,
                source: Actor::ExternalSystem,
                target: Actor::ThisSystem,
                method: Method::AsyncResponse,
                in_response_to: None, // Filled in by driver
                expects_async: false,
                message_type: "AwaitForcedReversal".to_string(),
                message_body: serde_json::json!({"deadline_days": FORCED_REVERSAL_DEADLINE_DAYS}),
                status: MessageStatus::Pending,
            };
            (entity, ctx, message)
        }

        PayerPay::AtNotifyReversed(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let cmd = UserCommand::Notify { status: Status::Reversed };
            // Notify to user, expects ack only
            let message = MessageRecord::request_to_user(
                cmd.event_type(),
                serde_json::to_value(&cmd).unwrap(),
            );
            (entity, ctx, message)
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

        let (entity, ctx, message) = decompose(&bookmark);

        assert!(matches!(
            entity,
            EntitySnapshot::Initiated(e) if e.id == TxnId(1) && e.payer == PayerId(42)
        ));
        assert_eq!(ctx, FlowContext::default());
        assert_eq!(message.id, None); // ID assigned by store
        assert_eq!(message.source, Actor::ThisSystem);
        assert_eq!(message.target, Actor::User);
        assert_eq!(message.method, Method::Request);
        assert!(message.expects_async); // Expects user to provide intent
        assert_eq!(message.message_type, "GetPaymentIntent");
    }

    #[test]
    fn test_decompose_at_collect_mpin() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let intent = ValidIntent {
            payee: Vpa(100),
            amount: Amount(5000),
        };
        let bookmark = PayerPay::AtCollectMpin(AtCollectMpin { txn, intent });

        let (entity, _ctx, message) = decompose(&bookmark);

        assert!(matches!(entity, EntitySnapshot::Initiated(_)));
        assert_eq!(message.message_type, "CollectMpin");
        assert_eq!(message.source, Actor::ThisSystem);
        assert_eq!(message.target, Actor::User);
        assert_eq!(message.method, Method::Request);
        assert!(message.expects_async); // Expects user to provide MPIN
    }

    #[test]
    fn test_decompose_at_req_pay() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let intent = ValidIntent { payee: Vpa(100), amount: Amount(5000) };
        let auth_txn = txn.authenticate(intent, Mpin(1234));
        let req = auth_txn.build_req_pay();
        let bookmark = PayerPay::AtReqPay(AtReqPay { txn: auth_txn, req });

        let (entity, _ctx, message) = decompose(&bookmark);

        assert!(matches!(entity, EntitySnapshot::Authenticated(_)));
        assert_eq!(message.message_type, "ReqPay");
        assert_eq!(message.source, Actor::ThisSystem);
        assert_eq!(message.target, Actor::ExternalSystem);
        assert_eq!(message.method, Method::Request);
        assert!(message.expects_async); // Expects payment callback
    }

    #[test]
    fn test_decompose_at_await_callback() {
        let txn = Txn::new(TxnId(1), PayerId(42));
        let intent = ValidIntent { payee: Vpa(100), amount: Amount(5000) };
        let auth_txn = txn.authenticate(intent, Mpin(1234));
        let req = auth_txn.build_req_pay();
        let sub_txn = auth_txn.submit(req);
        let bookmark = PayerPay::AtAwaitCallback(AtAwaitCallback { txn: sub_txn });

        let (entity, _ctx, message) = decompose(&bookmark);

        assert!(matches!(entity, EntitySnapshot::Submitted(_)));
        assert_eq!(message.message_type, "AwaitCallback");
        assert_eq!(message.source, Actor::ExternalSystem);
        assert_eq!(message.target, Actor::ThisSystem);
        assert_eq!(message.method, Method::AsyncResponse);
        assert_eq!(message.in_response_to, None); // Filled in by driver
    }
}
