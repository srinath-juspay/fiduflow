//! Conversions between SeaORM models and persistence layer types.

use crate::models::{events, flow_context, upi_txn};
use chrono::Utc;
use psp_flow::values::*;
use psp_persistence::entity::*;
use psp_persistence::{Actor, EventData, EventStatus, FlowContext, Method};
use sea_orm::ActiveValue::Set;
use thiserror::Error;

/// Errors that can occur during conversion.
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Invalid state: {0}")]
    InvalidState(String),
    #[error("Invalid event status: {0}")]
    InvalidEventStatus(String),
    #[error("Invalid reason: {0}")]
    InvalidReason(String),
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
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
    events::ActiveModel {
        id: Set(Default::default()), // auto-generated
        txn_id: Set(txn_id.0 as i64),
        seq: Set(event.seq),
        // Classification columns
        source: Set(event.source.as_str().to_string()),
        target: Set(event.target.as_str().to_string()),
        method: Set(event.method.as_str().to_string()),
        // Payload
        event_type: Set(event.event_type.clone()),
        event_body: Set(serde_json::to_string(&event.event_body).unwrap()),
        // Status
        status: Set(event.status.as_str().to_string()),
        created_at: Set(now),
        completed_at: Set(None),
    }
}

pub fn model_to_event(model: &events::Model) -> Result<EventData, ConversionError> {
    let status = EventStatus::from_str(&model.status)
        .ok_or_else(|| ConversionError::InvalidEventStatus(model.status.clone()))?;

    let source = Actor::from_str(&model.source)
        .ok_or_else(|| ConversionError::InvalidState(format!("Invalid source: {}", model.source)))?;
    let target = Actor::from_str(&model.target)
        .ok_or_else(|| ConversionError::InvalidState(format!("Invalid target: {}", model.target)))?;
    let method = Method::from_str(&model.method)
        .ok_or_else(|| ConversionError::InvalidState(format!("Invalid method: {}", model.method)))?;

    let event_body: serde_json::Value = serde_json::from_str(&model.event_body)?;

    Ok(EventData {
        seq: model.seq,
        source,
        target,
        method,
        event_type: model.event_type.clone(),
        event_body,
        status,
    })
}

// ---- FlowContext <-> flow_context::Model ----

pub fn context_to_model(txn_id: TxnId, ctx: &FlowContext) -> flow_context::ActiveModel {
    flow_context::ActiveModel {
        txn_id: Set(txn_id.0 as i64),
        data: Set(serde_json::to_string(ctx).unwrap()),
    }
}

pub fn model_to_context(model: &flow_context::Model) -> Result<FlowContext, ConversionError> {
    let ctx: FlowContext = serde_json::from_str(&model.data)?;
    Ok(ctx)
}
