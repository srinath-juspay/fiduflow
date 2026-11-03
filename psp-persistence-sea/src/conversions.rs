//! Conversions between SeaORM models and persistence layer types.

use crate::models::{events, flow_context, upi_txn};
use chrono::Utc;
use psp_flow::values::*;
use psp_persistence::entity::*;
use psp_persistence::event_data::*;
use psp_persistence::FlowContext;
use sea_orm::ActiveValue::Set;
use thiserror::Error;

/// Errors that can occur during conversion.
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Invalid event type: {0}")]
    InvalidEventType(String),
    #[error("Invalid event status: {0}")]
    InvalidEventStatus(String),
    #[error("Invalid reason: {0}")]
    InvalidReason(String),
    #[error("Invalid status: {0}")]
    InvalidStatus(String),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
}

// ---- Reason conversions ----

pub fn reason_to_str(r: Reason) -> &'static str {
    match r {
        Reason::InvalidAmount => "InvalidAmount",
        Reason::UnknownPayee => "UnknownPayee",
        Reason::PayeeIsPayer => "PayeeIsPayer",
        Reason::InsufficientFunds => "InsufficientFunds",
        Reason::BankDeclined => "BankDeclined",
        Reason::RiskRejected => "RiskRejected",
    }
}

pub fn reason_from_str(s: &str) -> Result<Reason, ConversionError> {
    match s {
        "InvalidAmount" => Ok(Reason::InvalidAmount),
        "UnknownPayee" => Ok(Reason::UnknownPayee),
        "PayeeIsPayer" => Ok(Reason::PayeeIsPayer),
        "InsufficientFunds" => Ok(Reason::InsufficientFunds),
        "BankDeclined" => Ok(Reason::BankDeclined),
        "RiskRejected" => Ok(Reason::RiskRejected),
        other => Err(ConversionError::InvalidReason(other.to_string())),
    }
}

// ---- Status conversions ----

pub fn status_to_str(s: &Status) -> String {
    match s {
        Status::Successful => "Successful".to_string(),
        Status::Failed(r) => format!("Failed:{}", reason_to_str(*r)),
        Status::Processing => "Processing".to_string(),
        Status::Confirmed => "Confirmed".to_string(),
        Status::Reversed => "Reversed".to_string(),
        Status::ReversalInProgress => "ReversalInProgress".to_string(),
    }
}

pub fn status_from_str(s: &str) -> Result<Status, ConversionError> {
    if let Some(reason_str) = s.strip_prefix("Failed:") {
        let reason = reason_from_str(reason_str)?;
        return Ok(Status::Failed(reason));
    }
    match s {
        "Successful" => Ok(Status::Successful),
        "Processing" => Ok(Status::Processing),
        "Confirmed" => Ok(Status::Confirmed),
        "Reversed" => Ok(Status::Reversed),
        "ReversalInProgress" => Ok(Status::ReversalInProgress),
        other => Err(ConversionError::InvalidStatus(other.to_string())),
    }
}

// ---- EntitySnapshot <-> upi_txn::Model ----

