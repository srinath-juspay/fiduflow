//! Values. No strings anywhere: reasons and statuses are enums. The pure decisions live in
//! the `flow!` block in `lib.rs`.

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct PayerId(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Vpa(pub u64);

/// Paise.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Amount(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Mpin(pub u32);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct Ref(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct TxnId(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Reason {
    InvalidAmount,
    UnknownPayee,
    PayeeIsPayer,
    InsufficientFunds,
    BankDeclined,
    RiskRejected,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub enum Status {
    Successful,
    Failed(Reason),
    Processing,
    Confirmed,
    Reversed,
    ReversalInProgress,
}

/// What the user typed. Unvalidated.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct RawIntent {
    pub payee: Vpa,
    pub amount: Amount,
}

/// A validated intent. Only `validate_pay_intent` constructs one.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct ValidIntent {
    pub payee: Vpa,
    pub amount: Amount,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
pub struct PayRequest {
    pub payer: PayerId,
    pub payee: Vpa,
    pub amount: Amount,
    pub mpin: Mpin,
}

/// Rs 1,00,000 per-transaction cap, in paise.
pub const MAX_AMOUNT_PAISE: u64 = 10_000_000;

pub const CALLBACK_TIMEOUT_SECS: u64 = 30;
pub const DEEMED_DEADLINE_DAYS: u64 = 3;
pub const FORCED_REVERSAL_DEADLINE_DAYS: u64 = 5;
