use std::{path::PathBuf, sync::Arc};

use argon2::PasswordHash;
use async_trait::async_trait;
use control_plane::{
    file_management::RecoveryObjectStorageResolver,
    plugin_management::{
        ArtifactRecoveryCoordinate, ArtifactRecoveryResolver, ExtensionInstallationService,
    },
    ports::{
        AuthRepository, BackupRepository, BootstrapRepository, CacheStore, SessionStore, TaskQueue,
    },
    system_recovery::{
        PostRestoreDependencyError, PostRestoreHealthVerifier, PostRestoreReconcileTarget,
        PostRestoreRecoveryContext, RecoveryAuditProjection, RecoveryAuditProjector,
        RecoveryEphemeralState,
    },
};
use domain::{
    BackupComponent, BackupComponentDisposition, BackupComponentKind, BackupComponentRestoreTarget,
    ContentDigest, UserStatus,
};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPoolOptions;
use tokio::io::AsyncReadExt;

/// Resets the three excluded ephemeral domains through explicit host capabilities.
///
/// Providers which retain the default unsupported implementation fail closed; the recovery
/// service then rolls durable/object/artifact targets back instead of reopening with stale state.
pub struct ApiRecoveryEphemeralState {
    sessions: Arc<dyn SessionStore>,
    cache: Arc<dyn CacheStore>,
    queue: Arc<dyn TaskQueue>,
}

impl ApiRecoveryEphemeralState {
    pub fn new(
        sessions: Arc<dyn SessionStore>,
        cache: Arc<dyn CacheStore>,
        queue: Arc<dyn TaskQueue>,
    ) -> Self {
        Self {
            sessions,
            cache,
            queue,
        }
    }
}

#[async_trait]
impl RecoveryEphemeralState for ApiRecoveryEphemeralState {
    async fn invalidate_after_restore(
        &self,
        _context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        self.sessions
            .reset_for_system_recovery()
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        self.cache
            .reset_for_system_recovery()
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        self.queue
            .reset_for_system_recovery()
            .await
            .map_err(|_| PostRestoreDependencyError)
    }
}

/// Invalidates durable session generations while the API process is stopped.
///
/// Cache and queue entries are intentionally absent: the offline command never loads them from a
/// BackupSet and a restarted API process constructs fresh ephemeral providers.
pub struct StoppedServerRecoveryEphemeralState {
    database_url: String,
}

impl StoppedServerRecoveryEphemeralState {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }
}

#[async_trait]
impl RecoveryEphemeralState for StoppedServerRecoveryEphemeralState {
    async fn invalidate_after_restore(
        &self,
        _context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        let result = sqlx::query(
            r#"
            update users
               set session_version = session_version + 1,
                   updated_at = now()
            "#,
        )
        .execute(&pool)
        .await
        .map_err(|_| PostRestoreDependencyError);
        pool.close().await;
        match result {
            Ok(receipt) if receipt.rows_affected() > 0 => Ok(()),
            _ => Err(PostRestoreDependencyError),
        }
    }
}

/// Runs idempotent restored-database migrations and rebuilds host-owned registries without any
/// remote artifact repair.
pub struct PostgreSqlPostRestoreReconciler {
    database_url: String,
    node_id: String,
    install_root: PathBuf,
}

impl PostgreSqlPostRestoreReconciler {
    pub fn new(
        database_url: impl Into<String>,
        node_id: impl Into<String>,
        install_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            database_url: database_url.into(),
            node_id: node_id.into(),
            install_root: install_root.into(),
        }
    }
}

#[async_trait]
impl PostRestoreReconcileTarget for PostgreSqlPostRestoreReconciler {
    async fn reconcile(
        &self,
        _context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        let runtime = storage_durable_postgres::build_main_durable_postgres_with_max_connections(
            &self.database_url,
            1,
        )
        .await
        .map_err(|_| PostRestoreDependencyError)?;
        let store = runtime.store;
        let result = async {
            BootstrapRepository::upsert_permission_catalog(
                &store,
                &access_control::permission_catalog(),
            )
            .await
            .map_err(|_| PostRestoreDependencyError)?;
            ExtensionInstallationService::new(store.clone(), self.install_root.clone())
                .reconcile_node_inventory(&self.node_id)
                .await
                .map_err(|_| PostRestoreDependencyError)?;
            Ok(())
        }
        .await;
        store.pool().close().await;
        result
    }
}

