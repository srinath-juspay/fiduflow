//! Values and the two pure functions. No strings anywhere: reasons and statuses are enums.

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct PayerId(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct Vpa(pub u64);

/// Paise.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct Amount(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct Mpin(pub u32);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct Ref(pub u64);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "runtime", derive(Debug))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct TxnId(pub u64);

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
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
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct RawIntent {
    pub payee: Vpa,
    pub amount: Amount,
}

/// A validated intent. Only `validate_pay_intent` constructs one.
#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct ValidIntent {
    pub payee: Vpa,
    pub amount: Amount,
}

#[derive(Clone, Copy)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct PayRequest {
    pub payer: PayerId,
    pub payee: Vpa,
    pub amount: Amount,
    pub mpin: Mpin,
}

/// Rs 1,00,000 per-transaction cap, in paise.
pub const MAX_AMOUNT_PAISE: u64 = 10_000_000;

/// High-value transaction threshold, in paise (Rs 1,00,000).
pub const HIGH_VALUE_THRESHOLD: u64 = 100_000;

// ============================================================================
// Flow Configuration
// ============================================================================

/// Policy for handling deemed (uncertain) transaction status.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub enum DeemedPolicy {
    /// Wait for NPCI resolution, then escalate to forced reversal on deadline.
    /// This is the current/default behavior.
    #[default]
    WaitForResolution,
    /// Immediately trigger reversal when deadline expires.
    AutoReverse,
    /// Escalate to ops team for manual decision.
    Escalate,
}

/// Mode for handling high-value transactions.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub enum HighValueMode {
    /// Normal flow, no additional verification.
    #[default]
    Standard,
    /// Require OTP verification after MPIN for high-value transactions.
    RequireOtp,
    /// Require manual approval before NPCI submission for high-value transactions.
    RequireApproval,
}

/// Configuration for flow behavior.
///
/// Passed to `deliver()` on each step. Can change mid-flow.
#[derive(Clone, Copy, Default)]
#[cfg_attr(feature = "runtime", derive(Debug, PartialEq, Eq))]
#[cfg_attr(feature = "persist", derive(serde::Serialize, serde::Deserialize))]
pub struct FlowConfig {
    /// How to handle deemed transaction status.
    pub deemed_policy: DeemedPolicy,
    /// How to handle high-value transactions.
    pub high_value_mode: HighValueMode,
}

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
