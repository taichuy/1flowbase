use access_control::{ConsoleAuthorization, ConsolePolicyGroup, ConsoleRouteBinding};
use serde::Serialize;
pub const COMPILED_CONSOLE_OPERATION_SNAPSHOT_SCHEMA_V1: &str =
    "1flowbase.compiled-console-operation-snapshot/v1";
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsoleBindingOwnerKind {
    Family,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConsoleMigrationDisposition {
    LegacyOperations { legacy_grants: Vec<String> },
    NoProjection { evidence: String },
    DefaultDisabled { evidence: String },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledConsoleBindingOwnership {
    pub contribution_id: String,
    pub owner_kind: ConsoleBindingOwnerKind,
    pub owner_id: String,
    pub interface_id: String,
    pub binding_id: String,
    pub protocol: String,
    pub route: ConsoleRouteBinding,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledConsoleOperationRecord {
    pub operation_id: String,
    pub authorization_profile_id: String,
    pub policy_group: ConsolePolicyGroup,
    pub authorization: ConsoleAuthorization,
    pub bindings: Vec<CompiledConsoleBindingOwnership>,
    pub migration: ConsoleMigrationDisposition,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompiledConsoleOperationSnapshot {
    pub schema_version: &'static str,
    pub operations: Vec<CompiledConsoleOperationRecord>,
}
impl CompiledConsoleOperationSnapshot {
    #[cfg(any(test, feature = "test-support"))]
    pub fn operation(&self, operation_id: &str) -> Option<&CompiledConsoleOperationRecord> {
        self.operations
            .binary_search_by(|operation| operation.operation_id.as_str().cmp(operation_id))
            .ok()
            .and_then(|index| self.operations.get(index))
    }
}
