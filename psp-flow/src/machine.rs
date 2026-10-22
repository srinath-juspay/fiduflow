//! The flow as a machine, with each wait point typed by its event.
//!
//! One struct per `Vis` node of the document's inlined tree, carrying the locals live
//! there. Each has `event()`, what it waits on, and `resume(r)`, where `r` has exactly the
//! response type of that event: `<AwaitCallback as Event>::Response` is `Callback`, so
//! `AtAwaitCallback::resume` cannot be handed anything else. This is what the `#[flow]`
//! macro would generate from the async-style body.
//!
//! `PayerPay` wraps the wait points so a stored instance is one value. `event` and
//! `deliver` are the runtime boundary: `deliver` decodes an `AnyResponse` for the
//! bookmark's event and calls `resume`; it is the only place a mismatch can occur.

use crate::entity::*;
use crate::events::*;
use crate::values::*;

pub const CALLBACK_TIMEOUT_SECS: u64 = 30;
pub const DEEMED_DEADLINE_DAYS: u64 = 3;
pub const FORCED_REVERSAL_DEADLINE_DAYS: u64 = 5;

macro_rules! wait_point {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy)]
        #[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
        pub struct $name { $(pub $field: $ty),* }
    };
}

wait_point!(AtGetPaymentIntent     { txn: Txn<Initiated> });
wait_point!(AtShowError            { txn: Txn<Initiated>, reason: Reason });
wait_point!(AtCollectMpin          { txn: Txn<Initiated>, intent: ValidIntent });
wait_point!(AtReqPay               { txn: Txn<Authenticated>, req: PayRequest });
wait_point!(AtNotifyRejected       { txn: Txn<Authenticated>, req: PayRequest, reason: Reason });
wait_point!(AtAwaitCallback        { txn: Txn<Submitted> });
wait_point!(AtNotifySuccess        { txn: Txn<Submitted>, debit_ref: Ref, credit_ref: Ref });
wait_point!(AtNotifyFailed         { txn: Txn<Submitted>, reason: Reason });
wait_point!(AtNotifyProcessing     { txn: Txn<Submitted> });
wait_point!(AtAwaitDeemed          { txn: Txn<Deemed> });
wait_point!(AtNotifyConfirmed      { txn: Txn<Deemed>, credit_ref: Ref });
wait_point!(AtNotifyDeemedReversed { txn: Txn<Deemed> });
wait_point!(AtNotifyReversalStart  { txn: Txn<ReversalPending> });
wait_point!(AtAwaitForcedReversal  { txn: Txn<ReversalPending> });
wait_point!(AtNotifyReversed       { txn: Txn<ReversalPending> });

/// The bookmark: one variant per wait point.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum PayerPay {
    AtGetPaymentIntent(AtGetPaymentIntent),
    AtShowError(AtShowError),
    AtCollectMpin(AtCollectMpin),
    AtReqPay(AtReqPay),
    AtNotifyRejected(AtNotifyRejected),
    AtAwaitCallback(AtAwaitCallback),
    AtNotifySuccess(AtNotifySuccess),
    AtNotifyFailed(AtNotifyFailed),
    AtNotifyProcessing(AtNotifyProcessing),
    AtAwaitDeemed(AtAwaitDeemed),
    AtNotifyConfirmed(AtNotifyConfirmed),
    AtNotifyDeemedReversed(AtNotifyDeemedReversed),
    AtNotifyReversalStart(AtNotifyReversalStart),
    AtAwaitForcedReversal(AtAwaitForcedReversal),
    AtNotifyReversed(AtNotifyReversed),
}

/// The framework's `Next`, instantiated for this flow.
pub type Next = driver_framework::Next<PayerPay, Txn<Terminal>>;
pub use driver_framework::WaitPoint;

/// Entry: `PayerPSP_Pay(payer_id)`.
pub fn start(id: TxnId, payer: PayerId) -> PayerPay {
    PayerPay::AtGetPaymentIntent(AtGetPaymentIntent { txn: Txn::new(id, payer) })
}