/// Verifies the finite stopped-server health matrix against the promoted database and restored
/// object/artifact targets. Every dependency is reopened after PostgreSQL promotion.
pub struct PostgreSqlPostRestoreHealthVerifier {
    database_url: String,
    node_id: String,
    expected_migration_head: domain::MigrationHead,
    repository: Arc<dyn BackupRepository>,
    object_resolver: Arc<dyn RecoveryObjectStorageResolver>,
    object_registry: Arc<storage_object::FileStorageDriverRegistry>,
    artifact_resolver: Arc<dyn ArtifactRecoveryResolver>,
}

impl PostgreSqlPostRestoreHealthVerifier {
    pub fn new(
        database_url: impl Into<String>,
        node_id: impl Into<String>,
        expected_migration_head: domain::MigrationHead,
        repository: Arc<dyn BackupRepository>,
        object_resolver: Arc<dyn RecoveryObjectStorageResolver>,
        object_registry: Arc<storage_object::FileStorageDriverRegistry>,
        artifact_resolver: Arc<dyn ArtifactRecoveryResolver>,
    ) -> Self {
        Self {
            database_url: database_url.into(),
            node_id: node_id.into(),
            expected_migration_head,
            repository,
            object_resolver,
            object_registry,
            artifact_resolver,
        }
    }

    async fn verify_database(
        &self,
        context: &PostRestoreRecoveryContext,
        expected_migration_head: &domain::MigrationHead,
    ) -> Result<(), PostRestoreDependencyError> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        let store = storage_durable_postgres::MainDurableStore::new(pool.clone());
        let result = async {
            let actual = storage_durable_postgres::migration_head(&pool)
                .await
                .map_err(|_| PostRestoreDependencyError)?;
            if &actual != expected_migration_head {
                return Err(PostRestoreDependencyError);
            }

            let user = AuthRepository::find_user_by_id(&store, context.actor_user_id)
                .await
                .map_err(|_| PostRestoreDependencyError)?
                .ok_or(PostRestoreDependencyError)?;
            if user.status != UserStatus::Active
                || user.password_hash.is_empty()
                || PasswordHash::new(&user.password_hash).is_err()
            {
                return Err(PostRestoreDependencyError);
            }
            let authenticator =
                AuthRepository::find_authenticator(&store, domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
                    .await
                    .map_err(|_| PostRestoreDependencyError)?
                    .filter(|authenticator| authenticator.enabled)
                    .ok_or(PostRestoreDependencyError)?;
            if authenticator.auth_type != "password-local" {
                return Err(PostRestoreDependencyError);
            }
            let scope = AuthRepository::default_scope_for_user(&store, context.actor_user_id)
                .await
                .map_err(|_| PostRestoreDependencyError)?;
            let actor = AuthRepository::load_actor_context(
                &store,
                context.actor_user_id,
                scope.tenant_id,
                scope.workspace_id,
                None,
            )
            .await
            .map_err(|_| PostRestoreDependencyError)?;
            if !actor.is_root || actor.user_id != context.actor_user_id {
                return Err(PostRestoreDependencyError);
            }

            let permission_count =
                sqlx::query_scalar::<_, i64>("select count(*) from permission_definitions")
                    .fetch_one(&pool)
                    .await
                    .map_err(|_| PostRestoreDependencyError)?;
            if permission_count < access_control::permission_catalog().len() as i64 {
                return Err(PostRestoreDependencyError);
            }
            let unhealthy_node_artifacts = sqlx::query_scalar::<_, i64>(
                r#"
                select count(*)
                  from extension_artifact_instances
                 where node_id = $1 and artifact_status <> 'ready'
                "#,
            )
            .bind(&self.node_id)
            .fetch_one(&pool)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
            if unhealthy_node_artifacts != 0 {
                return Err(PostRestoreDependencyError);
            }
            let missing_active_installations = sqlx::query_scalar::<_, i64>(
                r#"
                select count(*)
                  from extension_installations installation
                 where installation.desired_state = 'active_requested'
                   and not exists (
                       select 1
                         from extension_artifact_instances artifact
                        where artifact.node_id = $1
                          and artifact.installation_id = installation.id
                          and artifact.artifact_status = 'ready'
                   )
                "#,
            )
            .bind(&self.node_id)
            .fetch_one(&pool)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
            if missing_active_installations != 0 {
                return Err(PostRestoreDependencyError);
            }
            Ok(())
        }
        .await;
        pool.close().await;
        result
    }

