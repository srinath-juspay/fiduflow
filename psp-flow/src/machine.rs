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
pub const MAX_OPS_RETRIES: u8 = 3;

macro_rules! wait_point {
    ($name:ident { $($field:ident : $ty:ty),* $(,)? }) => {
        #[derive(Clone, Copy)]
        #[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
        pub struct $name { $(pub $field: $ty),* }
    };
}

// Initial state: waiting for payer ID to start the flow
wait_point!(AtInit                 { id: TxnId });
wait_point!(AtGetPaymentIntent     { txn: Txn<Initiated> });
wait_point!(AtShowError            { txn: Txn<Initiated>, reason: Reason });
wait_point!(AtCollectMpin          { txn: Txn<Initiated>, intent: ValidIntent });
// High-value verification wait points
wait_point!(AtAwaitOtp             { txn: Txn<Authenticated>, req: PayRequest });
wait_point!(AtAwaitApproval        { txn: Txn<Authenticated>, req: PayRequest });
wait_point!(AtReqPay               { txn: Txn<Authenticated>, req: PayRequest });
wait_point!(AtNotifyRejected       { txn: Txn<Authenticated>, req: PayRequest, reason: Reason });
wait_point!(AtAwaitCallback        { txn: Txn<Submitted> });
wait_point!(AtNotifySuccess        { txn: Txn<Submitted>, debit_ref: Ref, credit_ref: Ref });
wait_point!(AtNotifyFailed         { txn: Txn<Submitted>, reason: Reason });
wait_point!(AtNotifyProcessing     { txn: Txn<Submitted> });
wait_point!(AtAwaitDeemed          { txn: Txn<Deemed> });
// Ops escalation wait point with retry counter
wait_point!(AtEscalateToOps        { txn: Txn<Deemed>, retries_remaining: u8 });
wait_point!(AtNotifyConfirmed      { txn: Txn<Deemed>, credit_ref: Ref });
wait_point!(AtNotifyDeemedReversed { txn: Txn<Deemed> });
wait_point!(AtNotifyReversalStart  { txn: Txn<ReversalPending> });
wait_point!(AtAwaitForcedReversal  { txn: Txn<ReversalPending> });
wait_point!(AtNotifyReversed       { txn: Txn<ReversalPending> });

/// The bookmark: one variant per wait point.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum PayerPay {
    AtInit(AtInit),
    AtGetPaymentIntent(AtGetPaymentIntent),
    AtShowError(AtShowError),
    AtCollectMpin(AtCollectMpin),
    // High-value verification
    AtAwaitOtp(AtAwaitOtp),
    AtAwaitApproval(AtAwaitApproval),
    AtReqPay(AtReqPay),
    AtNotifyRejected(AtNotifyRejected),
    AtAwaitCallback(AtAwaitCallback),
    AtNotifySuccess(AtNotifySuccess),
    AtNotifyFailed(AtNotifyFailed),
    AtNotifyProcessing(AtNotifyProcessing),
    AtAwaitDeemed(AtAwaitDeemed),
    // Ops escalation
    AtEscalateToOps(AtEscalateToOps),
    AtNotifyConfirmed(AtNotifyConfirmed),
    AtNotifyDeemedReversed(AtNotifyDeemedReversed),
    AtNotifyReversalStart(AtNotifyReversalStart),
    AtAwaitForcedReversal(AtAwaitForcedReversal),
    AtNotifyReversed(AtNotifyReversed),
}

/// The framework's `Next`, instantiated for this flow.
pub type Next = driver_framework::Next<PayerPay, Txn<Terminal>>;
pub use driver_framework::WaitPoint;

/// Entry: Creates initial state waiting for payer ID.
pub fn initial(id: TxnId) -> PayerPay {
    PayerPay::AtInit(AtInit { id })
}

/// Legacy entry: `PayerPSP_Pay(payer_id)` - creates flow with payer already set.
#[deprecated(note = "Use initial() + Init response instead")]
pub fn start(id: TxnId, payer: PayerId) -> PayerPay {
    PayerPay::AtGetPaymentIntent(AtGetPaymentIntent { txn: Txn::new(id, payer) })
}

