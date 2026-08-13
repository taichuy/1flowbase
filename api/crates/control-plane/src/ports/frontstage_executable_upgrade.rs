use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstageExecutableCompilerFailure {
    pub error_code: String,
}

#[async_trait]
pub trait FrontstageExecutableUpgradeCompiler: Send + Sync {
    async fn compile_frontstage_executable(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        source: &domain::LegacyFrontstageExecutableSnapshotRow,
    ) -> Result<domain::CompiledFrontstageExecutable, FrontstageExecutableCompilerFailure>;
}

#[async_trait]
pub trait FrontstageExecutableUpgradeRepository: Send + Sync {
    async fn begin_frontstage_executable_upgrade(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
    ) -> anyhow::Result<domain::FrontstageExecutableUpgradeStart>;

    async fn capture_frontstage_executable_upgrade_snapshot(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        run_id: uuid::Uuid,
    ) -> anyhow::Result<domain::LegacyFrontstageExecutableSnapshot>;

    async fn commit_frontstage_executable_upgrade(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        snapshot: &domain::LegacyFrontstageExecutableSnapshot,
        compiled: &[domain::CompiledFrontstageExecutable],
    ) -> anyhow::Result<()>;

    async fn record_frontstage_executable_upgrade_failure(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
        failure: &domain::FrontstageExecutableUpgradeFailure,
    ) -> anyhow::Result<()>;

    async fn require_frontstage_executable_cutover(
        &self,
        target: &domain::FrontstageExecutableUpgradeTarget,
    ) -> anyhow::Result<()>;
}
