//! What the runtime needs to know about a flow, and nothing else.
//!
//! A flow is a first-order machine: a bookmark type (`State`), a terminal value (`Done`),
//! the interaction a bookmark waits on (`event`), and the pure function that consumes a
//! response (`step`). The `#[flow]` macro would implement `Flow` for a generated machine;
//! the application crate implements it by hand for its flow.

use core::fmt::Debug;

pub trait Flow {
    /// Stable name, used to key instances in storage.
    const NAME: &'static str;
    /// The bookmark: one variant per wait point, carrying the locals live there.
    type State: Copy + Debug;
    /// What a finished instance leaves behind (typically the terminal entity).
    type Done: Copy + Debug;
    /// What a bookmark asks of the outside world.
    type Event: Debug;
    /// What the outside world answers.
    type Response: Debug;

    fn event(s: &Self::State) -> Self::Event;
    /// `None` means the response does not fit what `s` is waiting on; the driver reports
    /// it as a mismatch and leaves the instance unchanged.
    fn step(s: Self::State, r: Self::Response) -> Option<Next<Self::State, Self::Done>>;
}

/// What `step` decides after consuming a response: either the flow pauses again at a new
/// bookmark, which the driver stores before issuing its event, or it is finished and this
/// is the value it leaves behind, which the driver stores and reports to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next<S, D> {
    /// Pause at this bookmark; its `event` is the next interaction to perform.
    Wait(S),
    /// The run is over; no further responses are accepted for the instance.
    Done(D),
}

/// What is stored per instance: a bookmark, or the finished value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored<S, D> {
    Waiting(S),
    Finished(D),
}
