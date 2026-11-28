use anyhow::{Context, Result};
use async_trait::async_trait;
use control_plane_contracts::ports::{
    AppendPrivacyMaskingLogInput, ListPrivacyMaskingLogsQuery, PrivacyMaskingLogsQueryPage,
    PrivacyMaskingRepository, PrivacyMaskingRuntimePolicyRecord, PrivacyRunPayloadMask,
    UpsertPrivacyMaskingRuntimePolicyInput,
};
use extension_contracts::{
    PrivacyMaskingLogEntryDto, PrivacyMaskingLogMatchDto, PrivacyMaskingSubjectKind,
    PrivacyMaskingWorkspacePolicyDto, PRIVACY_POLICY_SYNC_PERMISSION,
};
use sqlx::{Postgres, QueryBuilder, Row, Transaction};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use crate::PgControlPlaneStore;

pub(crate) async fn delete_policies_for_installations_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    installation_ids: &[Uuid],
) -> Result<u64> {
    if installation_ids.is_empty() {
        return Ok(0);
    }
    let result =
        sqlx::query("DELETE FROM privacy_masking_runtime_policies WHERE installation_id = ANY($1)")
            .bind(installation_ids)
            .execute(&mut **transaction)
            .await?;
    Ok(result.rows_affected())
}

#[async_trait]
impl PrivacyMaskingRepository for PgControlPlaneStore {
    async fn upsert_privacy_masking_policy(
        &self,
        input: &UpsertPrivacyMaskingRuntimePolicyInput,
    ) -> Result<PrivacyMaskingRuntimePolicyRecord> {
        let policy_json = serde_json::to_value(&input.policy)
            .context("failed to serialize privacy masking policy")?;
        let row = sqlx::query(
            r#"
            INSERT INTO privacy_masking_runtime_policies (
                installation_id,
                workspace_id,
                policy_version,
                policy_json,
                updated_by,
                created_at,
                updated_at
            ) VALUES ($1, $2, 1, $3, $4, NOW(), NOW())
            ON CONFLICT (installation_id, workspace_id) DO UPDATE
            SET
                policy_version = privacy_masking_runtime_policies.policy_version + 1,
                policy_json = EXCLUDED.policy_json,
                updated_by = EXCLUDED.updated_by,
                updated_at = NOW()
            RETURNING installation_id, workspace_id, policy_version, policy_json, updated_by, updated_at
            "#,
        )
        .bind(input.installation_id)
        .bind(input.workspace_id)
        .bind(policy_json)
        .bind(input.updated_by)
        .fetch_one(self.pool())
        .await?;

        self.observe_privacy_workspace_invalidation(input.workspace_id);
        map_policy_row(row)
    }

    async fn list_active_privacy_masking_policies(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PrivacyMaskingRuntimePolicyRecord>> {
        let rows = sqlx::query(
            r#"
            SELECT
                p.installation_id,
                p.workspace_id,
                p.policy_version,
                p.policy_json,
                p.updated_by,
                p.updated_at
            FROM privacy_masking_runtime_policies p
            INNER JOIN extension_installations ei
                ON ei.id = p.installation_id
            WHERE p.workspace_id = $1
              AND ei.desired_state = 'active_requested'
              AND EXISTS (
                  SELECT 1
                  FROM plugin_contribution_authorizations authz
                  WHERE authz.installation_id = p.installation_id
                    AND authz.permission = $2
                    AND authz.status = 'active'
              )
            ORDER BY p.updated_at ASC, p.installation_id ASC
            "#,
        )
        .bind(workspace_id)
        .bind(PRIVACY_POLICY_SYNC_PERMISSION)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_policy_row).collect()
    }

