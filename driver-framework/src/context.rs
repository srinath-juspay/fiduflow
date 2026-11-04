//! Generic context data structure for flow persistence.
//!
//! This type is domain-agnostic and can be used with any flow.
//! Context data is stored as a JSON value.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

/// A generic context data record for persistence.
///
/// This structure stores cross-segment locals as a JSON value.
/// The actual typed data lives in the domain layer and is
/// serialized/deserialized when storing/loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextData {
    /// The context data as JSON.
    pub data: JsonValue,
}

impl ContextData {
    /// Create a new context data record from a JSON value.
    pub fn new(data: JsonValue) -> Self {
        Self { data }
    }

    /// Create an empty context (null JSON).
    pub fn empty() -> Self {
        Self {
            data: JsonValue::Null,
        }
    }

    /// Check if the context is empty (null or empty object).
    pub fn is_empty(&self) -> bool {
        match &self.data {
            JsonValue::Null => true,
            JsonValue::Object(m) => m.is_empty(),
            _ => false,
        }
    }
}

impl Default for ContextData {
    fn default() -> Self {
        Self::empty()
    }
}
