//! Decompose a PayerPay bookmark into persistence-friendly snapshots.
//!
//! This function takes a flow bookmark and extracts:
//! - EntitySnapshot: the entity state data
//! - FlowContext: cross-segment locals
//! - EventData: the current event being awaited (with JSON request body)
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
            let request = GetPaymentIntentRequest { payer: w.txn.data.payer };
            let event = EventData::new(
                seq,
                EventType::GetPaymentIntent.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtShowError(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let request = ShowErrorRequest { reason: w.reason };
            let event = EventData::new(
                seq,
                EventType::ShowError.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtCollectMpin(w) => {
            let entity = EntitySnapshot::Initiated(InitiatedEntity {
                id: w.txn.id,
                payer: w.txn.data.payer,
            });
            let request = CollectMpinRequest {
                payer: w.txn.data.payer,
                intent: w.intent,
            };
            let event = EventData::new(
                seq,
                EventType::CollectMpin.as_str(),
                serde_json::to_value(&request).unwrap(),
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
            let request = ReqPayRequest { req: w.req };
            let event = EventData::new(
                seq,
                EventType::ReqPay.as_str(),
                serde_json::to_value(&request).unwrap(),
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
            let request = NotifyRequest { status: Status::Failed(w.reason) };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtAwaitCallback(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = AwaitCallbackRequest { timeout_secs: CALLBACK_TIMEOUT_SECS };
            let event = EventData::new(
                seq,
                EventType::AwaitCallback.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifySuccess(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::Successful };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyFailed(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::Failed(w.reason) };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyProcessing(w) => {
            let entity = EntitySnapshot::Submitted(SubmittedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::Processing };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtAwaitDeemed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = AwaitDeemedResolutionRequest { deadline_days: DEEMED_DEADLINE_DAYS };
            let event = EventData::new(
                seq,
                EventType::AwaitDeemedResolution.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyConfirmed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::Confirmed };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyDeemedReversed(w) => {
            let entity = EntitySnapshot::Deemed(DeemedEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::Reversed };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyReversalStart(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::ReversalInProgress };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtAwaitForcedReversal(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = AwaitForcedReversalRequest { deadline_days: FORCED_REVERSAL_DEADLINE_DAYS };
            let event = EventData::new(
                seq,
                EventType::AwaitForcedReversal.as_str(),
                serde_json::to_value(&request).unwrap(),
            );
            (entity, ctx, event)
        }

        PayerPay::AtNotifyReversed(w) => {
            let entity = EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id: w.txn.id,
                req: w.txn.data.req,
            });
            let request = NotifyRequest { status: Status::Reversed };
            let event = EventData::new(
                seq,
                EventType::Notify.as_str(),
                serde_json::to_value(&request).unwrap(),
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
        assert_eq!(event.event_type, "GetPaymentIntent");
        assert_eq!(event.status, EventStatus::Awaiting);

        // Verify JSON contains payer
        let req: GetPaymentIntentRequest = serde_json::from_value(event.request_body).unwrap();
        assert_eq!(req.payer, PayerId(42));
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

        // Verify JSON contains intent
        let req: CollectMpinRequest = serde_json::from_value(event.request_body).unwrap();
        assert_eq!(req.intent, intent);
        assert_eq!(req.payer, PayerId(42));
    }
}