    async fn verify_business_object(
        &self,
        component: &BackupComponent,
    ) -> Result<(), PostRestoreDependencyError> {
        let BackupComponentRestoreTarget::BusinessObject {
            storage_id,
            object_path,
        } = &component.restore_target
        else {
            return Err(PostRestoreDependencyError);
        };
        if component.disposition != BackupComponentDisposition::Embedded {
            return Err(PostRestoreDependencyError);
        }
        let storage = self
            .object_resolver
            .resolve(*storage_id)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        let driver = self
            .object_registry
            .get(&storage.driver_type)
            .ok_or(PostRestoreDependencyError)?;
        driver
            .validate_config(&storage.config_json)
            .map_err(|_| PostRestoreDependencyError)?;
        let opened = driver
            .open_read_stream(storage_object::OpenReadInput {
                config_json: &storage.config_json,
                object_path,
            })
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        let snapshot = opened.snapshot;
        if snapshot.content_length != component.size_bytes {
            return Err(PostRestoreDependencyError);
        }
        verify_reader(opened.reader, component).await?;
        driver
            .verify_read_unchanged(storage_object::VerifyReadUnchangedInput {
                config_json: &storage.config_json,
                object_path,
                snapshot: &snapshot,
            })
            .await
            .map_err(|_| PostRestoreDependencyError)
    }

    async fn verify_artifact(
        &self,
        component: &BackupComponent,
    ) -> Result<(), PostRestoreDependencyError> {
        let BackupComponentRestoreTarget::Artifact {
            category,
            organization,
            artifact_id,
            version,
        } = &component.restore_target
        else {
            return Err(PostRestoreDependencyError);
        };
        let coordinate = ArtifactRecoveryCoordinate {
            category: category.clone(),
            organization: organization.clone(),
            artifact_id: artifact_id.clone(),
            version: version.clone(),
            source_identity: component.source_identity.clone(),
        };
        match component.disposition {
            BackupComponentDisposition::Embedded => {
                let path = self
                    .artifact_resolver
                    .embedded_target(&coordinate)
                    .await
                    .map_err(|_| PostRestoreDependencyError)?;
                let metadata = tokio::fs::metadata(&path)
                    .await
                    .map_err(|_| PostRestoreDependencyError)?;
                if !metadata.is_file() || metadata.len() != component.size_bytes {
                    return Err(PostRestoreDependencyError);
                }
                let file = tokio::fs::File::open(path)
                    .await
                    .map_err(|_| PostRestoreDependencyError)?;
                verify_reader(Box::pin(file), component).await
            }
            BackupComponentDisposition::IdentityOnly => {
                let identity_digest = digest(component.source_identity.as_str().as_bytes())?;
                if identity_digest != component.content_digest {
                    return Err(PostRestoreDependencyError);
                }
                self.artifact_resolver
                    .verify_rebuildable(&coordinate)
                    .await
                    .map_err(|_| PostRestoreDependencyError)
            }
        }
    }
}

