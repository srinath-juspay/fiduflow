//! The `UpiTxn` entity. One struct per state; `Txn<S>` is the typestate view; the
//! transitions are the only way to move between states. (Hand-expansion of `#[entity]`.)

use crate::values::*;

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Initiated { pub payer: PayerId }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Authenticated { pub payer: PayerId, pub intent: ValidIntent, pub mpin: Mpin }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Submitted { pub req: PayRequest }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Deemed { pub req: PayRequest }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct ReversalPending { pub req: PayRequest }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Success { pub req: PayRequest, pub debit_ref: Option<Ref>, pub credit_ref: Ref }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Failed { pub req: PayRequest, pub reason: Reason }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Reversed { pub req: PayRequest }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Abandoned { pub payer: PayerId }

/// State set `Terminal`, as a type so a flow can return "any terminal state".
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Terminal {
    Success(Success),
    Failed(Failed),
    Reversed(Reversed),
    Abandoned(Abandoned),
}

/// Typed view of the entity: its id plus the data of exactly one state.
/// Fields are public so the persistence layer can reconstruct it from its tables;
/// the DSL would restrict that to the generated persistence code.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Txn<S> {
    pub id: TxnId,
    pub data: S,
}

impl Txn<Initiated> {
    pub fn new(id: TxnId, payer: PayerId) -> Self {
        Txn { id, data: Initiated { payer } }
    }
    pub fn abandon(self) -> Txn<Abandoned> {
        Txn { id: self.id, data: Abandoned { payer: self.data.payer } }
    }
    pub fn authenticate(self, intent: ValidIntent, mpin: Mpin) -> Txn<Authenticated> {
        Txn { id: self.id, data: Authenticated { payer: self.data.payer, intent, mpin } }
    }
}

impl Txn<Authenticated> {
    /// Pure. Needs an `Authenticated` transaction: this is property P5 as a type.
    pub fn build_req_pay(&self) -> PayRequest {
        PayRequest {
            payer: self.data.payer,
            payee: self.data.intent.payee,
            amount: self.data.intent.amount,
            mpin: self.data.mpin,
        }
    }
    pub fn submit(self, req: PayRequest) -> Txn<Submitted> {
        Txn { id: self.id, data: Submitted { req } }
    }
    pub fn reject(self, req: PayRequest, reason: Reason) -> Txn<Failed> {
        Txn { id: self.id, data: Failed { req, reason } }
    }
}

impl Txn<Submitted> {
    pub fn succeed(self, debit_ref: Ref, credit_ref: Ref) -> Txn<Success> {
        Txn { id: self.id, data: Success { req: self.data.req, debit_ref: Some(debit_ref), credit_ref } }
    }
    pub fn fail(self, reason: Reason) -> Txn<Failed> {
        Txn { id: self.id, data: Failed { req: self.data.req, reason } }
    }
    pub fn deem(self) -> Txn<Deemed> {
        Txn { id: self.id, data: Deemed { req: self.data.req } }
    }
}

impl Txn<Deemed> {
    pub fn confirm(self, credit_ref: Ref) -> Txn<Success> {
        Txn { id: self.id, data: Success { req: self.data.req, debit_ref: None, credit_ref } }
    }
    pub fn reverse(self) -> Txn<Reversed> {
        Txn { id: self.id, data: Reversed { req: self.data.req } }
    }
    pub fn escalate_reversal(self) -> Txn<ReversalPending> {
        Txn { id: self.id, data: ReversalPending { req: self.data.req } }
    }
}

impl Txn<ReversalPending> {
    pub fn reversed(self) -> Txn<Reversed> {
        Txn { id: self.id, data: Reversed { req: self.data.req } }
    }
}

// Terminal wrapping, written out instead of `From` impls to keep the translation plain.
impl Txn<Success>   { pub fn terminal(self) -> Txn<Terminal> { Txn { id: self.id, data: Terminal::Success(self.data) } } }
impl Txn<Failed>    { pub fn terminal(self) -> Txn<Terminal> { Txn { id: self.id, data: Terminal::Failed(self.data) } } }
impl Txn<Reversed>  { pub fn terminal(self) -> Txn<Terminal> { Txn { id: self.id, data: Terminal::Reversed(self.data) } } }
impl Txn<Abandoned> { pub fn terminal(self) -> Txn<Terminal> { Txn { id: self.id, data: Terminal::Abandoned(self.data) } } }