pub fn entity_to_model(id: TxnId, entity: &EntitySnapshot) -> upi_txn::ActiveModel {
    let now = Utc::now();
    let mut model = upi_txn::ActiveModel {
        id: Set(id.0 as i64),
        state: Set(entity.state_name().to_string()),
        payer: Set(0), // will be set below
        payee: Set(None),
        amount: Set(None),
        mpin: Set(None),
        debit_ref: Set(None),
        credit_ref: Set(None),
        failure_reason: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    };

    match entity {
        EntitySnapshot::Initiated(e) => {
            model.payer = Set(e.payer.0 as i64);
        }
        EntitySnapshot::Authenticated(e) => {
            model.payer = Set(e.payer.0 as i64);
            model.payee = Set(Some(e.intent.payee.0 as i64));
            model.amount = Set(Some(e.intent.amount.0 as i64));
            model.mpin = Set(Some(e.mpin.0 as i64));
        }
        EntitySnapshot::Submitted(e) => {
            model.payer = Set(e.req.payer.0 as i64);
            model.payee = Set(Some(e.req.payee.0 as i64));
            model.amount = Set(Some(e.req.amount.0 as i64));
            model.mpin = Set(Some(e.req.mpin.0 as i64));
        }
        EntitySnapshot::Deemed(e) => {
            model.payer = Set(e.req.payer.0 as i64);
            model.payee = Set(Some(e.req.payee.0 as i64));
            model.amount = Set(Some(e.req.amount.0 as i64));
            model.mpin = Set(Some(e.req.mpin.0 as i64));
        }
        EntitySnapshot::ReversalPending(e) => {
            model.payer = Set(e.req.payer.0 as i64);
            model.payee = Set(Some(e.req.payee.0 as i64));
            model.amount = Set(Some(e.req.amount.0 as i64));
            model.mpin = Set(Some(e.req.mpin.0 as i64));
        }
        EntitySnapshot::Success(e) => {
            model.payer = Set(e.req.payer.0 as i64);
            model.payee = Set(Some(e.req.payee.0 as i64));
            model.amount = Set(Some(e.req.amount.0 as i64));
            model.mpin = Set(Some(e.req.mpin.0 as i64));
            model.debit_ref = Set(e.debit_ref.map(|r| r.0 as i64));
            model.credit_ref = Set(Some(e.credit_ref.0 as i64));
        }
        EntitySnapshot::Failed(e) => {
            model.payer = Set(e.req.payer.0 as i64);
            model.payee = Set(Some(e.req.payee.0 as i64));
            model.amount = Set(Some(e.req.amount.0 as i64));
            model.mpin = Set(Some(e.req.mpin.0 as i64));
            model.failure_reason = Set(Some(reason_to_str(e.reason).to_string()));
        }
        EntitySnapshot::Reversed(e) => {
            model.payer = Set(e.req.payer.0 as i64);
            model.payee = Set(Some(e.req.payee.0 as i64));
            model.amount = Set(Some(e.req.amount.0 as i64));
            model.mpin = Set(Some(e.req.mpin.0 as i64));
        }
        EntitySnapshot::Abandoned(e) => {
            model.payer = Set(e.payer.0 as i64);
        }
    }

    model
}

pub fn model_to_entity(model: &upi_txn::Model) -> Result<EntitySnapshot, ConversionError> {
    let id = TxnId(model.id as u64);
    let payer = PayerId(model.payer as u64);

    match model.state.as_str() {
        "Initiated" => Ok(EntitySnapshot::Initiated(InitiatedEntity { id, payer })),

        "Authenticated" => {
            let payee = Vpa(
                model
                    .payee
                    .ok_or(ConversionError::MissingField("payee"))? as u64,
            );
            let amount = Amount(
                model
                    .amount
                    .ok_or(ConversionError::MissingField("amount"))? as u64,
            );
            let mpin = Mpin(
                model.mpin.ok_or(ConversionError::MissingField("mpin"))? as u32,
            );
            Ok(EntitySnapshot::Authenticated(AuthenticatedEntity {
                id,
                payer,
                intent: ValidIntent { payee, amount },
                mpin,
            }))
        }

        "Submitted" => {
            let req = extract_pay_request(model)?;
            Ok(EntitySnapshot::Submitted(SubmittedEntity { id, req }))
        }

        "Deemed" => {
            let req = extract_pay_request(model)?;
            Ok(EntitySnapshot::Deemed(DeemedEntity { id, req }))
        }

        "ReversalPending" => {
            let req = extract_pay_request(model)?;
            Ok(EntitySnapshot::ReversalPending(ReversalPendingEntity {
                id,
                req,
            }))
        }

        "Success" => {
            let req = extract_pay_request(model)?;
            let debit_ref = model.debit_ref.map(|r| Ref(r as u64));
            let credit_ref = Ref(
                model
                    .credit_ref
                    .ok_or(ConversionError::MissingField("credit_ref"))? as u64,
            );
            Ok(EntitySnapshot::Success(SuccessEntity {
                id,
                req,
                debit_ref,
                credit_ref,
            }))
        }

        "Failed" => {
            let req = extract_pay_request(model)?;
            let reason_str = model
                .failure_reason
                .as_ref()
                .ok_or(ConversionError::MissingField("failure_reason"))?;
            let reason = reason_from_str(reason_str)?;
            Ok(EntitySnapshot::Failed(FailedEntity { id, req, reason }))
        }

        "Reversed" => {
            let req = extract_pay_request(model)?;
            Ok(EntitySnapshot::Reversed(ReversedEntity { id, req }))
        }

        "Abandoned" => Ok(EntitySnapshot::Abandoned(AbandonedEntity { id, payer })),

        other => Err(ConversionError::InvalidState(other.to_string())),
    }
}

