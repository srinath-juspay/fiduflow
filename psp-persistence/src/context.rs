//! Context types for the PSP flow persistence layer.
//!
//! - `FlowContext`: Typed cross-segment locals (re-exported from psp_flow)
//! - `ContextData`: Generic JSON storage (re-exported from driver_framework)
//! - `Segment`: Flow segment enum for querying by state

// Re-export typed FlowContext from psp_flow
pub use psp_flow::persist::FlowContext;

// Re-export generic ContextData from driver_framework
pub use driver_framework::ContextData;

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
