//! The flow runtime: core abstractions for flow machines.
//!
//! This module provides:
//! - `Flow` trait: defines what a flow is
//! - `Next` enum: flow progression
//! - `WaitPoint` trait: interaction points
//! - `Event` trait: request-response typing

#![no_std]
#![forbid(unsafe_code)]

pub mod machine;

pub use machine::{Event, Flow, Next, Refused, Stored, WaitPoint};
