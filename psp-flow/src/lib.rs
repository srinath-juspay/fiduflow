//! UPI PSP "Pay (push)" flow as a first-order interaction machine.
//!
//! This crate is what the `#[flow]` macro would *generate* from the async-style body.
//! It is written by hand for the spike so it can be fed to Charon/Aeneas unchanged:
//! `no_std`, no closures, no `dyn`, no strings, no async, no derives beyond Clone/Copy.
//!
//! Layers:
//! * `values`  — refined value types and the two pure functions of the flow.
//! * `entity`  — the `UpiTxn` entity: one struct per state, `Txn<S>` typestate, transitions.
//! * `events`  — interactions, each co-defined with its response type.
//! * `machine` — the flow: one typed wait point per `Vis` node with `event`/`resume`, plus the boundary `event`/`deliver`.
//! * `persist` — (feature: persist) typed event request/response for storage.

#![cfg_attr(not(feature = "persist"), no_std)]
#![forbid(unsafe_code)]

pub mod values;
pub mod entity;
pub mod events;
pub mod machine;

#[cfg(feature = "persist")]
pub mod persist;

pub use values::*;
pub use entity::*;
pub use events::*;
pub use machine::*;

#[cfg(feature = "persist")]
pub use persist::*;
