//! What the runtime needs to know about a flow, and nothing else.
//!
//! A flow is a first-order machine: a bookmark type (`State`), a terminal value (`Done`),
//! the interaction a bookmark waits on (`event`), and the pure function that consumes a
//! response (`step`). The `#[flow]` macro would implement `Flow` for a generated machine;
//! here the application crate implements it by hand for its flow.
//!
//! No database, no domain, no IO in this crate.

#![forbid(unsafe_code)]

use core::fmt::Debug;

pub type InstanceId = i64;

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
    /// Why a response can be refused (it does not fit the event).
    type Violation: Debug;

    fn event(s: &Self::State) -> Self::Event;
    fn step(s: Self::State, r: Self::Response) -> Result<Next<Self::State, Self::Done>, Self::Violation>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next<S, D> {
    Wait(S),
    Done(D),
}

/// What is stored per instance: a bookmark, or the finished value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored<S, D> {
    Waiting(S),
    Finished(D),
}