#[async_trait]
impl PostRestoreHealthVerifier for PostgreSqlPostRestoreHealthVerifier {
    async fn verify(
        &self,
        context: &PostRestoreRecoveryContext,
    ) -> Result<(), PostRestoreDependencyError> {
        let sealed = self
            .repository
            .load_manifest(context.backup_set_id)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        self.verify_database(context, &self.expected_migration_head)
            .await?;
        for component in sealed.manifest().components() {
            match component.kind {
                BackupComponentKind::PostgreSql => {}
                BackupComponentKind::BusinessObject => {
                    self.verify_business_object(component).await?
                }
                BackupComponentKind::ExtensionArtifact | BackupComponentKind::McpArtifact => {
                    self.verify_artifact(component).await?
                }
            }
        }
        Ok(())
    }
}

async fn verify_reader(
    mut reader: control_plane::ports::BackupComponentReader,
    component: &BackupComponent,
) -> Result<(), PostRestoreDependencyError> {
    let mut hasher = Sha256::new();
    let mut size_bytes = 0_u64;
    let mut buffer = vec![0_u8; 256 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        if read == 0 {
            break;
        }
        size_bytes = size_bytes
            .checked_add(read as u64)
            .ok_or(PostRestoreDependencyError)?;
        if size_bytes > component.size_bytes {
            return Err(PostRestoreDependencyError);
        }
        hasher.update(&buffer[..read]);
    }
    let content_digest = ContentDigest::try_from(format!("{:x}", hasher.finalize()))
        .map_err(|_| PostRestoreDependencyError)?;
    if size_bytes != component.size_bytes || content_digest != component.content_digest {
        return Err(PostRestoreDependencyError);
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> Result<ContentDigest, PostRestoreDependencyError> {
    ContentDigest::try_from(format!("{:x}", Sha256::digest(bytes)))
        .map_err(|_| PostRestoreDependencyError)
}

#[derive(Clone)]
pub struct PostgreSqlRecoveryAuditProjector {
    database_url: String,
}

impl PostgreSqlRecoveryAuditProjector {
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
        }
    }
}

#[async_trait]
impl RecoveryAuditProjector for PostgreSqlRecoveryAuditProjector {
    async fn project(
        &self,
        projection: &RecoveryAuditProjection,
    ) -> Result<(), PostRestoreDependencyError> {
        let payload = serde_json::json!({
            "source_event_id": projection.source_event_id,
            "recovery_job_id": projection.recovery_job_id,
            "backup_set_id": projection.backup_set_id,
            "safety_backup_set_id": projection.safety_backup_set_id,
            "outcome": projection.outcome.as_str(),
            "failure_code": projection.failure_code,
            "before_snapshot": projection.before_snapshot,
            "requested_target_snapshot": projection.requested_target_snapshot,
            "effective_after_snapshot": projection.effective_after_snapshot,
        });
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&self.database_url)
            .await
            .map_err(|_| PostRestoreDependencyError)?;
        let projected = sqlx::query_scalar::<_, uuid::Uuid>(
            r#"
            insert into audit_logs (
                id,
                workspace_id,
                scope_id,
                actor_user_id,
                target_type,
                target_id,
                event_code,
                payload,
                created_by,
                updated_by,
                created_at,
                updated_at
            )
            values ($1, null, $2, $3, 'system_recovery', $4,
                    $5, $6, $3, $3, $7, $7)
            on conflict (id) do update set id = excluded.id
             where audit_logs.workspace_id is not distinct from excluded.workspace_id
               and audit_logs.scope_id = excluded.scope_id
               and audit_logs.actor_user_id is not distinct from excluded.actor_user_id
               and audit_logs.target_type = excluded.target_type
               and audit_logs.target_id is not distinct from excluded.target_id
               and audit_logs.event_code = excluded.event_code
               and audit_logs.payload = excluded.payload
               and audit_logs.created_at = excluded.created_at
            returning id
            "#,
        )
        .bind(projection.audit_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(projection.actor_user_id)
        .bind(projection.recovery_job_id.as_uuid())
        .bind(projection.outcome.event_code())
        .bind(payload)
        .bind(projection.occurred_at)
        .fetch_optional(&pool)
        .await
        .map_err(|_| PostRestoreDependencyError)?;
        pool.close().await;
        if projected == Some(projection.audit_id) {
            Ok(())
        } else {
            Err(PostRestoreDependencyError)
        }
    }
}