// ---- collect_intent ------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>> for AtGetPaymentIntent {
    type Event = GetPaymentIntent;
    fn event(&self) -> GetPaymentIntent {
        GetPaymentIntent { payer: self.txn.data.payer }
    }
    fn resume(self, raw: <GetPaymentIntent as Event>::Response) -> Next {
        match validate_pay_intent(self.txn.data.payer, raw) {
            Err(reason) => Next::Wait(PayerPay::AtShowError(AtShowError { txn: self.txn, reason })),
            Ok(intent) => Next::Wait(PayerPay::AtCollectMpin(AtCollectMpin { txn: self.txn, intent })),
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtShowError {
    type Event = ShowError;
    fn event(&self) -> ShowError {
        ShowError { reason: self.reason }
    }
    fn resume(self, _r: <ShowError as Event>::Response) -> Next {
        Next::Done(self.txn.abandon().terminal())
    }}

// ---- authenticate_payer, then Tau(build_req_pay) --------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>> for AtCollectMpin {
    type Event = CollectMpin;
    fn event(&self) -> CollectMpin {
        CollectMpin { payer: self.txn.data.payer, intent: self.intent }
    }
    fn resume(self, mpin: <CollectMpin as Event>::Response) -> Next {
        let txn = self.txn.authenticate(self.intent, mpin);
        let req = txn.build_req_pay();
        Next::Wait(PayerPay::AtReqPay(AtReqPay { txn, req }))
    }}

// ---- submit_to_npci -------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>> for AtReqPay {
    type Event = ReqPay;
    fn event(&self) -> ReqPay {
        ReqPay { req: self.req }
    }
    fn resume(self, resp: <ReqPay as Event>::Response) -> Next {
        match resp {
            ReqPayResponse::Rejected { reason } => {
                Next::Wait(PayerPay::AtNotifyRejected(AtNotifyRejected { txn: self.txn, req: self.req, reason }))
            }
            ReqPayResponse::Ack => Next::Wait(PayerPay::AtAwaitCallback(AtAwaitCallback { txn: self.txn.submit(self.req) })),
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyRejected {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Failed(self.reason) }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Done(self.txn.reject(self.req, self.reason).terminal())
    }}

// ---- npci_callback --------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>> for AtAwaitCallback {
    type Event = AwaitCallback;
    fn event(&self) -> AwaitCallback {
        AwaitCallback { timeout_secs: CALLBACK_TIMEOUT_SECS }
    }
    fn resume(self, cb: <AwaitCallback as Event>::Response) -> Next {
        match cb {
            Callback::Success { debit_ref, credit_ref } => {
                Next::Wait(PayerPay::AtNotifySuccess(AtNotifySuccess { txn: self.txn, debit_ref, credit_ref }))
            }
            Callback::Failed { reason } => Next::Wait(PayerPay::AtNotifyFailed(AtNotifyFailed { txn: self.txn, reason })),
            Callback::Deemed => Next::Wait(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn: self.txn })),
            Callback::Timeout => Next::Wait(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn: self.txn })),
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifySuccess {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Successful }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Done(self.txn.succeed(self.debit_ref, self.credit_ref).terminal())
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyFailed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Failed(self.reason) }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Done(self.txn.fail(self.reason).terminal())
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyProcessing {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Processing }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Wait(PayerPay::AtAwaitDeemed(AtAwaitDeemed { txn: self.txn.deem() }))
    }}

// ---- deemed_resolution ----------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>> for AtAwaitDeemed {
    type Event = AwaitDeemedResolution;
    fn event(&self) -> AwaitDeemedResolution {
        AwaitDeemedResolution { deadline_days: DEEMED_DEADLINE_DAYS }
    }
    fn resume(self, res: <AwaitDeemedResolution as Event>::Response) -> Next {
        match res {
            Resolution::DeemedSuccess { credit_ref } => {
                Next::Wait(PayerPay::AtNotifyConfirmed(AtNotifyConfirmed { txn: self.txn, credit_ref }))
            }
            Resolution::DeemedFailed => Next::Wait(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed { txn: self.txn })),
            Resolution::DeadlineExpired => {
                Next::Wait(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart { txn: self.txn.escalate_reversal() }))
            }
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyConfirmed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Confirmed }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Done(self.txn.confirm(self.credit_ref).terminal())
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyDeemedReversed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Reversed }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Done(self.txn.reverse().terminal())
    }}

