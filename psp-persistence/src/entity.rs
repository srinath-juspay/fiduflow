//! Typed entity snapshots per state.
//!
//! Each state has its own typed struct containing exactly the fields present in that state.
//! This avoids Option-heavy designs and makes the data requirements explicit at the type level.
//!
//! # Entity State Transitions
//!
//! ```text
//! Uninitialized  →  Initiated  →  Authenticated  →  Submitted  →  Success
//!                      ↓               ↓               ↓             ↓
//!                  Abandoned        Abandoned       Failed       Deemed → Success
//!                                                                   ↓       ↓
//!                                                              ReversalPending → Reversed
//! ```
//!
//! # When Each Variant Is Used
//!
//! - **Uninitialized**: Flow started but payer not yet identified (AtInit)
//! - **Initiated**: Payer identified, collecting intent/MPIN (AtGetPaymentIntent, AtShowError, AtCollectMpin)
//! - **Authenticated**: MPIN collected, ready to submit (AtReqPay, AtNotifyRejected)
//! - **Submitted**: Payment request sent to NPCI (AtAwaitCallback, AtNotify* states)
//! - **Deemed**: NPCI returned uncertain status (AtAwaitDeemed, AtNotify* states)
//! - **ReversalPending**: Reversal in progress (AtAwaitForcedReversal, AtNotify* states)
//! - **Success/Failed/Reversed/Abandoned**: Terminal states (no wait points)

use psp_flow::values::*;

/// Entity snapshot for the Uninitialized state (waiting for payer ID).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UninitializedEntity {
    pub id: TxnId,
}

/// Entity snapshot for the Initiated state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InitiatedEntity {
    pub id: TxnId,
    pub payer: PayerId,
}

/// Entity snapshot for the Authenticated state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedEntity {
    pub id: TxnId,
    pub payer: PayerId,
    pub intent: ValidIntent,
    pub mpin: Mpin,
}

/// Entity snapshot for the Submitted state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubmittedEntity {
    pub id: TxnId,
    pub req: PayRequest,
}

/// Entity snapshot for the Deemed state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeemedEntity {
    pub id: TxnId,
    pub req: PayRequest,
}

/// Entity snapshot for the ReversalPending state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReversalPendingEntity {
    pub id: TxnId,
    pub req: PayRequest,
}

/// Entity snapshot for the Success terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuccessEntity {
    pub id: TxnId,
    pub req: PayRequest,
    pub debit_ref: Option<Ref>,
    pub credit_ref: Ref,
}

/// Entity snapshot for the Failed terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FailedEntity {
    pub id: TxnId,
    pub req: PayRequest,
    pub reason: Reason,
}

/// Entity snapshot for the Reversed terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReversedEntity {
    pub id: TxnId,
    pub req: PayRequest,
}

/// Entity snapshot for the Abandoned terminal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbandonedEntity {
    pub id: TxnId,
    pub payer: PayerId,
}

/// The entity snapshot: one variant per entity state.
/// This mirrors the state machine's entity states but as a persistence-friendly enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitySnapshot {
    Uninitialized(UninitializedEntity),
    Initiated(InitiatedEntity),
    Authenticated(AuthenticatedEntity),
    Submitted(SubmittedEntity),
    Deemed(DeemedEntity),
    ReversalPending(ReversalPendingEntity),
    // Terminal states
    Success(SuccessEntity),
    Failed(FailedEntity),
    Reversed(ReversedEntity),
    Abandoned(AbandonedEntity),
}

impl EntitySnapshot {
    /// Get the transaction ID from any snapshot variant.
    pub fn id(&self) -> TxnId {
        match self {
            EntitySnapshot::Uninitialized(e) => e.id,
            EntitySnapshot::Initiated(e) => e.id,
            EntitySnapshot::Authenticated(e) => e.id,
            EntitySnapshot::Submitted(e) => e.id,
            EntitySnapshot::Deemed(e) => e.id,
            EntitySnapshot::ReversalPending(e) => e.id,
            EntitySnapshot::Success(e) => e.id,
            EntitySnapshot::Failed(e) => e.id,
            EntitySnapshot::Reversed(e) => e.id,
            EntitySnapshot::Abandoned(e) => e.id,
        }
    }

    /// Get the state name as a string (for database storage).
    pub fn state_name(&self) -> &'static str {
        match self {
            EntitySnapshot::Uninitialized(_) => "Uninitialized",
            EntitySnapshot::Initiated(_) => "Initiated",
            EntitySnapshot::Authenticated(_) => "Authenticated",
            EntitySnapshot::Submitted(_) => "Submitted",
            EntitySnapshot::Deemed(_) => "Deemed",
            EntitySnapshot::ReversalPending(_) => "ReversalPending",
            EntitySnapshot::Success(_) => "Success",
            EntitySnapshot::Failed(_) => "Failed",
            EntitySnapshot::Reversed(_) => "Reversed",
            EntitySnapshot::Abandoned(_) => "Abandoned",
        }
    }

    /// Check if this is a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            EntitySnapshot::Success(_)
                | EntitySnapshot::Failed(_)
                | EntitySnapshot::Reversed(_)
                | EntitySnapshot::Abandoned(_)
        )
    }
}
