//! Reconstruct a PayerPay bookmark from persistence snapshots.
//!
//! This function takes an entity snapshot, context, and event history and
//! reconstructs the original PayerPay bookmark. The reconstruction is pure
//! and can be formally verified.
//!
//! The key insight is that the current wait point can be determined by:
//! 1. The entity state
//! 2. The current event type
//! 3. Data from previous events (stored in their responses)

use crate::context::FlowContext;
use crate::entity::*;
use crate::event_data::*;
use psp_flow::entity::*;
use psp_flow::machine::*;
use psp_flow::values::*;

/// Errors that can occur during reconstruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconstructError {
    /// No current (non-processed) event found in the event list.
    NoCurrentEvent,
    /// The entity state and event type combination is invalid.
    InvalidStateEventCombination {
        state: &'static str,
        event_type: EventType,
    },
    /// Missing required data from event history.
    MissingEventData(&'static str),
    /// The entity snapshot is a terminal state (should use Stored::Finished).
    TerminalState,
}

/// Find the current (non-processed) event from the event list.
fn find_current_event(events: &[EventData]) -> Option<&EventData> {
    events.iter().find(|e| e.is_current())
}

/// Extract a validated intent from event history.
/// The intent comes from a previous GetPaymentIntent response.
#[allow(dead_code)]
fn extract_intent_from_events(events: &[EventData]) -> Result<ValidIntent, ReconstructError> {
    for event in events {
        if event.event_type == EventType::GetPaymentIntent {
            if let Some(resp) = &event.response {
                if let Some(raw) = resp.raw_intent {
                    // We trust stored intents are valid (they were validated before storage)
                    return Ok(ValidIntent {
                        payee: raw.payee,
                        amount: raw.amount,
                    });
                }
            }
        }
        // Also check CollectMpin request which stores the intent
        if event.event_type == EventType::CollectMpin {
            if let Some(intent) = event.request.intent {
                return Ok(intent);
            }
        }
    }
    Err(ReconstructError::MissingEventData("intent"))
}

/// Extract reason from the current event request.
fn extract_reason_from_request(event: &EventData) -> Result<Reason, ReconstructError> {
    event
        .request
        .reason
        .ok_or(ReconstructError::MissingEventData("reason"))
}

/// Extract reason from status in request.
fn extract_reason_from_status(event: &EventData) -> Result<Reason, ReconstructError> {
    match event.request.status {
        Some(Status::Failed(reason)) => Ok(reason),
        _ => Err(ReconstructError::MissingEventData("reason from status")),
    }
}

/// Extract refs from event history for NotifySuccess.
fn extract_success_refs(events: &[EventData]) -> Result<(Ref, Ref), ReconstructError> {
    // Look for AwaitCallback response with success
    for event in events {
        if event.event_type == EventType::AwaitCallback {
            if let Some(resp) = &event.response {
                if resp.callback_type == Some(CallbackType::Success) {
                    let debit_ref = resp
                        .debit_ref
                        .ok_or(ReconstructError::MissingEventData("debit_ref"))?;
                    let credit_ref = resp
                        .credit_ref
                        .ok_or(ReconstructError::MissingEventData("credit_ref"))?;
                    return Ok((debit_ref, credit_ref));
                }
            }
        }
    }
    Err(ReconstructError::MissingEventData("success refs"))
}

/// Extract failure reason from event history for NotifyFailed.
#[allow(dead_code)]
fn extract_failure_reason(events: &[EventData]) -> Result<Reason, ReconstructError> {
    // Look for AwaitCallback response with failure
    for event in events {
        if event.event_type == EventType::AwaitCallback {
            if let Some(resp) = &event.response {
                if resp.callback_type == Some(CallbackType::Failed) {
                    return resp
                        .callback_reason
                        .ok_or(ReconstructError::MissingEventData("callback_reason"));
                }
            }
        }
    }
    Err(ReconstructError::MissingEventData("failure reason"))
}

/// Extract credit_ref from event history for NotifyConfirmed.
fn extract_confirmed_credit_ref(events: &[EventData]) -> Result<Ref, ReconstructError> {
    // Look for AwaitDeemedResolution response with success
    for event in events {
        if event.event_type == EventType::AwaitDeemedResolution {
            if let Some(resp) = &event.response {
                if resp.resolution_type == Some(ResolutionType::DeemedSuccess) {
                    return resp
                        .credit_ref
                        .ok_or(ReconstructError::MissingEventData("credit_ref"));
                }
            }
        }
    }
    Err(ReconstructError::MissingEventData("confirmed credit_ref"))
}

