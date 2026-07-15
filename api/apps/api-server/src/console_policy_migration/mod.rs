mod cli;
mod crosswalk;
mod live;
mod report;

use anyhow::{Result, bail};
use control_plane::ports::{
    RoleConsolePolicyMigrationCutoverMarker, RoleConsolePolicyMigrationRepository,
};

pub use cli::{ConsolePolicyMigrationCommand, parse_command, run_from_env};
pub use crosswalk::{
    CompiledCoreConsolePolicyMigration, ConsolePolicyMigrationOperationDisposition,
    ConsolePolicyMigrationOperationDispositionKind, compile_core_console_policy_migration_plan,
    live_legacy_migration_source,
};
#[cfg(test)]
pub(crate) use live::preview_live_migration;
pub use report::{
    ConsolePolicyMigrationEvidencePaths, ConsolePolicyMigrationEvidenceReport,
    ConsolePolicyMigrationUnknownGrant, write_evidence_report,
};

pub async fn require_runtime_console_policy_cutover(
    store: &storage_durable::MainDurableStore,
) -> Result<()> {
    let state = store.role_console_policy_migration_cutover_state().await?;
    match state.marker {
        RoleConsolePolicyMigrationCutoverMarker::ConsolePolicy => Ok(()),
        RoleConsolePolicyMigrationCutoverMarker::Legacy => bail!(
            "console policy migration is still legacy; run rehearsal, apply, smoke, and finalize before starting this runtime"
        ),
        RoleConsolePolicyMigrationCutoverMarker::Fenced => bail!(
            "console policy migration is fenced for run {}; finish smoke/finalize or rollback before starting this runtime",
            state
                .run_id
                .map(|run_id| run_id.to_string())
                .unwrap_or_else(|| "<missing>".to_string())
        ),
    }
}
