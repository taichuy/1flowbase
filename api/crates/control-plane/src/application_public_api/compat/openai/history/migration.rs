//! Offline-only conversion of frozen legacy proofs. Not compiled into the default runtime.
//!
//! Ordered source items must reproduce the complete persisted legacy proof before
//! a replacement can be trusted. Progress hashes are for graph/prefix discovery only.
use anyhow::{ensure, Result};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::{append_digest, History, HistoryExt};

#[path = "migration/legacy.rs"]
mod legacy;
#[path = "migration/legacy_v2.rs"]
mod legacy_v2;

#[derive(Clone, Serialize)]
struct LegacyHistory {
    version: u32,
    item_count: usize,
    digest: String,
}

impl LegacyHistory {
    fn empty(version: u32) -> Result<Self> {
        ensure!(
            matches!(version, 1 | 2),
            "native_history_version_unsupported"
        );
        Ok(Self {
            version,
            item_count: 0,
            digest: format!(
                "{:x}",
                base16ct::HexDisplay(&Sha256::digest(format!(
                    "1flowbase.responses.history.v{version}"
                )))
            ),
        })
    }
}

/// Accumulates the old frozen proof and the current proof from the same ordered items.
/// State can branch by cloning accumulated source items. A persisted predecessor
/// can only seed both chains through a successful verification receipt.
#[derive(Clone)]
pub struct MigrationAccumulator {
    old: LegacyHistory,
    current: History,
}

impl MigrationAccumulator {
    pub fn new(legacy_version: u32) -> Result<Self> {
        Ok(Self {
            old: LegacyHistory::empty(legacy_version)?,
            current: History::empty(),
        })
    }

    /// Failure leaves both accumulators unchanged.
    pub fn append(&mut self, item: &Value) -> Result<()> {
        let old_item = match self.old.version {
            1 => legacy::normalize_item(item)?,
            2 => legacy_v2::normalize_item(item)?,
            _ => unreachable!("legacy version is checked by the constructor"),
        };
        let mut current = self.current.clone();
        current.append(item)?;
        let old_digest = append_digest(
            &self.old.digest,
            format!("1flowbase.responses.history.item.v{}\0", self.old.version).as_bytes(),
            &old_item,
        )?;
        let next_count = self
            .old
            .item_count
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("native_history_item_count_overflow"))?;
        self.old.digest = old_digest;
        self.old.item_count = next_count;
        self.current = current;
        Ok(())
    }

    /// Read-only progress, deliberately not a verification receipt.
    pub fn snapshot(&self) -> UnverifiedProgress {
        UnverifiedProgress {
            old: self.old.clone(),
            current: self.current.clone(),
        }
    }

    /// Check the full old format, count and digest; never relabel an unchecked proof.
    /// Unknown fields and malformed or unsupported versions are rejected.
    pub fn verify(&self, expected: &Value) -> Result<VerifiedReplacement> {
        ensure!(!expected.is_null(), "native_history_evidence_missing");
        let expected: History = serde_json::from_value(expected.clone())
            .map_err(|_| anyhow::anyhow!("native_history_evidence_invalid"))?;
        ensure!(
            expected.digest.len() == 64 && expected.digest.bytes().all(|c| c.is_ascii_hexdigit()),
            "native_history_evidence_invalid"
        );
        ensure!(
            matches!(expected.version, 1 | 2),
            "native_history_version_unsupported"
        );
        ensure!(
            expected.version == self.old.version,
            "native_history_version_mismatch"
        );
        ensure!(
            expected.item_count == self.old.item_count,
            "native_history_item_count_mismatch"
        );
        ensure!(
            expected.digest == self.old.digest,
            "native_history_mismatch"
        );
        Ok(VerifiedReplacement {
            old: self.old.clone(),
            current: self.current.clone(),
        })
    }
}

/// Unverified progress hashes. These values cannot seed a trusted predecessor.
#[derive(Clone, Serialize)]
pub struct UnverifiedProgress {
    old: LegacyHistory,
    current: History,
}

impl UnverifiedProgress {
    pub fn old(&self) -> Value {
        serde_json::json!(self.old)
    }

    pub fn current(&self) -> Value {
        serde_json::json!(self.current)
    }
}

/// A verified old/current replacement pair. No public constructor or Deserialize:
/// callers can serialize it for review, but only `verify` establishes trust.
#[derive(Serialize)]
pub struct VerifiedReplacement {
    old: LegacyHistory,
    current: History,
}

impl VerifiedReplacement {
    pub fn old(&self) -> Value {
        serde_json::json!(self.old)
    }

    pub fn current(&self) -> Value {
        serde_json::json!(self.current)
    }

    /// Continue both chains from this verified predecessor, preserving item counts.
    pub fn continue_history(&self) -> MigrationAccumulator {
        MigrationAccumulator {
            old: self.old.clone(),
            current: self.current.clone(),
        }
    }
}

#[cfg(test)]
#[path = "migration/_tests/mod.rs"]
mod tests;

#[path = "migration/source_graph.rs"]
mod source_graph;
pub use source_graph::{
    HistorySourceGraph, ScopedHistoryTarget, SourceCoverage, SourceGraphResolution, SourceLocator,
    SourceNamespace, SourcedReplacement, UnresolvedSource,
};

/// Reuse the runtime checkpoint materializer. The returned pool retains the
/// recovery marker. Preserve all user values and strip/replace only that marker
/// when producing a replacement snapshot; never replace the pool with the marker.
pub fn materialize_checkpoint_content(
    lineage: &[crate::ports::RuntimeContextContentVersion],
) -> Result<serde_json::Map<String, Value>> {
    crate::orchestration_runtime::materialize_responses_history_checkpoint(lineage)
}

#[path = "migration/planner.rs"]
mod planner;
pub use planner::{
    apply_verified_flow, build_plan, collect_proofs, inspect_flow, preview_verified_flow,
    public_rounds, FlowInspection,
};
