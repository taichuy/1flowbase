use super::*;

#[async_trait]
impl PortableTemplateBaselineRepository for PgControlPlaneStore {
    async fn load_template_baselines(
        &self,
        scope: &TemplateBaselineScope,
    ) -> Result<Vec<TemplateResourceBaseline>> {
        let rows = sqlx::query("SELECT kind,source_id,target_id,generation,applied_fingerprint,pending_operation_id,pending_expected_fingerprint,pending_desired_fingerprint,committed_operation_id,committed_fingerprint FROM application_template_resource_baselines WHERE workspace_id=$1 AND template_id=$2")
            .bind(scope.workspace_id).bind(&scope.template_id).fetch_all(self.pool()).await?;
        rows.into_iter()
            .map(|row| {
                let target_id: String = row.try_get("target_id")?;
                let pending_id: Option<Uuid> = row.try_get("pending_operation_id")?;
                Ok(TemplateResourceBaseline {
                    key: TemplateResourceKey {
                        kind: row.try_get("kind")?,
                        source_id: row.try_get("source_id")?,
                    },
                    target_id: target_id.clone(),
                    generation: row.try_get("generation")?,
                    applied_fingerprint: row.try_get("applied_fingerprint")?,
                    pending: pending_id
                        .map(|operation_id| -> Result<_> {
                            Ok(TemplateWriteIntent {
                                operation_id,
                                target_id,
                                expected_fingerprint: row
                                    .try_get("pending_expected_fingerprint")?,
                                desired_fingerprint: row.try_get("pending_desired_fingerprint")?,
                            })
                        })
                        .transpose()?,
                    committed_operation_id: row.try_get("committed_operation_id")?,
                    committed_fingerprint: row.try_get("committed_fingerprint")?,
                })
            })
            .collect()
    }

    async fn prepare_template_write(
        &self,
        scope: &TemplateBaselineScope,
        key: &TemplateResourceKey,
        expected_generation: Option<i64>,
        intent: &TemplateWriteIntent,
    ) -> Result<bool> {
        let result = if let Some(generation) = expected_generation {
            sqlx::query("UPDATE application_template_resource_baselines SET pending_operation_id=$5,pending_expected_fingerprint=$6,pending_desired_fingerprint=$7,generation=generation+1,updated_at=now() WHERE workspace_id=$1 AND template_id=$2 AND kind=$3 AND source_id=$4 AND generation=$8 AND target_id=$9 AND applied_fingerprint IS NOT DISTINCT FROM $6 AND pending_operation_id IS NULL")
                .bind(scope.workspace_id).bind(&scope.template_id).bind(&key.kind).bind(&key.source_id)
                .bind(intent.operation_id).bind(&intent.expected_fingerprint).bind(&intent.desired_fingerprint)
                .bind(generation).bind(&intent.target_id).execute(self.pool()).await?
        } else {
            anyhow::ensure!(
                intent.expected_fingerprint.is_none(),
                "portable_template_cannot_adopt_untracked_target"
            );
            sqlx::query("INSERT INTO application_template_resource_baselines (id,workspace_id,scope_id,template_id,kind,source_id,target_id,pending_operation_id,pending_expected_fingerprint,pending_desired_fingerprint) VALUES ($1,$2,$2,$3,$4,$5,$6,$7,$8,$9) ON CONFLICT (workspace_id,template_id,kind,source_id) DO NOTHING")
                .bind(Uuid::now_v7()).bind(scope.workspace_id).bind(&scope.template_id).bind(&key.kind).bind(&key.source_id)
                .bind(&intent.target_id).bind(intent.operation_id).bind(&intent.expected_fingerprint).bind(&intent.desired_fingerprint).execute(self.pool()).await?
        };
        Ok(result.rows_affected() == 1)
    }

    async fn finalize_template_write(
        &self,
        scope: &TemplateBaselineScope,
        key: &TemplateResourceKey,
        operation_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query("UPDATE application_template_resource_baselines SET applied_fingerprint=committed_fingerprint,pending_operation_id=NULL,pending_expected_fingerprint=NULL,pending_desired_fingerprint=NULL,committed_operation_id=NULL,committed_fingerprint=NULL,generation=generation+1,updated_at=now() WHERE workspace_id=$1 AND template_id=$2 AND kind=$3 AND source_id=$4 AND pending_operation_id=$5 AND committed_operation_id=$5")
            .bind(scope.workspace_id).bind(&scope.template_id).bind(&key.kind).bind(&key.source_id).bind(operation_id).execute(self.pool()).await?;
        Ok(result.rows_affected() == 1)
    }

    async fn abandon_template_write(
        &self,
        scope: &TemplateBaselineScope,
        key: &TemplateResourceKey,
        operation_id: Uuid,
    ) -> Result<bool> {
        let result = sqlx::query("UPDATE application_template_resource_baselines SET pending_operation_id=NULL,pending_expected_fingerprint=NULL,pending_desired_fingerprint=NULL,generation=generation+1,updated_at=now() WHERE workspace_id=$1 AND template_id=$2 AND kind=$3 AND source_id=$4 AND pending_operation_id=$5 AND committed_operation_id IS NULL")
            .bind(scope.workspace_id).bind(&scope.template_id).bind(&key.kind).bind(&key.source_id).bind(operation_id).execute(self.pool()).await?;
        Ok(result.rows_affected() == 1)
    }
}

/// Call on the resource owner's active transaction connection AFTER a successful
/// compare-and-write, BEFORE commit. The caller must rollback when this returns false.
/// Do not call on a fresh connection or after an independently committed owner write.
pub(crate) async fn acknowledge_template_write(
    connection: &mut sqlx::PgConnection,
    scope: &TemplateBaselineScope,
    key: &TemplateResourceKey,
    intent: &TemplateWriteIntent,
    actual_fingerprint: &str,
) -> Result<bool> {
    let result = sqlx::query("UPDATE application_template_resource_baselines SET committed_operation_id=$5,committed_fingerprint=$9,updated_at=now() WHERE workspace_id=$1 AND template_id=$2 AND kind=$3 AND source_id=$4 AND pending_operation_id=$5 AND target_id=$6 AND pending_expected_fingerprint IS NOT DISTINCT FROM $7 AND pending_desired_fingerprint=$8 AND committed_operation_id IS NULL")
        .bind(scope.workspace_id).bind(&scope.template_id).bind(&key.kind).bind(&key.source_id)
        .bind(intent.operation_id).bind(&intent.target_id).bind(&intent.expected_fingerprint).bind(&intent.desired_fingerprint)
        .bind(actual_fingerprint).execute(connection).await?;
    Ok(result.rows_affected() == 1)
}
