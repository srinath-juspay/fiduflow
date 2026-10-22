//! Interactions, co-defined with their responses.
//!
//! Every event is a struct implementing `Event`, whose associated `Response` type is what
//! the outside world answers. A wait point's `resume` takes exactly that type (see
//! `machine.rs`), so a mismatched response is unrepresentable inside the machine.
//!
//! `AnyEvent` and `AnyResponse` exist only at the runtime boundary, where a stored instance
//! must say what it waits on and a response arrives as data. The DSL would generate them.

use crate::values::*;

pub use driver_framework::Event;

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Party { User, Npci }

// ---- User interactions --------------------------------------------------------------

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct GetPaymentIntent { pub payer: PayerId }
impl Event for GetPaymentIntent { type Response = RawIntent; }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct ShowError { pub reason: Reason }
impl Event for ShowError { type Response = (); }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct CollectMpin { pub payer: PayerId, pub intent: ValidIntent }
impl Event for CollectMpin { type Response = Mpin; }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Notify { pub status: Status }
impl Event for Notify { type Response = (); }

// ---- NPCI interactions --------------------------------------------------------------

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct ReqPay { pub req: PayRequest }
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum ReqPayResponse { Ack, Rejected { reason: Reason } }
impl Event for ReqPay { type Response = ReqPayResponse; }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct AwaitCallback { pub timeout_secs: u64 }
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Callback {
    Success { debit_ref: Ref, credit_ref: Ref },
    Failed { reason: Reason },
    Deemed,
    Timeout,
}
impl Event for AwaitCallback { type Response = Callback; }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct AwaitDeemedResolution { pub deadline_days: u64 }
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Resolution {
    DeemedSuccess { credit_ref: Ref },
    DeemedFailed,
    DeadlineExpired,
}
impl Event for AwaitDeemedResolution { type Response = Resolution; }

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct AwaitForcedReversal { pub deadline_days: u64 }
impl Event for AwaitForcedReversal { type Response = (); }

// ---- Runtime boundary ---------------------------------------------------------------

/// Any event, as data: what a stored instance says it is waiting on.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum AnyEvent {
    GetPaymentIntent(GetPaymentIntent),
    ShowError(ShowError),
    CollectMpin(CollectMpin),
    Notify(Notify),
    ReqPay(ReqPay),
    AwaitCallback(AwaitCallback),
    AwaitDeemedResolution(AwaitDeemedResolution),
    AwaitForcedReversal(AwaitForcedReversal),
}

/// Any response, as data: what arrives from the outside world before it is decoded for
/// the bookmark's event.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum AnyResponse {
    Intent(RawIntent),
    Mpin(Mpin),
    Unit,
    ReqPay(ReqPayResponse),
    Callback(Callback),
    Resolution(Resolution),
}

impl AnyEvent {
    pub fn party(&self) -> Party {
        match self {
            AnyEvent::GetPaymentIntent(_) | AnyEvent::ShowError(_) | AnyEvent::CollectMpin(_) | AnyEvent::Notify(_) => Party::User,
            AnyEvent::ReqPay(_) | AnyEvent::AwaitCallback(_) | AnyEvent::AwaitDeemedResolution(_) | AnyEvent::AwaitForcedReversal(_) => Party::Npci,
        }
    }

    /// P1 talks about these: the user is told something.
    pub fn is_user_notification(&self) -> bool {
        match self {
            AnyEvent::ShowError(_) | AnyEvent::Notify(_) => true,
            _ => false,
        }
    }
}
