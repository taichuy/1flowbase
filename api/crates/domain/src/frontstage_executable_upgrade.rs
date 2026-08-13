use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontstageExecutableUpgradeTarget {
    pub marker: String,
    pub contract_identity: Value,
    pub compiler_identity: Value,
    pub toolchain_lock: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrontstageExecutableCatalogLocator {
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub contribution_code: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyFrontstageExecutableSnapshotRow {
    pub row_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
    pub source_code: String,
    pub source_sha256: String,
    pub catalog_locator: FrontstageExecutableCatalogLocator,
    pub runtime_descriptor: Value,
    pub dependency_lock: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegacyFrontstageExecutableSnapshot {
    pub run_id: Uuid,
    pub rows: Vec<LegacyFrontstageExecutableSnapshotRow>,
    pub snapshot_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompiledFrontstageExecutable {
    pub row_id: Uuid,
    pub source_sha256: String,
    pub dependency_lock: Value,
    pub generated_css: String,
    pub generated_css_sha256: String,
    pub compiler_identity: Value,
    pub toolchain_lock: Value,
    pub contract_identity: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontstageExecutableUpgradeFailure {
    pub run_id: Uuid,
    pub marker: String,
    pub error_code: String,
    pub target_identity: Value,
    pub compiler_identity: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontstageExecutableUpgradeStart {
    Completed,
    Run { run_id: Uuid, attempt: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrontstageExecutableUpgradeOutcome {
    Completed {
        run_id: Option<Uuid>,
        upgraded: usize,
    },
}
