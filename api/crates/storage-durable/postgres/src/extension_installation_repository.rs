use anyhow::Result;
use async_trait::async_trait;
use control_plane::ports::{ExtensionInstallationRepository, UpsertExtensionInstallationInput};
use sqlx::{postgres::PgRow, Row};
use uuid::Uuid;

use crate::{
    mappers::extension_installation::{
        to_extension_installation_record, StoredExtensionInstallationRow,
    },
    repositories::PgControlPlaneStore,
};

const RETURNING_COLUMNS: &str = r#"
    id, category, organization, artifact_id, artifact_version, node_id,
    source, trust, local_path, checksum, signature_status, signature_algorithm,
    signing_key_id, warnings, receipt, application_action, status, installed_by, created_at, updated_at
"#;

#[async_trait]
impl ExtensionInstallationRepository for PgControlPlaneStore {
    async fn upsert_extension_installation(
        &self,
        input: &UpsertExtensionInstallationInput,
    ) -> Result<domain::ExtensionInstallationRecord> {
        let query = format!(
            r#"
            insert into extension_installations (
                id, category, organization, artifact_id, artifact_version, node_id,
                source, trust, local_path, checksum, signature_status, signature_algorithm,
                signing_key_id, warnings, receipt, application_action, status, installed_by
            ) values (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18
            )
            on conflict (category, organization, artifact_id, artifact_version, node_id)
            do update set
                source = excluded.source,
                trust = excluded.trust,
                local_path = excluded.local_path,
                checksum = excluded.checksum,
                signature_status = excluded.signature_status,
                signature_algorithm = excluded.signature_algorithm,
                signing_key_id = excluded.signing_key_id,
                warnings = excluded.warnings,
                receipt = excluded.receipt,
                application_action = excluded.application_action,
                status = excluded.status,
                installed_by = excluded.installed_by,
                updated_at = now()
            returning {RETURNING_COLUMNS}
            "#
        );
        let row = sqlx::query(&query)
            .bind(input.installation_id)
            .bind(input.identity.category.as_str())
            .bind(&input.identity.organization)
            .bind(&input.identity.artifact_id)
            .bind(&input.identity.version)
            .bind(&input.identity.node_id)
            .bind(&input.source)
            .bind(&input.trust)
            .bind(&input.local_path)
            .bind(&input.checksum)
            .bind(input.signature_status.as_str())
            .bind(&input.signature_algorithm)
            .bind(&input.signing_key_id)
            .bind(serde_json::to_value(&input.warnings)?)
            .bind(&input.receipt)
            .bind(input.application_action.as_str())
            .bind(input.status.as_str())
            .bind(input.installed_by)
            .fetch_one(self.pool())
            .await?;
        map_row(row)
    }

    async fn find_extension_installation(
        &self,
        identity: &domain::ExtensionInstallationIdentity,
    ) -> Result<Option<domain::ExtensionInstallationRecord>> {
        let query = format!(
            r#"
            select {RETURNING_COLUMNS}
            from extension_installations
            where category = $1 and organization = $2 and artifact_id = $3
              and artifact_version = $4 and node_id = $5
            "#
        );
        sqlx::query(&query)
            .bind(identity.category.as_str())
            .bind(&identity.organization)
            .bind(&identity.artifact_id)
            .bind(&identity.version)
            .bind(&identity.node_id)
            .fetch_optional(self.pool())
            .await?
            .map(map_row)
            .transpose()
    }

    async fn list_extension_installations_for_node(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::ExtensionInstallationRecord>> {
        let query = format!(
            r#"
            select {RETURNING_COLUMNS}
            from extension_installations
            where node_id = $1
            order by updated_at desc, id desc
            "#
        );
        sqlx::query(&query)
            .bind(node_id)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(map_row)
            .collect()
    }

    async fn set_extension_installation_status(
        &self,
        installation_id: Uuid,
        status: domain::ExtensionInstallationStatus,
    ) -> Result<()> {
        sqlx::query(
            r#"
            update extension_installations
            set status = $2, updated_at = now()
            where id = $1
            "#,
        )
        .bind(installation_id)
        .bind(status.as_str())
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

fn map_row(row: PgRow) -> Result<domain::ExtensionInstallationRecord> {
    to_extension_installation_record(StoredExtensionInstallationRow {
        id: row.try_get("id")?,
        category: row.try_get("category")?,
        organization: row.try_get("organization")?,
        artifact_id: row.try_get("artifact_id")?,
        artifact_version: row.try_get("artifact_version")?,
        node_id: row.try_get("node_id")?,
        source: row.try_get("source")?,
        trust: row.try_get("trust")?,
        local_path: row.try_get("local_path")?,
        checksum: row.try_get("checksum")?,
        signature_status: row.try_get("signature_status")?,
        signature_algorithm: row.try_get("signature_algorithm")?,
        signing_key_id: row.try_get("signing_key_id")?,
        warnings: row.try_get("warnings")?,
        receipt: row.try_get("receipt")?,
        application_action: row.try_get("application_action")?,
        status: row.try_get("status")?,
        installed_by: row.try_get("installed_by")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
