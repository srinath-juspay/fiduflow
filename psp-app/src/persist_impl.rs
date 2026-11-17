//! Implementation of the Persist trait for PayFlow.

use crate::{EntitySnapshot, FlowContext, PayFlow, PayerPay, Txn};
use driver_framework::{MessageRecord, Persist, ReconstructError};
use psp_flow::Terminal;
use psp_persistence::entity::*;

impl Persist for PayFlow {
    type Entity = EntitySnapshot;
    type Context = FlowContext;

    fn decompose(state: &PayerPay) -> (EntitySnapshot, FlowContext, MessageRecord) {
        psp_persistence::decompose(state)
    }

    fn reconstruct(
        entity: &EntitySnapshot,
        ctx: &FlowContext,
        messages: &[MessageRecord],
    ) -> Result<PayerPay, ReconstructError> {
        psp_persistence::reconstruct(entity, ctx, messages)
    }

    fn terminal_entity(done: &Txn<Terminal>) -> EntitySnapshot {
        let id = done.id;
        match &done.data {
            Terminal::Success(s) => EntitySnapshot::Success(SuccessEntity {
                id,
                req: s.req,
                debit_ref: s.debit_ref,
                credit_ref: s.credit_ref,
            }),
            Terminal::Failed(f) => EntitySnapshot::Failed(FailedEntity {
                id,
                req: f.req,
                reason: f.reason,
            }),
            Terminal::Reversed(r) => EntitySnapshot::Reversed(ReversedEntity { id, req: r.req }),
            Terminal::Abandoned(a) => EntitySnapshot::Abandoned(AbandonedEntity {
                id,
                payer: a.payer,
            }),
        }
    }

    fn is_terminal(entity: &EntitySnapshot) -> bool {
        entity.is_terminal()
    }
}