// ---- init ----------------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtInit {
    type Event = Init;
    fn event(&self) -> Init {
        Init { id: self.id }
    }
    fn resume(self, payer: <Init as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Wait(PayerPay::AtGetPaymentIntent(AtGetPaymentIntent {
            txn: Txn::new(self.id, payer),
        }))
    }
}

// ---- collect_intent ------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtGetPaymentIntent {
    type Event = GetPaymentIntent;
    fn event(&self) -> GetPaymentIntent {
        GetPaymentIntent { payer: self.txn.data.payer }
    }
    fn resume(self, raw: <GetPaymentIntent as Event>::Response, _cfg: &FlowConfig) -> Next {
        match validate_pay_intent(self.txn.data.payer, raw) {
            Err(reason) => Next::Wait(PayerPay::AtShowError(AtShowError { txn: self.txn, reason })),
            Ok(intent) => Next::Wait(PayerPay::AtCollectMpin(AtCollectMpin { txn: self.txn, intent })),
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtShowError {
    type Event = ShowError;
    fn event(&self) -> ShowError {
        ShowError { reason: self.reason }
    }
    fn resume(self, _r: <ShowError as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Done(self.txn.abandon().terminal())
    }}

// ---- authenticate_payer, then Tau(build_req_pay) --------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtCollectMpin {
    type Event = CollectMpin;
    fn event(&self) -> CollectMpin {
        CollectMpin { payer: self.txn.data.payer, intent: self.intent }
    }
    fn resume(self, mpin: <CollectMpin as Event>::Response, cfg: &FlowConfig) -> Next {
        let txn = self.txn.authenticate(self.intent, mpin);
        let req = txn.build_req_pay();
        let is_high_value = req.amount.0 > HIGH_VALUE_THRESHOLD;

        match (is_high_value, cfg.high_value_mode) {
            (true, HighValueMode::RequireOtp) => {
                Next::Wait(PayerPay::AtAwaitOtp(AtAwaitOtp { txn, req }))
            }
            (true, HighValueMode::RequireApproval) => {
                Next::Wait(PayerPay::AtAwaitApproval(AtAwaitApproval { txn, req }))
            }
            _ => Next::Wait(PayerPay::AtReqPay(AtReqPay { txn, req })),
        }
    }}

// ---- high-value verification ---------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtAwaitOtp {
    type Event = AwaitOtp;
    fn event(&self) -> AwaitOtp {
        AwaitOtp { payer: self.req.payer }
    }
    fn resume(self, result: <AwaitOtp as Event>::Response, _cfg: &FlowConfig) -> Next {
        match result {
            OtpResult::Valid => Next::Wait(PayerPay::AtReqPay(AtReqPay { txn: self.txn, req: self.req })),
            OtpResult::Invalid | OtpResult::Timeout => {
                Next::Wait(PayerPay::AtNotifyRejected(AtNotifyRejected {
                    txn: self.txn,
                    req: self.req,
                    reason: Reason::RiskRejected,
                }))
            }
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtAwaitApproval {
    type Event = AwaitApproval;
    fn event(&self) -> AwaitApproval {
        AwaitApproval { req: self.req }
    }
    fn resume(self, result: <AwaitApproval as Event>::Response, _cfg: &FlowConfig) -> Next {
        match result {
            ApprovalResult::Approved => Next::Wait(PayerPay::AtReqPay(AtReqPay { txn: self.txn, req: self.req })),
            ApprovalResult::Rejected | ApprovalResult::Timeout => {
                Next::Wait(PayerPay::AtNotifyRejected(AtNotifyRejected {
                    txn: self.txn,
                    req: self.req,
                    reason: Reason::RiskRejected,
                }))
            }
        }
    }}

// ---- submit_to_npci -------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtReqPay {
    type Event = ReqPay;
    fn event(&self) -> ReqPay {
        ReqPay { req: self.req }
    }
    fn resume(self, resp: <ReqPay as Event>::Response, _cfg: &FlowConfig) -> Next {
        match resp {
            ReqPayResponse::Rejected { reason } => {
                Next::Wait(PayerPay::AtNotifyRejected(AtNotifyRejected { txn: self.txn, req: self.req, reason }))
            }
            ReqPayResponse::Ack => Next::Wait(PayerPay::AtAwaitCallback(AtAwaitCallback { txn: self.txn.submit(self.req) })),
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyRejected {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Failed(self.reason) }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Done(self.txn.reject(self.req, self.reason).terminal())
    }}

// ---- npci_callback --------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtAwaitCallback {
    type Event = AwaitCallback;
    fn event(&self) -> AwaitCallback {
        AwaitCallback { timeout_secs: CALLBACK_TIMEOUT_SECS }
    }
    fn resume(self, cb: <AwaitCallback as Event>::Response, _cfg: &FlowConfig) -> Next {
        match cb {
            Callback::Success { debit_ref, credit_ref } => {
                Next::Wait(PayerPay::AtNotifySuccess(AtNotifySuccess { txn: self.txn, debit_ref, credit_ref }))
            }
            Callback::Failed { reason } => Next::Wait(PayerPay::AtNotifyFailed(AtNotifyFailed { txn: self.txn, reason })),
            Callback::Deemed => Next::Wait(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn: self.txn })),
            Callback::Timeout => Next::Wait(PayerPay::AtNotifyProcessing(AtNotifyProcessing { txn: self.txn })),
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifySuccess {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Successful }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Done(self.txn.succeed(self.debit_ref, self.credit_ref).terminal())
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyFailed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Failed(self.reason) }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Done(self.txn.fail(self.reason).terminal())
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyProcessing {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Processing }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Wait(PayerPay::AtAwaitDeemed(AtAwaitDeemed { txn: self.txn.deem() }))
    }}