fn extract_pay_request(model: &upi_txn::Model) -> Result<PayRequest, ConversionError> {
    Ok(PayRequest {
        payer: PayerId(model.payer as u64),
        payee: Vpa(
            model
                .payee
                .ok_or(ConversionError::MissingField("payee"))? as u64,
        ),
        amount: Amount(
            model
                .amount
                .ok_or(ConversionError::MissingField("amount"))? as u64,
        ),
        mpin: Mpin(model.mpin.ok_or(ConversionError::MissingField("mpin"))? as u32),
    })
}

// ---- EventData <-> events::Model ----

pub fn event_to_model(txn_id: TxnId, event: &EventData) -> events::ActiveModel {
    let now = Utc::now();
    let mut model = events::ActiveModel {
        id: Set(Default::default()), // auto-generated
        txn_id: Set(txn_id.0 as i64),
        seq: Set(event.seq),
        event_type: Set(event.event_type.as_str().to_string()),
        status: Set(event.status.as_str().to_string()),
        // Request fields
        req_payer: Set(event.request.payer.map(|p| p.0 as i64)),
        req_intent_payee: Set(event.request.intent.map(|i| i.payee.0 as i64)),
        req_intent_amount: Set(event.request.intent.map(|i| i.amount.0 as i64)),
        req_reason: Set(event.request.reason.map(|r| reason_to_str(r).to_string())),
        req_status: Set(event.request.status.map(|s| status_to_str(&s))),
        req_pay_payer: Set(event.request.req.map(|r| r.payer.0 as i64)),
        req_pay_payee: Set(event.request.req.map(|r| r.payee.0 as i64)),
        req_pay_amount: Set(event.request.req.map(|r| r.amount.0 as i64)),
        req_pay_mpin: Set(event.request.req.map(|r| r.mpin.0 as i64)),
        req_timeout_secs: Set(event.request.timeout_secs.map(|t| t as i64)),
        req_deadline_days: Set(event.request.deadline_days.map(|d| d as i64)),
        // Response fields (will be updated later)
        resp_raw_payee: Set(None),
        resp_raw_amount: Set(None),
        resp_mpin: Set(None),
        resp_req_pay_result: Set(None),
        resp_reject_reason: Set(None),
        resp_callback_type: Set(None),
        resp_debit_ref: Set(None),
        resp_credit_ref: Set(None),
        resp_callback_reason: Set(None),
        resp_resolution_type: Set(None),
        // Timestamps
        created_at: Set(now),
        sent_at: Set(None),
        received_at: Set(None),
        processed_at: Set(None),
    };

    // Fill response fields if present
    if let Some(resp) = &event.response {
        if let Some(raw) = resp.raw_intent {
            model.resp_raw_payee = Set(Some(raw.payee.0 as i64));
            model.resp_raw_amount = Set(Some(raw.amount.0 as i64));
        }
        if let Some(mpin) = resp.mpin {
            model.resp_mpin = Set(Some(mpin.0 as i64));
        }
        if let Some(result) = resp.req_pay_result {
            model.resp_req_pay_result = Set(Some(result.as_str().to_string()));
        }
        if let Some(reason) = resp.reject_reason {
            model.resp_reject_reason = Set(Some(reason_to_str(reason).to_string()));
        }
        if let Some(cb_type) = resp.callback_type {
            model.resp_callback_type = Set(Some(cb_type.as_str().to_string()));
        }
        if let Some(debit_ref) = resp.debit_ref {
            model.resp_debit_ref = Set(Some(debit_ref.0 as i64));
        }
        if let Some(credit_ref) = resp.credit_ref {
            model.resp_credit_ref = Set(Some(credit_ref.0 as i64));
        }
        if let Some(reason) = resp.callback_reason {
            model.resp_callback_reason = Set(Some(reason_to_str(reason).to_string()));
        }
        if let Some(res_type) = resp.resolution_type {
            model.resp_resolution_type = Set(Some(res_type.as_str().to_string()));
        }
    }

    model
}

