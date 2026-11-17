//! Decompose a PayerPay bookmark into persistence-friendly snapshots.
//!
//! This function takes a flow bookmark and extracts:
//! - EntitySnapshot: the entity state data
//! - FlowContext: cross-segment locals
//! - EventData: the current event being awaited (with classification + JSON body)
//!
//! The decomposition is pure and can be formally verified.

use crate::context::FlowContext;
use crate::entity::*;
use crate::event_data::*;
use psp_flow::machine::*;
use psp_flow::values::*;

/// Decompose a PayerPay bookmark into its persistence components.
///
/// Returns (EntitySnapshot, FlowContext, EventData).
/// The `seq` parameter is the sequence number to assign to the event.
pub fn decompose(bookmark: &PayerPay, seq: i32) -> (EntitySnapshot, FlowContext, EventData) {
    let ctx = FlowContext::default();

    match bookmark {
        PayerPay::AtGetPaymentIntent(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let cmd = UserCommand::GetPaymentIntent { payer: w.txn.data.payer };
            let event = EventData::user_command(
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
            let event = EventData::user_command(
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
            let event = EventData::user_command(
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
            let event = EventData::system_command(
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
            let event = EventData::user_command(
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
            // AwaitCallback is waiting for SystemAsyncCallback
            // We store it as a system command placeholder (the ReqPay was already sent)
            let event = EventData::new(
                seq,
                Direction::In,
                TargetSource::System,
                InitiatedBy::Them,
                Timing::AsyncCallback,
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
            let event = EventData::user_command(
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
            let event = EventData::user_command(
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
            let event = EventData::user_command(
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
            // AwaitDeemedResolution is waiting for SystemAsyncCallback
            let event = EventData::new(
                seq,
                Direction::In,
                TargetSource::System,
                InitiatedBy::Them,
                Timing::AsyncCallback,
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
            let event = EventData::user_command(
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
            let event = EventData::user_command(
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
            let event = EventData::user_command(
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
            // AwaitForcedReversal is waiting for SystemAsyncCallback
            let event = EventData::new(
                seq,
                Direction::In,
                TargetSource::System,
                InitiatedBy::Them,
                Timing::AsyncCallback,
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
            let event = EventData::user_command(
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
        assert_eq!(event.direction, Direction::Out);
        assert_eq!(event.target_source, TargetSource::User);
        assert_eq!(event.initiated_by, InitiatedBy::Us);
        assert_eq!(event.timing, Timing::Command);
        assert_eq!(event.event_type, "GetPaymentIntent");
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
        assert_eq!(event.event_type, "CollectMpin");
        assert_eq!(event.direction, Direction::Out);
        assert_eq!(event.target_source, TargetSource::User);
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
        assert_eq!(event.event_type, "ReqPay");
        assert_eq!(event.direction, Direction::Out);
        assert_eq!(event.target_source, TargetSource::System);
        assert_eq!(event.timing, Timing::Command);
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
        assert_eq!(event.event_type, "AwaitCallback");
        assert_eq!(event.direction, Direction::In);
        assert_eq!(event.target_source, TargetSource::System);
        assert_eq!(event.timing, Timing::AsyncCallback);
    }
}