// ---- deemed_resolution ----------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtAwaitDeemed {
    type Event = AwaitDeemedResolution;
    fn event(&self) -> AwaitDeemedResolution {
        AwaitDeemedResolution { deadline_days: DEEMED_DEADLINE_DAYS }
    }
    fn resume(self, res: <AwaitDeemedResolution as Event>::Response, cfg: &FlowConfig) -> Next {
        match res {
            Resolution::DeemedSuccess { credit_ref } => {
                Next::Wait(PayerPay::AtNotifyConfirmed(AtNotifyConfirmed { txn: self.txn, credit_ref }))
            }
            Resolution::DeemedFailed => Next::Wait(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed { txn: self.txn })),
            Resolution::DeadlineExpired => match cfg.deemed_policy {
                DeemedPolicy::WaitForResolution => {
                    Next::Wait(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart { txn: self.txn.escalate_reversal() }))
                }
                DeemedPolicy::AutoReverse => {
                    Next::Wait(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed { txn: self.txn }))
                }
                DeemedPolicy::Escalate => {
                    Next::Wait(PayerPay::AtEscalateToOps(AtEscalateToOps {
                        txn: self.txn,
                        retries_remaining: MAX_OPS_RETRIES,
                    }))
                }
            }
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtEscalateToOps {
    type Event = EscalateToOps;
    fn event(&self) -> EscalateToOps {
        EscalateToOps { txn_id: self.txn.id }
    }
    fn resume(self, decision: <EscalateToOps as Event>::Response, _cfg: &FlowConfig) -> Next {
        match decision {
            OpsDecision::Reverse => {
                // Ops decided to initiate reversal
                Next::Wait(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart { txn: self.txn.escalate_reversal() }))
            }
            OpsDecision::Retry => {
                if self.retries_remaining > 0 {
                    // Ops wants to retry - loop back with decremented counter
                    Next::Wait(PayerPay::AtEscalateToOps(AtEscalateToOps {
                        txn: self.txn,
                        retries_remaining: self.retries_remaining - 1,
                    }))
                } else {
                    // No more retries allowed, proceed to reversal
                    Next::Wait(PayerPay::AtNotifyReversalStart(AtNotifyReversalStart { txn: self.txn.escalate_reversal() }))
                }
            }
            OpsDecision::Fail => {
                // Ops decided to mark as failed/reversed immediately
                Next::Wait(PayerPay::AtNotifyDeemedReversed(AtNotifyDeemedReversed { txn: self.txn }))
            }
        }
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyConfirmed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Confirmed }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Done(self.txn.confirm(self.credit_ref).terminal())
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyDeemedReversed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Reversed }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Done(self.txn.reverse().terminal())
    }}

