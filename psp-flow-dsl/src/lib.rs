//! UPI PSP "Pay (push)" flow, written with the `flow!` macro. The hand-written version of
//! the same flow is the `psp-flow` crate; this crate exists to compare the two.
//!
//! One invocation holds the entity (states), the interactions (each with its response
//! type), the pure decisions, and the body in an async style. The macro lowers it to a
//! first-order machine: one typed wait point per `ask`, the `PayerPay` bookmark enum,
//! `start`, and the boundary `event`/`deliver`. `scripts/expand-psp-dsl.sh` writes the
//! formatted expansion to `generated-src/expanded.rs` (ignored by git) for reading; it is
//! what Charon and Aeneas translate.
//!
//! `#[at(Name)]` names a wait point; without it the name is `At` + the event name.

#![no_std]
#![forbid(unsafe_code)]

pub mod values;
pub use values::*;

flow_macro::flow! {
    payer_pay: Txn<Initiated> -> Txn<Terminal>;

    entity Txn (id: TxnId) {
        Initiated       { payer: PayerId }
        Authenticated   { payer: PayerId, intent: ValidIntent, mpin: Mpin }
        Submitted       { req: PayRequest }
        Deemed          { req: PayRequest }
        ReversalPending { req: PayRequest }
        terminal Success   { req: PayRequest, debit_ref: Option<Ref>, credit_ref: Ref }
        terminal Failed    { req: PayRequest, reason: Reason }
        terminal Reversed  { req: PayRequest }
        terminal Abandoned { payer: PayerId }
    }

    events {
        User.GetPaymentIntent { payer: PayerId } -> RawIntent;
        User.ShowError { reason: Reason } -> ();
        User.CollectMpin { payer: PayerId, intent: ValidIntent } -> Mpin;
        User.Notify { status: Status } -> ();
        Npci.ReqPay { req: PayRequest } -> enum ReqPayResponse { Ack, Rejected { reason: Reason } };
        Npci.AwaitCallback { timeout_secs: u64 } -> enum Callback {
            Success { debit_ref: Ref, credit_ref: Ref },
            Failed { reason: Reason },
            Deemed,
            Timeout
        };
        Npci.AwaitDeemedResolution { deadline_days: u64 } -> enum Resolution {
            DeemedSuccess { credit_ref: Ref },
            DeemedFailed,
            DeadlineExpired
        };
        Npci.AwaitForcedReversal { deadline_days: u64 } -> ();
    }

    decision validate_pay_intent(payer: PayerId, raw: RawIntent) -> Result<ValidIntent, Reason> {
        if raw.amount.0 == 0 || raw.amount.0 > MAX_AMOUNT_PAISE {
            return Err(Reason::InvalidAmount);
        }
        if raw.payee.0 == 0 {
            return Err(Reason::UnknownPayee);
        }
        if raw.payee.0 == payer.0 {
            return Err(Reason::PayeeIsPayer);
        }
        Ok(ValidIntent { payee: raw.payee, amount: raw.amount })
    }

    decision build_req_pay(txn: &Txn<Authenticated>) -> PayRequest {
        PayRequest {
            payer: txn.data.payer,
            payee: txn.data.intent.payee,
            amount: txn.data.intent.amount,
            mpin: txn.data.mpin,
        }
    }

    body(txn) {
        // collect_intent
        let raw = ask(User::GetPaymentIntent { payer: txn.payer }).await;
        let intent = match validate_pay_intent(txn.payer, raw) {
            Err(reason) => {
                ask(User::ShowError { reason }).await;
                return txn.to(Abandoned { payer });
            }
            Ok(v) => v,
        };

        // authenticate_payer, then Tau(build_req_pay)
        let mpin = ask(User::CollectMpin { payer: txn.payer, intent }).await;
        let txn = txn.to(Authenticated { payer, intent, mpin });
        let req = build_req_pay(&txn);

        // submit_to_npci
        let txn = match ask(Npci::ReqPay { req }).await {
            ReqPayResponse::Rejected { reason } => {
                #[at(AtNotifyRejected)]
                ask(User::Notify { status: Status::Failed(reason) }).await;
                return txn.to(Failed { req, reason });
            }
            ReqPayResponse::Ack => txn.to(Submitted { req }),
        };

        // npci_callback
        let txn = match ask(Npci::AwaitCallback { timeout_secs: CALLBACK_TIMEOUT_SECS }).await {
            Callback::Success { debit_ref, credit_ref } => {
                #[at(AtNotifySuccess)]
                ask(User::Notify { status: Status::Successful }).await;
                return txn.to(Success { req, debit_ref: Some(debit_ref), credit_ref });
            }
            Callback::Failed { reason } => {
                #[at(AtNotifyFailed)]
                ask(User::Notify { status: Status::Failed(reason) }).await;
                return txn.to(Failed { req, reason });
            }
            Callback::Deemed | Callback::Timeout => {
                #[at(AtNotifyProcessing)]
                ask(User::Notify { status: Status::Processing }).await;
                txn.to(Deemed { req })
            }
        };

        // deemed_resolution
        #[at(AtAwaitDeemed)]
        let txn = match ask(Npci::AwaitDeemedResolution { deadline_days: DEEMED_DEADLINE_DAYS }).await {
            Resolution::DeemedSuccess { credit_ref } => {
                #[at(AtNotifyConfirmed)]
                ask(User::Notify { status: Status::Confirmed }).await;
                return txn.to(Success { req, debit_ref: None, credit_ref });
            }
            Resolution::DeemedFailed => {
                #[at(AtNotifyDeemedReversed)]
                ask(User::Notify { status: Status::Reversed }).await;
                return txn.to(Reversed { req });
            }
            Resolution::DeadlineExpired => txn.to(ReversalPending { req }),
        };

        // forced_reversal
        #[at(AtNotifyReversalStart)]
        ask(User::Notify { status: Status::ReversalInProgress }).await;
        ask(Npci::AwaitForcedReversal { deadline_days: FORCED_REVERSAL_DEADLINE_DAYS }).await;
        #[at(AtNotifyReversed)]
        ask(User::Notify { status: Status::Reversed }).await;
        txn.to(Reversed { req })
    }
}
