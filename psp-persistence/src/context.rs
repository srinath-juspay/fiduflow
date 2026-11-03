//! Cross-segment context contracts.
//!
//! This module defines what each segment reads and writes from a shared context.
//! Currently the context is empty (no cross-segment locals needed), but the structure
//! allows for extension when segments need to pass data to later segments.

/// The segment of the flow, used for querying transactions by their current position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Segment {
    Initiated,
    Authenticated,
    Submitted,
    Deemed,
    ReversalPending,
}

impl Segment {
    /// Get the segment name as a string (for database storage).
    pub fn as_str(&self) -> &'static str {
        match self {
            Segment::Initiated => "Initiated",
            Segment::Authenticated => "Authenticated",
            Segment::Submitted => "Submitted",
            Segment::Deemed => "Deemed",
            Segment::ReversalPending => "ReversalPending",
        }
    }

    /// Parse a segment from a string.
    pub fn from_str(s: &str) -> Option<Segment> {
        match s {
            "Initiated" => Some(Segment::Initiated),
            "Authenticated" => Some(Segment::Authenticated),
            "Submitted" => Some(Segment::Submitted),
            "Deemed" => Some(Segment::Deemed),
            "ReversalPending" => Some(Segment::ReversalPending),
            _ => None,
        }
    }
}

// Per-segment contracts (marker types for documentation and future compile-time checks)

/// What the Initiated segment reads from context.
pub struct InitiatedReads;

/// What the Initiated segment writes to context.
pub struct InitiatedWrites;

/// What the Authenticated segment reads from context.
pub struct AuthenticatedReads;

/// What the Authenticated segment writes to context.
pub struct AuthenticatedWrites;

/// What the Submitted segment reads from context.
pub struct SubmittedReads;

/// What the Submitted segment writes to context.
pub struct SubmittedWrites;

/// What the Deemed segment reads from context.
pub struct DeemedReads;

/// What the Deemed segment writes to context.
pub struct DeemedWrites;

/// What the ReversalPending segment reads from context.
pub struct ReversalPendingReads;

/// What the ReversalPending segment writes to context.
pub struct ReversalPendingWrites;

/// Storage for cross-segment locals.
///
/// This struct holds values that need to be passed between segments.
/// Currently empty but can be extended as needed.
///
/// # Example Extension
///
/// ```ignore
/// pub struct FlowContext {
///     pub high_value_transaction: Option<bool>,
///     pub risk_score: Option<u32>,
/// }
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FlowContext {
    // Add cross-segment locals here as needed.
    // The Default derive provides an empty context.
}

impl FlowContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }
}
