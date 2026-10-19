//! Values and the two pure functions. No strings anywhere: reasons and statuses are enums.

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

/// Pure decision. `Tau(validate_pay_intent(intent))` in the document.
pub fn validate_pay_intent(payer: PayerId, raw: RawIntent) -> Result<ValidIntent, Reason> {
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
