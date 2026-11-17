//! The flow runtime: core abstractions for flow machines.
//!
//! This module provides:
//! - `Flow` trait: defines what a flow is
//! - `Next` enum: flow progression
//! - `WaitPoint` trait: interaction points
//! - `Event` trait: request-response typing
//! - `MessageRecord` (feature: persist): generic message storage with JSON
//! - `ContextData` (feature: persist): generic context storage with JSON
//! - `Persist` trait (feature: persist): decompose/reconstruct for storage
//! - `FlowStore` trait (feature: persist): database storage interface
//! - `Driver` (feature: persist): generic flow driver

#![cfg_attr(not(feature = "persist"), no_std)]
#![forbid(unsafe_code)]

pub mod machine;

#[cfg(feature = "persist")]
pub mod context;
#[cfg(feature = "persist")]
pub mod message;
#[cfg(feature = "persist")]
pub mod persist;

pub use machine::{Event, Flow, Next, Refused, Stored, WaitPoint};

#[cfg(feature = "persist")]
pub use context::ContextData;
#[cfg(feature = "persist")]
pub use message::{Actor, MessageRecord, MessageStatus, Method};
#[cfg(feature = "persist")]
pub use persist::{DeliverResult, Driver, DriverError, FlowStore, Persist, ReconstructError};