// ---- forced_reversal ------------------------------------------------------------------

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyReversalStart {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::ReversalInProgress }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Wait(PayerPay::AtAwaitForcedReversal(AtAwaitForcedReversal { txn: self.txn }))
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtAwaitForcedReversal {
    type Event = AwaitForcedReversal;
    fn event(&self) -> AwaitForcedReversal {
        AwaitForcedReversal { deadline_days: FORCED_REVERSAL_DEADLINE_DAYS }
    }
    fn resume(self, _r: <AwaitForcedReversal as Event>::Response, _cfg: &FlowConfig) -> Next {
        Next::Wait(PayerPay::AtNotifyReversed(AtNotifyReversed { txn: self.txn }))
    }}

impl WaitPoint<PayerPay, Txn<Terminal>, FlowConfig> for AtNotifyReversed {
    type Event = Notify;
    fn event(&self) -> Notify {
        Notify { status: Status::Reversed }
    }
    fn resume(self, _r: <Notify as Event>::Response, _cfg: &FlowConfig) -> Next {
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
        PayerPay::AtInit(w) => AnyEvent::Init(w.event()),
        PayerPay::AtGetPaymentIntent(w) => AnyEvent::GetPaymentIntent(w.event()),
        PayerPay::AtShowError(w) => AnyEvent::ShowError(w.event()),
        PayerPay::AtCollectMpin(w) => AnyEvent::CollectMpin(w.event()),
        // High-value verification
        PayerPay::AtAwaitOtp(w) => AnyEvent::AwaitOtp(w.event()),
        PayerPay::AtAwaitApproval(w) => AnyEvent::AwaitApproval(w.event()),
        PayerPay::AtReqPay(w) => AnyEvent::ReqPay(w.event()),
        PayerPay::AtNotifyRejected(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtAwaitCallback(w) => AnyEvent::AwaitCallback(w.event()),
        PayerPay::AtNotifySuccess(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyFailed(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyProcessing(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtAwaitDeemed(w) => AnyEvent::AwaitDeemedResolution(w.event()),
        // Ops escalation
        PayerPay::AtEscalateToOps(w) => AnyEvent::EscalateToOps(w.event()),
        PayerPay::AtNotifyConfirmed(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyDeemedReversed(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtNotifyReversalStart(w) => AnyEvent::Notify(w.event()),
        PayerPay::AtAwaitForcedReversal(w) => AnyEvent::AwaitForcedReversal(w.event()),
        PayerPay::AtNotifyReversed(w) => AnyEvent::Notify(w.event()),
    }
}

/// Decode the response for the bookmark's event and resume with the given config.
/// The only place a mismatch can occur; inside `resume` the types make it impossible.
pub fn deliver(s: PayerPay, r: AnyResponse, cfg: &FlowConfig) -> Result<Next, Mismatch> {
    Ok(match (s, r) {
        (PayerPay::AtInit(w), AnyResponse::Init(payer)) => w.resume(payer, cfg),
        (PayerPay::AtGetPaymentIntent(w), AnyResponse::Intent(raw)) => w.resume(raw, cfg),
        (PayerPay::AtShowError(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtCollectMpin(w), AnyResponse::Mpin(mpin)) => w.resume(mpin, cfg),
        // High-value verification
        (PayerPay::AtAwaitOtp(w), AnyResponse::Otp(result)) => w.resume(result, cfg),
        (PayerPay::AtAwaitApproval(w), AnyResponse::Approval(result)) => w.resume(result, cfg),
        (PayerPay::AtReqPay(w), AnyResponse::ReqPay(resp)) => w.resume(resp, cfg),
        (PayerPay::AtNotifyRejected(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtAwaitCallback(w), AnyResponse::Callback(cb)) => w.resume(cb, cfg),
        (PayerPay::AtNotifySuccess(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtNotifyFailed(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtNotifyProcessing(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtAwaitDeemed(w), AnyResponse::Resolution(res)) => w.resume(res, cfg),
        // Ops escalation
        (PayerPay::AtEscalateToOps(w), AnyResponse::OpsDecision(decision)) => w.resume(decision, cfg),
        (PayerPay::AtNotifyConfirmed(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtNotifyDeemedReversed(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtNotifyReversalStart(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtAwaitForcedReversal(w), AnyResponse::Unit) => w.resume((), cfg),
        (PayerPay::AtNotifyReversed(w), AnyResponse::Unit) => w.resume((), cfg),
        _ => return Err(Mismatch::ResponseMismatch),
    })
}