    async fn delete_privacy_masking_policies_for_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM privacy_masking_runtime_policies WHERE installation_id = $1")
                .bind(installation_id)
                .execute(self.pool())
                .await?;
        self.observe_privacy_installation_invalidation(installation_id);
        Ok(result.rows_affected())
    }

    async fn mask_privacy_run_inputs(
        &self,
        workspace_id: Uuid,
        flow_run_id: Uuid,
        mask: &PrivacyRunPayloadMask<'_>,
    ) -> Result<()> {
        self.rewrite_privacy_run_payloads(workspace_id, flow_run_id, mask)
            .await
    }

    async fn append_privacy_masking_log(&self, input: &AppendPrivacyMaskingLogInput) -> Result<()> {
        let matched_rules_json = serde_json::to_value(&input.matched_rules)
            .context("failed to serialize privacy masking log matches")?;
        sqlx::query(
            r#"
            INSERT INTO privacy_masking_logs (
                id,
                workspace_id,
                installation_id,
                flow_run_id,
                application_id,
                provider_instance_id,
                subject_kind,
                subject_id,
                subject_name,
                model_id,
                protocol,
                restore_enabled,
                hit_count,
                restored_count,
                matched_rules_json,
                created_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16
            )
            "#,
        )
        .bind(input.id)
        .bind(input.workspace_id)
        .bind(input.installation_id)
        .bind(input.flow_run_id)
        .bind(input.application_id)
        .bind(input.provider_instance_id)
        .bind(input.subject_kind.as_str())
        .bind(&input.subject_id)
        .bind(&input.subject_name)
        .bind(&input.model_id)
        .bind(&input.protocol)
        .bind(input.restore_enabled)
        .bind(i32::try_from(input.hit_count).unwrap_or(i32::MAX))
        .bind(i32::try_from(input.restored_count).unwrap_or(i32::MAX))
        .bind(matched_rules_json)
        .bind(input.created_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    async fn list_privacy_masking_logs(
        &self,
        query: &ListPrivacyMaskingLogsQuery,
    ) -> Result<PrivacyMaskingLogsQueryPage> {
        let limit = query.limit.clamp(1, 200);
        let fetch_limit = i64::from(limit) + 1;

        let mut builder: QueryBuilder<Postgres> = QueryBuilder::new(
            r#"
            SELECT
                id,
                workspace_id,
                flow_run_id,
                application_id,
                provider_instance_id,
                subject_kind,
                subject_id,
                subject_name,
                model_id,
                protocol,
                restore_enabled,
                hit_count,
                restored_count,
                matched_rules_json,
                created_at
            FROM privacy_masking_logs
            WHERE workspace_id =
            "#,
        );
        builder.push_bind(query.workspace_id);

        if let Some(subject_kind) = query.subject_kind {
            builder.push(" AND subject_kind = ");
            builder.push_bind(subject_kind.as_str());
        }
        if let Some(subject_id) = query
            .subject_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            builder.push(" AND subject_id = ");
            builder.push_bind(subject_id);
        }
        if let Some(flow_run_id) = query.flow_run_id {
            builder.push(" AND flow_run_id = ");
            builder.push_bind(flow_run_id);
        }
        if let Some(placeholder_kind) = query
            .placeholder_kind
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let filter_json = serde_json::json!([{ "placeholder_kind": placeholder_kind }]);
            builder.push(" AND matched_rules_json @> ");
            builder.push_bind(filter_json);
        }
        if let Some(rule_id) = query
            .rule_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            let filter_json = serde_json::json!([{ "rule_id": rule_id }]);
            builder.push(" AND matched_rules_json @> ");
            builder.push_bind(filter_json);
        }
        if let Some((cursor_ts, cursor_id)) = query.cursor {
            builder.push(" AND (created_at, id) < (");
            builder.push_bind(cursor_ts);
            builder.push(", ");
            builder.push_bind(cursor_id);
            builder.push(")");
        }

        builder.push(" ORDER BY created_at DESC, id DESC LIMIT ");
        builder.push_bind(fetch_limit);

        let rows = builder.build().fetch_all(self.pool()).await?;
        let has_more = rows.len() > limit as usize;
        let mut items = Vec::with_capacity(rows.len().min(limit as usize));
        let mut last_key = None;

        for row in rows.into_iter().take(limit as usize) {
            let id: Uuid = row.try_get("id")?;
            let created_at: OffsetDateTime = row.try_get("created_at")?;
            last_key = Some((created_at, id));
            items.push(map_log_row(row, id, created_at)?);
        }

        Ok(PrivacyMaskingLogsQueryPage {
            items,
            next_cursor: if has_more { last_key } else { None },
        })
    }
}

fn map_policy_row(row: sqlx::postgres::PgRow) -> Result<PrivacyMaskingRuntimePolicyRecord> {
    let policy_json: serde_json::Value = row.try_get("policy_json")?;
    let policy: PrivacyMaskingWorkspacePolicyDto = serde_json::from_value(policy_json)
        .context("failed to deserialize privacy masking policy")?;
    Ok(PrivacyMaskingRuntimePolicyRecord {
        installation_id: row.try_get("installation_id")?,
        workspace_id: row.try_get("workspace_id")?,
        policy_version: row.try_get("policy_version")?,
        policy,
        updated_by: row.try_get("updated_by")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn map_log_row(
    row: sqlx::postgres::PgRow,
    id: Uuid,
    created_at: OffsetDateTime,
) -> Result<PrivacyMaskingLogEntryDto> {
    let workspace_id: Uuid = row.try_get("workspace_id")?;
    let flow_run_id: Option<Uuid> = row.try_get("flow_run_id")?;
    let application_id: Option<Uuid> = row.try_get("application_id")?;
    let provider_instance_id: Option<Uuid> = row.try_get("provider_instance_id")?;
    let subject_kind_raw: String = row.try_get("subject_kind")?;
    let subject_kind = PrivacyMaskingSubjectKind::parse(&subject_kind_raw)
        .ok_or_else(|| anyhow::anyhow!("unknown privacy subject_kind: {subject_kind_raw}"))?;
    let matched_rules_json: serde_json::Value = row.try_get("matched_rules_json")?;
    let matched_rules: Vec<PrivacyMaskingLogMatchDto> =
        serde_json::from_value(matched_rules_json).unwrap_or_default();
    let hit_count: i32 = row.try_get("hit_count")?;
    let restored_count: i32 = row.try_get("restored_count")?;

    Ok(PrivacyMaskingLogEntryDto {
        log_id: id.to_string(),
        workspace_id: workspace_id.to_string(),
        flow_run_id: flow_run_id.map(|v| v.to_string()),
        application_id: application_id.map(|v| v.to_string()),
        provider_instance_id: provider_instance_id.map(|v| v.to_string()),
        subject_kind,
        subject_id: row.try_get("subject_id")?,
        subject_name: row.try_get("subject_name")?,
        model_id: row.try_get("model_id")?,
        protocol: row.try_get("protocol")?,
        restore_enabled: row.try_get("restore_enabled")?,
        hit_count: u32::try_from(hit_count.max(0)).unwrap_or(0),
        restored_count: u32::try_from(restored_count.max(0)).unwrap_or(0),
        matched_rules,
        created_at: created_at
            .format(&Rfc3339)
            .unwrap_or_else(|_| created_at.to_string()),
    })
}