// ---- forced_reversal ------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyReversalStart {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::ReversalInProgress }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Wait(PayerPay::AtAwaitForcedReversal(AtAwaitForcedReversal { txn: self.txn }))
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtAwaitForcedReversal {
    type Event = AwaitForcedReversal;
    fn event(&self) -> AwaitForcedReversal {
        AwaitForcedReversal { deadline_days: FORCED_REVERSAL_DEADLINE_DAYS }
    }
    fn resume(self, _r: <AwaitForcedReversal as Event>::Response) -> Next {
        Next::Wait(PayerPay::AtNotifyReversed(AtNotifyReversed { txn: self.txn }))
    }}

impl WaitPoint<PayerPay, Txn<Terminal>> for AtNotifyReversed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Reversed }
    }
    fn resume(self, _r: <Notify as Event>::Response) -> Next {
        Next::Done(self.txn.reversed().terminal())
    }}

// ---- Runtime boundary -----------------------------------------------------------------

/// The response did not decode as the bookmark's event expects.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Mismatch {
    ResponseMismatch,
}

/// What a bookmark is waiting on, as data.
pub fn event(s: &PayerPay) -> AnyEvent {
    match s {
        PayerPay::AtGetPaymentIntent(w) => AnyEvent::GetPaymentIntent(w.event()),
        PayerPay::AtShowError(w) => AnyEvent::ShowError(w.event()),
        PayerPay::AtCollectMpin(w) => AnyEvent::CollectMpin(w.event()),
        PayerPay::AtReqPay(w) => AnyEvent::ReqPay(w.event()),
        PayerPay::AtNotifyRejected(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtAwaitCallback(w) => AnyEvent::AwaitCallback(w.event()),
        PayerPay::AtNotifySuccess(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyFailed(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyProcessing(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtAwaitDeemed(w) => AnyEvent::AwaitDeemedResolution(w.event()),
        PayerPay::AtNotifyConfirmed(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyDeemedReversed(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyReversalStart(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtAwaitForcedReversal(w) => AnyEvent::AwaitForcedReversal(w.event()),
        PayerPay::AtNotifyReversed(w) => AnyEvent::Notify(w.event()),
    }
}

/// Decode the response for the bookmark's event and resume. The only place a mismatch
/// can occur; inside `resume` the types make it impossible.
pub fn deliver(s: PayerPay, r: AnyResponse) -> Result<Next, Mismatch> {
    Ok(match (s, r) {
        (PayerPay::AtGetPaymentIntent(w), AnyResponse::Intent(raw)) => w.resume(raw),
        (PayerPay::AtShowError(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtCollectMpin(w), AnyResponse::Mpin(mpin)) => w.resume(mpin),
        (PayerPay::AtReqPay(w), AnyResponse::ReqPay(resp)) => w.resume(resp),
        (PayerPay::AtNotifyRejected(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtAwaitCallback(w), AnyResponse::Callback(cb)) => w.resume(cb),
        (PayerPay::AtNotifySuccess(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtNotifyFailed(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtNotifyProcessing(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtAwaitDeemed(w), AnyResponse::Resolution(res)) => w.resume(res),
        (PayerPay::AtNotifyConfirmed(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtNotifyDeemedReversed(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtNotifyReversalStart(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtAwaitForcedReversal(w), AnyResponse::Unit) => w.resume(()),
        (PayerPay::AtNotifyReversed(w), AnyResponse::Unit) => w.resume(()),
        _ => return Err(Mismatch::ResponseMismatch),
    })
}