pub fn model_to_event(model: &events::Model) -> Result<EventData, ConversionError> {
    let event_type = EventType::from_str(&model.event_type)
        .ok_or_else(|| ConversionError::InvalidEventType(model.event_type.clone()))?;

    let status = EventStatus::from_str(&model.status)
        .ok_or_else(|| ConversionError::InvalidEventStatus(model.status.clone()))?;

    // Build request
    let intent = match (model.req_intent_payee, model.req_intent_amount) {
        (Some(payee), Some(amount)) => Some(ValidIntent {
            payee: Vpa(payee as u64),
            amount: Amount(amount as u64),
        }),
        _ => None,
    };

    let req = match (
        model.req_pay_payer,
        model.req_pay_payee,
        model.req_pay_amount,
        model.req_pay_mpin,
    ) {
        (Some(payer), Some(payee), Some(amount), Some(mpin)) => Some(PayRequest {
            payer: PayerId(payer as u64),
            payee: Vpa(payee as u64),
            amount: Amount(amount as u64),
            mpin: Mpin(mpin as u32),
        }),
        _ => None,
    };

    let req_status = model
        .req_status
        .as_ref()
        .map(|s| status_from_str(s))
        .transpose()?;

    let req_reason = model
        .req_reason
        .as_ref()
        .map(|r| reason_from_str(r))
        .transpose()?;

    let request = EventRequest {
        event_type: Some(event_type),
        payer: model.req_payer.map(|p| PayerId(p as u64)),
        intent,
        reason: req_reason,
        status: req_status,
        req,
        timeout_secs: model.req_timeout_secs.map(|t| t as u64),
        deadline_days: model.req_deadline_days.map(|d| d as u64),
    };

    // Build response if any response fields are set
    let response = if model.resp_raw_payee.is_some()
        || model.resp_mpin.is_some()
        || model.resp_req_pay_result.is_some()
        || model.resp_callback_type.is_some()
        || model.resp_resolution_type.is_some()
    {
        let raw_intent = match (model.resp_raw_payee, model.resp_raw_amount) {
            (Some(payee), Some(amount)) => Some(RawIntent {
                payee: Vpa(payee as u64),
                amount: Amount(amount as u64),
            }),
            _ => None,
        };

        let req_pay_result = model
            .resp_req_pay_result
            .as_ref()
            .and_then(|r| ReqPayResult::from_str(r));

        let reject_reason = model
            .resp_reject_reason
            .as_ref()
            .map(|r| reason_from_str(r))
            .transpose()?;

        let callback_type = model
            .resp_callback_type
            .as_ref()
            .and_then(|c| CallbackType::from_str(c));

        let callback_reason = model
            .resp_callback_reason
            .as_ref()
            .map(|r| reason_from_str(r))
            .transpose()?;

        let resolution_type = model
            .resp_resolution_type
            .as_ref()
            .and_then(|r| ResolutionType::from_str(r));

        Some(EventResponse {
            raw_intent,
            mpin: model.resp_mpin.map(|m| Mpin(m as u32)),
            req_pay_result,
            reject_reason,
            callback_type,
            debit_ref: model.resp_debit_ref.map(|r| Ref(r as u64)),
            credit_ref: model.resp_credit_ref.map(|r| Ref(r as u64)),
            callback_reason,
            resolution_type,
        })
    } else {
        None
    };

    Ok(EventData {
        seq: model.seq,
        event_type,
        status,
        request,
        response,
    })
}

// ---- FlowContext <-> flow_context::Model ----

pub fn context_to_model(txn_id: TxnId, _ctx: &FlowContext) -> flow_context::ActiveModel {
    flow_context::ActiveModel {
        txn_id: Set(txn_id.0 as i64),
        // Add field mappings here as context grows
    }
}

pub fn model_to_context(_model: &flow_context::Model) -> FlowContext {
    // Currently empty, just return default
    FlowContext::default()
}
