use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// Online facts for one provider attempt. Detail capture is explicitly opt-in.
pub(super) struct ProviderStreamTiming {
    event_count: u64,
    total_size_bytes: u64,
    first_ingress_ms: Option<u64>,
    last_ingress_ms: Option<u64>,
    max_append_delay_ms: Option<u64>,
    event_kind_counts: BTreeMap<&'static str, u64>,
    details: Option<Vec<Value>>,
}

impl ProviderStreamTiming {
    pub(super) fn new(capture_details: bool) -> Self {
        Self {
            event_count: 0,
            total_size_bytes: 0,
            first_ingress_ms: None,
            last_ingress_ms: None,
            max_append_delay_ms: None,
            event_kind_counts: BTreeMap::new(),
            details: capture_details.then(Vec::new),
        }
    }

    pub(super) fn observe(
        &mut self,
        sequence: u64,
        event_kind: &'static str,
        size_bytes: usize,
        ingress_ms: u64,
        runtime_append_ms: u64,
    ) -> Result<()> {
        // Calculate every fallible total before changing the attempt's facts.
        let event_count = self
            .event_count
            .checked_add(1)
            .context("provider stream timing event count overflow")?;
        let size_bytes = u64::try_from(size_bytes)
            .context("provider stream timing event size exceeds integer representation")?;
        let total_size_bytes = self
            .total_size_bytes
            .checked_add(size_bytes)
            .context("provider stream timing total size overflow")?;
        let kind_count = self
            .event_kind_counts
            .get(event_kind)
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .context("provider stream timing kind count overflow")?;

        self.event_count = event_count;
        self.total_size_bytes = total_size_bytes;
        self.event_kind_counts.insert(event_kind, kind_count);
        self.first_ingress_ms = Some(
            self.first_ingress_ms
                .map_or(ingress_ms, |first| first.min(ingress_ms)),
        );
        self.last_ingress_ms = Some(
            self.last_ingress_ms
                .map_or(ingress_ms, |last| last.max(ingress_ms)),
        );
        if let Some(delay) = runtime_append_ms.checked_sub(ingress_ms) {
            self.max_append_delay_ms = Some(
                self.max_append_delay_ms
                    .map_or(delay, |maximum| maximum.max(delay)),
            );
        }
        if let Some(details) = &mut self.details {
            details.push(json!({
                "sequence": sequence,
                "event_kind": event_kind,
                "size_bytes": size_bytes,
                "ingress_ms": ingress_ms,
                "runtime_append_ms": runtime_append_ms,
            }));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn event_count(&self) -> u64 {
        self.event_count
    }

    pub(super) fn first_ingress_ms(&self) -> Option<u64> {
        self.first_ingress_ms
    }

    pub(super) fn max_append_delay_ms(&self) -> Option<u64> {
        self.max_append_delay_ms
    }

    pub(super) fn summary(&self) -> Value {
        json!({
            "schema_version": 1,
            "capture_mode": if self.details.is_some() { "detailed" } else { "summary" },
            "event_count": self.event_count,
            "total_size_bytes": self.total_size_bytes,
            "first_ingress_ms": self.first_ingress_ms,
            "last_ingress_ms": self.last_ingress_ms,
            "max_append_delay_ms": self.max_append_delay_ms,
            "event_kind_counts": self.event_kind_counts,
        })
    }

    pub(super) fn into_details(self) -> Option<Value> {
        self.details.map(Value::Array)
    }
}

#[cfg(test)]
#[path = "_tests/stream_timing.rs"]
mod tests;
