//! Reconstruct a PayerPay bookmark from persistence snapshots.
//!
//! This function takes an entity snapshot, context, and event history and
//! reconstructs the original PayerPay bookmark. The reconstruction is pure
//! and can be formally verified.

use crate::context::FlowContext;
use crate::entity::*;
use crate::event_data::*;
use psp_flow::entity::*;
use psp_flow::machine::*;
use psp_flow::values::*;

/// Errors that can occur during reconstruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructError {
    /// No pending event found in the event list.
    NoPendingEvent,
    /// The entity state and event type combination is invalid.
    InvalidStateEventCombination { state: &'static str, event_type: String },
    /// Missing required data from event history.
    MissingEventData(&'static str),
    /// The entity snapshot is a terminal state.
    TerminalState,
    /// Failed to deserialize JSON.
    JsonError(String),
}

/// Find the pending event from the event list.
fn find_pending_event(events: &[EventData]) -> Option<&EventData> {
    events.iter().find(|e| e.is_pending())
}

/// Extract success refs from previous system async response.
fn extract_success_refs(events: &[EventData]) -> Result<(Ref, Ref), ReconstructError> {
    for event in events {
        // Look for ExternalSystem → ThisSystem async response
        if event.method == Method::AsyncResponse && event.source == Actor::ExternalSystem {
            if let Ok(callback) = serde_json::from_value::<SystemAsyncCallback>(event.event_body.clone()) {
                if let SystemAsyncCallback::PaymentSuccess { debit_ref, credit_ref } = callback {
                    return Ok((debit_ref, credit_ref));
                }
            }
        }
    }
    Err(ReconstructError::MissingEventData("success refs"))
}

/// Extract credit_ref from deemed resolution callback.
fn extract_confirmed_credit_ref(events: &[EventData]) -> Result<Ref, ReconstructError> {
    for event in events {
        // Look for ExternalSystem → ThisSystem async response
        if event.method == Method::AsyncResponse && event.source == Actor::ExternalSystem {
            if let Ok(callback) = serde_json::from_value::<SystemAsyncCallback>(event.event_body.clone()) {
                if let SystemAsyncCallback::DeemedSuccess { credit_ref } = callback {
                    return Ok(credit_ref);
                }
            }
        }
    }
    Err(ReconstructError::MissingEventData("confirmed credit_ref"))
}

/// Reconstruct a PayerPay bookmark from persistence snapshots.
pub fn reconstruct(
    entity: &EntitySnapshot,
    _ctx: &FlowContext,
    events: &[EventData],
) -> Result<PayerPay, ReconstructError> {
    if entity.is_terminal() {
        return Err(ReconstructError::TerminalState);
    }

    let current = find_pending_event(events).ok_or(ReconstructError::NoPendingEvent)?;
    let event_type = current.event_type.as_str();

    match (entity, event_type) {
        // Initiated state combinations
        (EntitySnapshot::Initiated(e), "GetPaymentIntent") => {
            let txn = Txn {
                id: e.id,
                data: Initiated { payer: e.payer },
            };
            Ok(PayerPay::AtGetPaymentIntent(AtGetPaymentIntent { txn }))
        }

        (EntitySnapshot::Initiated(e), "ShowError") => {
            let txn = Txn {
                id: e.id,
                data: Initiated { payer: e.payer },
            };
            let cmd: UserCommand = serde_json::from_value(current.event_body.clone())
                .map_err(|e| ReconstructError::JsonError(e.to_string()))?;
            let reason = match cmd {
                UserCommand::ShowError { reason } => reason,
                _ => return Err(ReconstructError::MissingEventData("reason")),
            };
            Ok(PayerPay::AtShowError(AtShowError { txn, reason }))
        }

        (EntitySnapshot::Initiated(e), "CollectMpin") => {
            let txn = Txn {
                id: e.id,
                data: Initiated { payer: e.payer },
            };
            let cmd: UserCommand = serde_json::from_value(current.event_body.clone())
                .map_err(|e| ReconstructError::JsonError(e.to_string()))?;
            let intent = match cmd {
                UserCommand::CollectMpin { intent, .. } => intent,
                _ => return Err(ReconstructError::MissingEventData("intent")),
            };
            Ok(PayerPay::AtCollectMpin(AtCollectMpin { txn, intent }))
        }

        // Authenticated state combinations
        (EntitySnapshot::Authenticated(e), "ReqPay") => {
            let txn = Txn {
                id: e.id,
                data: Authenticated {
                    payer: e.payer,
                    intent: e.intent,
                    mpin: e.mpin,
                },
            };
            let req = txn.build_req_pay();
            Ok(PayerPay::AtReqPay(AtReqPay { txn, req }))
        }

        (EntitySnapshot::Authenticated(e), "Notify") => {
            let txn = Txn {
                id: e.id,
                data: Authenticated {
                    payer: e.payer,
                    intent: e.intent,
                    mpin: e.mpin,
                },
            };
            let req = txn.build_req_pay();
            let cmd: UserCommand = serde_json::from_value(current.event_body.clone())
                .map_err(|e| ReconstructError::JsonError(e.to_string()))?;
            let reason = match cmd {
                UserCommand::Notify { status: Status::Failed(r) } => r,
                _ => return Err(ReconstructError::MissingEventData("reason from status")),
            };
            Ok(PayerPay::AtNotifyRejected(AtNotifyRejected { txn, req, reason }))
        }

        // Submitted state combinations
        (EntitySnapshot::Submitted(e), "AwaitCallback") => {
            let txn = Txn {
                id: e.id,
                data: Submitted { req: e.req },
            };
            Ok(PayerPay::AtAwaitCallback(AtAwaitCallback { txn }))
        }

        (EntitySnapshot::Submitted(e), "Notify") => {
            let txn = Txn {
                id: e.id,
                data: Submitted { req: e.req },
            };
            let cmd: UserCommand = serde_json::from_value(current.event_body.clone())
                .map_err(|e| ReconstructError::JsonError(e.to_string()))?;
            match cmd {
                UserCommand::Notify { status: Status::Successful } => {
                    let (debit_ref, credit_ref) = extract_success_refs(events)?;
                    Ok(PayerPay::AtNotifySuccess(AtNotifySuccess { txn, debit_ref, credit_ref }))
                }
                UserCommand::Notify { status: Status::Failed(reason) } => {
                    Ok(PayerPay::AtNotifyFailed(AtNotifyFailed { txn, reason }))
                }
                UserCommand::Notify { status: Status::Processing } => {
                    Ok(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn }))
                }
                _ => Err(ReconstructError::MissingEventData("notify status")),
            }
        }

        // Deemed state combinations
        (EntitySnapshot::Deemed(e), "AwaitDeemedResolution") => {
            let txn = Txn {
                id: e.id,
                data: Deemed { req: e.req },
            };
            Ok(PayerPay::AtAwaitDeemed(AtAwaitDeemed { txn }))
        }

        (EntitySnapshot::Deemed(e), "Notify") => {
            let txn = Txn {
                id: e.id,
                data: Deemed { req: e.req },
            };
            let cmd: UserCommand = serde_json::from_value(current.event_body.clone())
                .map_err(|e| ReconstructError::JsonError(e.to_string()))?;
            match cmd {
                UserCommand::Notify { status: Status::Confirmed } => {
                    let credit_ref = extract_confirmed_credit_ref(events)?;
                    Ok(PayerPay::AtNotifyConfirmed(AtNotifyConfirmed { txn, credit_ref }))
                }
                UserCommand::Notify { status: Status::Reversed } => {
                    Ok(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed { txn }))
                }
                _ => Err(ReconstructError::MissingEventData("notify status")),
            }
        }

        // ReversalPending state combinations
        (EntitySnapshot::ReversalPending(e), "Notify") => {
            let txn = Txn {
                id: e.id,
                data: ReversalPending { req: e.req },
            };
            let cmd: UserCommand = serde_json::from_value(current.event_body.clone())
                .map_err(|e| ReconstructError::JsonError(e.to_string()))?;
            match cmd {
                UserCommand::Notify { status: Status::ReversalInProgress } => {
                    Ok(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart { txn }))
                }
                UserCommand::Notify { status: Status::Reversed } => {
                    Ok(PayerPay::AtNotifyReversed(AtNotifyReversed { txn }))
                }
                _ => Err(ReconstructError::MissingEventData("notify status")),
            }
        }

        (EntitySnapshot::ReversalPending(e), "AwaitForcedReversal") => {
            let txn = Txn {
                id: e.id,
                data: ReversalPending { req: e.req },
            };
            Ok(PayerPay::AtAwaitForcedReversal(AtAwaitForcedReversal { txn }))
        }

        // Invalid combinations
        _ => Err(ReconstructError::InvalidStateEventCombination {
            state: entity.state_name(),
            event_type: event_type.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decompose::decompose;

    fn make_initiated_txn() -> Txn<Initiated> {
        Txn::new(TxnId(1), PayerId(42))
    }

    fn make_intent() -> ValidIntent {
        ValidIntent {
            payee: Vpa(100),
            amount: Amount(5000),
        }
    }

    fn make_authenticated_txn() -> Txn<Authenticated> {
        make_initiated_txn().authenticate(make_intent(), Mpin(1234))
    }

    fn make_submitted_txn() -> Txn<Submitted> {
        let auth = make_authenticated_txn();
        let req = auth.build_req_pay();
        auth.submit(req)
    }

    fn make_deemed_txn() -> Txn<Deemed> {
        make_submitted_txn().deem()
    }

    fn make_reversal_pending_txn() -> Txn<ReversalPending> {
        make_deemed_txn().escalate_reversal()
    }

    /// Round-trip test helper: decompose then reconstruct and verify equality.
    fn round_trip(bookmark: PayerPay) {
        let (entity, ctx, event) = decompose(&bookmark, 0);
        let reconstructed = reconstruct(&entity, &ctx, &[event]).unwrap();
        assert_eq!(reconstructed, bookmark);
    }

    #[test]
    fn test_round_trip_at_get_payment_intent() {
        let txn = make_initiated_txn();
        round_trip(PayerPay::AtGetPaymentIntent(AtGetPaymentIntent { txn }));
    }

    #[test]
    fn test_round_trip_at_show_error() {
        let txn = make_initiated_txn();
        round_trip(PayerPay::AtShowError(AtShowError {
            txn,
            reason: Reason::InvalidAmount,
        }));
    }

    #[test]
    fn test_round_trip_at_collect_mpin() {
        let txn = make_initiated_txn();
        round_trip(PayerPay::AtCollectMpin(AtCollectMpin {
            txn,
            intent: make_intent(),
        }));
    }

    #[test]
    fn test_round_trip_at_req_pay() {
        let txn = make_authenticated_txn();
        let req = txn.build_req_pay();
        round_trip(PayerPay::AtReqPay(AtReqPay { txn, req }));
    }

    #[test]
    fn test_round_trip_at_notify_rejected() {
        let txn = make_authenticated_txn();
        let req = txn.build_req_pay();
        round_trip(PayerPay::AtNotifyRejected(AtNotifyRejected {
            txn,
            req,
            reason: Reason::InsufficientFunds,
        }));
    }

    #[test]
    fn test_round_trip_at_await_callback() {
        let txn = make_submitted_txn();
        round_trip(PayerPay::AtAwaitCallback(AtAwaitCallback { txn }));
    }

    #[test]
    fn test_round_trip_at_notify_success() {
        let txn = make_submitted_txn();
        let bookmark = PayerPay::AtNotifySuccess(AtNotifySuccess {
            txn,
            debit_ref: Ref(1001),
            credit_ref: Ref(2002),
        });

        let (entity, ctx, notify_event) = decompose(&bookmark, 1);

        // Create prerequisite callback event with success response
        let callback = SystemAsyncCallback::PaymentSuccess {
            debit_ref: Ref(1001),
            credit_ref: Ref(2002),
        };
        let mut callback_event = EventData::async_response_from_system(
            0,
            0, // in_response_to
            "PaymentSuccess",
            serde_json::to_value(&callback).unwrap(),
        );
        callback_event.mark_completed();

        let reconstructed = reconstruct(&entity, &ctx, &[callback_event, notify_event]).unwrap();
        assert_eq!(reconstructed, bookmark);
    }

    #[test]
    fn test_round_trip_at_notify_failed() {
        let txn = make_submitted_txn();
        round_trip(PayerPay::AtNotifyFailed(AtNotifyFailed {
            txn,
            reason: Reason::BankDeclined,
        }));
    }

    #[test]
    fn test_round_trip_at_notify_processing() {
        let txn = make_submitted_txn();
        round_trip(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn }));
    }

    #[test]
    fn test_round_trip_at_await_deemed() {
        let txn = make_deemed_txn();
        round_trip(PayerPay::AtAwaitDeemed(AtAwaitDeemed { txn }));
    }

    #[test]
    fn test_round_trip_at_notify_confirmed() {
        let txn = make_deemed_txn();
        let bookmark = PayerPay::AtNotifyConfirmed(AtNotifyConfirmed {
            txn,
            credit_ref: Ref(3003),
        });

        let (entity, ctx, notify_event) = decompose(&bookmark, 1);

        let callback = SystemAsyncCallback::DeemedSuccess { credit_ref: Ref(3003) };
        let mut callback_event = EventData::async_response_from_system(
            0,
            0, // in_response_to
            "DeemedSuccess",
            serde_json::to_value(&callback).unwrap(),
        );
        callback_event.mark_completed();

        let reconstructed = reconstruct(&entity, &ctx, &[callback_event, notify_event]).unwrap();
        assert_eq!(reconstructed, bookmark);
    }

    #[test]
    fn test_round_trip_at_notify_deemed_reversed() {
        let txn = make_deemed_txn();
        round_trip(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed { txn }));
    }

    #[test]
    fn test_round_trip_at_notify_reversal_start() {
        let txn = make_reversal_pending_txn();
        round_trip(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart { txn }));
    }

    #[test]
    fn test_round_trip_at_await_forced_reversal() {
        let txn = make_reversal_pending_txn();
        round_trip(PayerPay::AtAwaitForcedReversal(AtAwaitForcedReversal { txn }));
    }

    #[test]
    fn test_round_trip_at_notify_reversed() {
        let txn = make_reversal_pending_txn();
        round_trip(PayerPay::AtNotifyReversed(AtNotifyReversed { txn }));
    }

    #[test]
    fn test_terminal_state_error() {
        let entity = EntitySnapshot::Success(SuccessEntity {
            id: TxnId(1),
            req: PayRequest {
                payer: PayerId(42),
                payee: Vpa(100),
                amount: Amount(5000),
                mpin: Mpin(1234),
            },
            debit_ref: Some(Ref(1)),
            credit_ref: Ref(2),
        });
        let ctx = FlowContext::default();
        let event = EventData::request_to_user(
            0,
            "Notify",
            serde_json::to_value(&UserCommand::Notify { status: Status::Successful }).unwrap(),
        );

        let result = reconstruct(&entity, &ctx, &[event]);
        assert_eq!(result, Err(ReconstructError::TerminalState));
    }

    #[test]
    fn test_no_pending_event_error() {
        let entity = EntitySnapshot::Initiated(InitiatedEntity {
            id: TxnId(1),
            payer: PayerId(42),
        });
        let ctx = FlowContext::default();

        let mut event = EventData::request_to_user_async(
            0,
            "GetPaymentIntent",
            serde_json::to_value(&UserCommand::GetPaymentIntent { payer: PayerId(42) }).unwrap(),
        );
        event.mark_completed();

        let result = reconstruct(&entity, &ctx, &[event]);
        assert_eq!(result, Err(ReconstructError::NoPendingEvent));
    }

    #[test]
    fn test_invalid_state_event_combination() {
        let entity = EntitySnapshot::Initiated(InitiatedEntity {
            id: TxnId(1),
            payer: PayerId(42),
        });
        let ctx = FlowContext::default();

        // Create an event that doesn't match the Initiated state
        // AwaitCallback is only valid in Submitted state
        let event = EventData::async_response(
            0,
            Actor::ExternalSystem,
            Actor::ThisSystem,
            0, // in_response_to
            "AwaitCallback",
            serde_json::json!({}),
        );

        let result = reconstruct(&entity, &ctx, &[event]);
        assert!(matches!(
            result,
            Err(ReconstructError::InvalidStateEventCombination { .. })
        ));
    }
}