/// Reconstruct a PayerPay bookmark from persistence snapshots.
///
/// # Arguments
/// * `entity` - The entity snapshot
/// * `_ctx` - The flow context (currently unused)
/// * `events` - The event history including the current event
///
/// # Returns
/// The reconstructed PayerPay bookmark, or an error if reconstruction fails.
pub fn reconstruct(
    entity: &EntitySnapshot,
    _ctx: &FlowContext,
    events: &[EventData],
) -> Result<PayerPay, ReconstructError> {
    // Terminal states should not be reconstructed as PayerPay
    if entity.is_terminal() {
        return Err(ReconstructError::TerminalState);
    }

    let current = find_current_event(events).ok_or(ReconstructError::NoCurrentEvent)?;

    match (entity, current.event_type) {
        // Initiated state combinations
        (EntitySnapshot::Initiated(e), EventType::GetPaymentIntent) => {
            let txn = Txn {
                id: e.id,
                data: Initiated { payer: e.payer },
            };
            Ok(PayerPay::AtGetPaymentIntent(AtGetPaymentIntent { txn }))
        }

        (EntitySnapshot::Initiated(e), EventType::ShowError) => {
            let txn = Txn {
                id: e.id,
                data: Initiated { payer: e.payer },
            };
            let reason = extract_reason_from_request(current)?;
            Ok(PayerPay::AtShowError(AtShowError { txn, reason }))
        }

        (EntitySnapshot::Initiated(e), EventType::CollectMpin) => {
            let txn = Txn {
                id: e.id,
                data: Initiated { payer: e.payer },
            };
            let intent = current
                .request
                .intent
                .ok_or(ReconstructError::MissingEventData("intent in request"))?;
            Ok(PayerPay::AtCollectMpin(AtCollectMpin { txn, intent }))
        }

        // Authenticated state combinations
        (EntitySnapshot::Authenticated(e), EventType::ReqPay) => {
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

        (EntitySnapshot::Authenticated(e), EventType::Notify) => {
            let txn = Txn {
                id: e.id,
                data: Authenticated {
                    payer: e.payer,
                    intent: e.intent,
                    mpin: e.mpin,
                },
            };
            let req = txn.build_req_pay();
            let reason = extract_reason_from_status(current)?;
            Ok(PayerPay::AtNotifyRejected(AtNotifyRejected {
                txn,
                req,
                reason,
            }))
        }

        // Submitted state combinations
        (EntitySnapshot::Submitted(e), EventType::AwaitCallback) => {
            let txn = Txn {
                id: e.id,
                data: Submitted { req: e.req },
            };
            Ok(PayerPay::AtAwaitCallback(AtAwaitCallback { txn }))
        }

        (EntitySnapshot::Submitted(e), EventType::Notify) => {
            let txn = Txn {
                id: e.id,
                data: Submitted { req: e.req },
            };
            // Determine which Notify variant based on the status in request
            match current.request.status {
                Some(Status::Successful) => {
                    let (debit_ref, credit_ref) = extract_success_refs(events)?;
                    Ok(PayerPay::AtNotifySuccess(AtNotifySuccess {
                        txn,
                        debit_ref,
                        credit_ref,
                    }))
                }
                Some(Status::Failed(reason)) => {
                    Ok(PayerPay::AtNotifyFailed(AtNotifyFailed { txn, reason }))
                }
                Some(Status::Processing) => {
                    Ok(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn }))
                }
                _ => Err(ReconstructError::MissingEventData("notify status")),
            }
        }

        // Deemed state combinations
        (EntitySnapshot::Deemed(e), EventType::AwaitDeemedResolution) => {
            let txn = Txn {
                id: e.id,
                data: Deemed { req: e.req },
            };
            Ok(PayerPay::AtAwaitDeemed(AtAwaitDeemed { txn }))
        }

        (EntitySnapshot::Deemed(e), EventType::Notify) => {
            let txn = Txn {
                id: e.id,
                data: Deemed { req: e.req },
            };
            // Determine which Notify variant based on the status in request
            match current.request.status {
                Some(Status::Confirmed) => {
                    let credit_ref = extract_confirmed_credit_ref(events)?;
                    Ok(PayerPay::AtNotifyConfirmed(AtNotifyConfirmed {
                        txn,
                        credit_ref,
                    }))
                }
                Some(Status::Reversed) => {
                    Ok(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed {
                        txn,
                    }))
                }
                _ => Err(ReconstructError::MissingEventData("notify status")),
            }
        }

        // ReversalPending state combinations
        (EntitySnapshot::ReversalPending(e), EventType::Notify) => {
            let txn = Txn {
                id: e.id,
                data: ReversalPending { req: e.req },
            };
            // Determine which Notify variant based on the status in request
            match current.request.status {
                Some(Status::ReversalInProgress) => {
                    Ok(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart {
                        txn,
                    }))
                }
                Some(Status::Reversed) => {
                    Ok(PayerPay::AtNotifyReversed(AtNotifyReversed { txn }))
                }
                _ => Err(ReconstructError::MissingEventData("notify status")),
            }
        }

        (EntitySnapshot::ReversalPending(e), EventType::AwaitForcedReversal) => {
            let txn = Txn {
                id: e.id,
                data: ReversalPending { req: e.req },
            };
            Ok(PayerPay::AtAwaitForcedReversal(AtAwaitForcedReversal {
                txn,
            }))
        }

        // Invalid combinations
        _ => Err(ReconstructError::InvalidStateEventCombination {
            state: entity.state_name(),
            event_type: current.event_type,
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

    // ---- Initiated state tests ----

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

    // ---- Authenticated state tests ----

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

    // ---- Submitted state tests ----

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

        // AtNotifySuccess requires previous AwaitCallback event with success response
        let (entity, ctx, notify_event) = decompose(&bookmark, 1);

        // Create the prerequisite AwaitCallback event with response
        let mut callback_event = EventData::new(
            0,
            EventType::AwaitCallback,
            EventRequest::await_callback(CALLBACK_TIMEOUT_SECS),
        );
        callback_event.status = EventStatus::Processed;
        callback_event.response = Some(EventResponse::callback_success(Ref(1001), Ref(2002)));

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

    // ---- Deemed state tests ----

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

        // AtNotifyConfirmed requires previous AwaitDeemedResolution with success response
        let (entity, ctx, notify_event) = decompose(&bookmark, 1);

        let mut resolution_event = EventData::new(
            0,
            EventType::AwaitDeemedResolution,
            EventRequest::await_deemed_resolution(DEEMED_DEADLINE_DAYS),
        );
        resolution_event.status = EventStatus::Processed;
        resolution_event.response = Some(EventResponse::resolution_success(Ref(3003)));

        let reconstructed =
            reconstruct(&entity, &ctx, &[resolution_event, notify_event]).unwrap();
        assert_eq!(reconstructed, bookmark);
    }

    #[test]
    fn test_round_trip_at_notify_deemed_reversed() {
        let txn = make_deemed_txn();
        round_trip(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed {
            txn,
        }));
    }

    // ---- ReversalPending state tests ----

    #[test]
    fn test_round_trip_at_notify_reversal_start() {
        let txn = make_reversal_pending_txn();
        round_trip(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart {
            txn,
        }));
    }

    #[test]
    fn test_round_trip_at_await_forced_reversal() {
        let txn = make_reversal_pending_txn();
        round_trip(PayerPay::AtAwaitForcedReversal(AtAwaitForcedReversal {
            txn,
        }));
    }

    #[test]
    fn test_round_trip_at_notify_reversed() {
        let txn = make_reversal_pending_txn();
        round_trip(PayerPay::AtNotifyReversed(AtNotifyReversed { txn }));
    }

    // ---- Error cases ----

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
        let event = EventData::new(
            0,
            EventType::Notify,
            EventRequest::notify(Status::Successful),
        );

        let result = reconstruct(&entity, &ctx, &[event]);
        assert_eq!(result, Err(ReconstructError::TerminalState));
    }

    #[test]
    fn test_no_current_event_error() {
        let entity = EntitySnapshot::Initiated(InitiatedEntity {
            id: TxnId(1),
            payer: PayerId(42),
        });
        let ctx = FlowContext::default();

        // All events are processed
        let mut event = EventData::new(
            0,
            EventType::GetPaymentIntent,
            EventRequest::get_payment_intent(PayerId(42)),
        );
        event.status = EventStatus::Processed;

        let result = reconstruct(&entity, &ctx, &[event]);
        assert_eq!(result, Err(ReconstructError::NoCurrentEvent));
    }

    #[test]
    fn test_invalid_state_event_combination() {
        let entity = EntitySnapshot::Initiated(InitiatedEntity {
            id: TxnId(1),
            payer: PayerId(42),
        });
        let ctx = FlowContext::default();

        // AwaitCallback is not valid for Initiated state
        let event = EventData::new(
            0,
            EventType::AwaitCallback,
            EventRequest::await_callback(30),
        );

        let result = reconstruct(&entity, &ctx, &[event]);
        assert!(matches!(
            result,
            Err(ReconstructError::InvalidStateEventCombination { .. })
        ));
    }
}
