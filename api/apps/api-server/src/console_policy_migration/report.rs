use std::{fs, path::PathBuf};

use anyhow::Result;
use control_plane::{
    ports::{RoleConsolePolicyMigrationCutoverMarker, RoleConsolePolicyMigrationCutoverState},
    role::console_policy_migration::ConsolePolicyMigrationLegacyGrantMapping,
};
use serde::Serialize;
use serde_json::Value;

use super::{
    crosswalk::{CompiledCoreConsolePolicyMigration, LIVE_CORE_MIGRATION_SOURCE_CONTRACT},
    ConsolePolicyMigrationOperationDisposition,
};

#[derive(Debug, Clone, Serialize)]
pub struct ConsolePolicyMigrationUnknownGrant {
    pub role_id: String,
    pub workspace_id: String,
    pub role_code: String,
    pub grant: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsolePolicyMigrationCutoverEvidence {
    pub marker: String,
    pub run_id: Option<String>,
    pub catalog_fingerprint: Option<String>,
    pub mapping_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConsolePolicyMigrationEvidenceReport {
    pub schema_version: &'static str,
    pub command: String,
    pub run_id: String,
    pub source_contract: &'static str,
    pub catalog_fingerprint: String,
    pub mapping_fingerprint: String,
    pub operation_dispositions: Vec<ConsolePolicyMigrationOperationDisposition>,
    pub legacy_mappings: Vec<ConsolePolicyMigrationLegacyGrantMapping>,
    pub role_projections: Vec<Value>,
    pub actor_multi_role_operation_matrix: Vec<Value>,
    pub unknown_grants: Vec<ConsolePolicyMigrationUnknownGrant>,
    pub authorization_deltas: Vec<Value>,
    pub validation_errors: Vec<String>,
    pub cutover_before: Option<ConsolePolicyMigrationCutoverEvidence>,
    pub cutover_after: Option<ConsolePolicyMigrationCutoverEvidence>,
    pub runtime_cutover: &'static str,
}

impl ConsolePolicyMigrationEvidenceReport {
    pub fn for_compiled(
        command: impl Into<String>,
        run_id: impl Into<String>,
        migration: &CompiledCoreConsolePolicyMigration,
    ) -> Self {
        Self {
            schema_version: "1flowbase.console-policy-migration-evidence/v1",
            command: command.into(),
            run_id: run_id.into(),
            source_contract: LIVE_CORE_MIGRATION_SOURCE_CONTRACT,
            catalog_fingerprint: migration.plan().catalog_fingerprint().to_string(),
            mapping_fingerprint: migration.plan().mapping_fingerprint().to_string(),
            operation_dispositions: migration.dispositions().to_vec(),
            legacy_mappings: migration.legacy_mappings().to_vec(),
            role_projections: Vec::new(),
            actor_multi_role_operation_matrix: Vec::new(),
            unknown_grants: Vec::new(),
            authorization_deltas: Vec::new(),
            validation_errors: Vec::new(),
            cutover_before: None,
            cutover_after: None,
            runtime_cutover: "The API runtime consumes the finalized cutover marker; this CLI owns rehearsal, apply, finalize, and rollback.",
        }
    }

    pub fn markdown(&self) -> String {
        let mut markdown = format!(
            "# Console policy migration evidence\n\n- Command: `{}`\n- Run: `{}`\n- Catalog fingerprint: `{}`\n- Mapping fingerprint: `{}`\n- Operation dispositions: {}\n- Role projections: {}\n- Actor operation/row matrices: {}\n- Unknown grants: {}\n- Authorization deltas: {}\n\n{}\n",
            self.command,
            self.run_id,
            self.catalog_fingerprint,
            self.mapping_fingerprint,
            self.operation_dispositions.len(),
            self.role_projections.len(),
            self.actor_multi_role_operation_matrix.len(),
            self.unknown_grants.len(),
            self.authorization_deltas.len(),
            self.runtime_cutover,
        );
        markdown.push_str("\n## Cutover fence\n\n");
        append_cutover(&mut markdown, "Before", self.cutover_before.as_ref());
        append_cutover(&mut markdown, "After", self.cutover_after.as_ref());
        markdown.push_str("\n## Operation dispositions\n\n");
        markdown.push_str("| Operation | Group | Authorization | Disposition |\n");
        markdown.push_str("| --- | --- | --- | --- |\n");
        for operation in &self.operation_dispositions {
            let disposition = match &operation.disposition {
                super::ConsolePolicyMigrationOperationDispositionKind::Operations { .. } => {
                    "operations"
                }
                super::ConsolePolicyMigrationOperationDispositionKind::NoProjection { .. } => {
                    "no_projection"
                }
                super::ConsolePolicyMigrationOperationDispositionKind::DefaultDisabledNewOperation {
                    ..
                } => "default_disabled_new_operation",
            };
            markdown.push_str(&format!(
                "| `{}` | `{}:{}` | `{}` | `{}` |\n",
                operation.operation_id,
                operation.policy_group_kind,
                operation.policy_group_id,
                operation.authorization,
                disposition,
            ));
        }
        append_json_section(&mut markdown, "Role projections", &self.role_projections);
        append_json_section(
            &mut markdown,
            "Actor multi-role operation/row matrix",
            &self.actor_multi_role_operation_matrix,
        );
        if !self.unknown_grants.is_empty() {
            markdown.push_str("\n## Unknown legacy grants\n\n");
            for unknown in &self.unknown_grants {
                markdown.push_str(&format!(
                    "- `{}` / `{}`: `{}`\n",
                    unknown.workspace_id, unknown.role_code, unknown.grant
                ));
            }
        }
        append_json_section(
            &mut markdown,
            "Authorization deltas",
            &self.authorization_deltas,
        );
        if !self.validation_errors.is_empty() {
            markdown.push_str("\n## Validation errors\n\n");
            for error in &self.validation_errors {
                markdown.push_str("- ");
                markdown.push_str(error);
                markdown.push('\n');
            }
        }
        markdown
    }
}

fn append_cutover(
    markdown: &mut String,
    phase: &str,
    cutover: Option<&ConsolePolicyMigrationCutoverEvidence>,
) {
    match cutover {
        Some(cutover) => markdown.push_str(&format!(
            "- {phase}: `{}` (run `{}`)\n",
            cutover.marker,
            cutover.run_id.as_deref().unwrap_or("none"),
        )),
        None => markdown.push_str(&format!("- {phase}: not queried\n")),
    }
}

fn append_json_section(markdown: &mut String, heading: &str, values: &[Value]) {
    markdown.push_str(&format!("\n## {heading}\n\n```json\n"));
    let value = serde_json::to_string_pretty(values)
        .expect("migration report values must be JSON serializable");
    markdown.push_str(&value);
    markdown.push_str("\n```\n");
}

impl From<RoleConsolePolicyMigrationCutoverState> for ConsolePolicyMigrationCutoverEvidence {
    fn from(state: RoleConsolePolicyMigrationCutoverState) -> Self {
        Self {
            marker: match state.marker {
                RoleConsolePolicyMigrationCutoverMarker::Legacy => "legacy",
                RoleConsolePolicyMigrationCutoverMarker::Fenced => "fenced",
                RoleConsolePolicyMigrationCutoverMarker::ConsolePolicy => "console_policy",
            }
            .to_string(),
            run_id: state.run_id.map(|id| id.to_string()),
            catalog_fingerprint: state.catalog_fingerprint,
            mapping_fingerprint: state.mapping_fingerprint,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConsolePolicyMigrationEvidencePaths {
    pub json: PathBuf,
    pub markdown: PathBuf,
}

pub fn write_evidence_report(
    report: &ConsolePolicyMigrationEvidenceReport,
) -> Result<ConsolePolicyMigrationEvidencePaths> {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("tmp/test-governance");
    fs::create_dir_all(&directory)?;
    let stem = format!(
        "console-policy-migration-{}-{}",
        report.command, report.run_id
    );
    let json = directory.join(format!("{stem}.json"));
    let markdown = directory.join(format!("{stem}.md"));
    fs::write(&json, serde_json::to_string_pretty(report)?)?;
    fs::write(&markdown, report.markdown())?;
    Ok(ConsolePolicyMigrationEvidencePaths { json, markdown })
}
