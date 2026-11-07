//! Host business fields are independent of the managed kind-only summary.
use super::*;
use serde_json::Value;

impl Input {
    pub(super) fn event_schema() -> Value {
        event_projection::batch_schema()
    }
    pub(super) fn event_value(&self) -> Value {
        event_projection::batch_value(&self.0)
    }
}
impl Output {
    pub(super) fn event_schema() -> Value {
        event_projection::receipt_schema()
    }
    pub(super) fn event_value(&self) -> Value {
        event_projection::receipt_value(&self.0)
    }
}
