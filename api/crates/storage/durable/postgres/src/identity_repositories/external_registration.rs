//! Atomic external onboarding. Identity is the idempotency key; email never merges users.
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{AuthRepository, CreateExternalRegisteredMemberInput, ExternalRegistrationRepository},
    ControlPlaneContractError as ControlPlaneError,
};
use sha2::{Digest, Sha256};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::repositories::{primary_workspace_id, PgControlPlaneStore};

#[async_trait]
impl ExternalRegistrationRepository for PgControlPlaneStore {
    async fn create_external_registered_member(
        &self,
        input: &CreateExternalRegisteredMemberInput,
    ) -> Result<domain::UserRecord> {
        let identity = &input.identity;
        let tuple = serde_json::to_vec(&(
            identity.connection_id(),
            identity.issuer(),
            identity.realm(),
            identity.subject_type(),
            identity.subject_value(),
        ))?;
        let digest = Sha256::digest(&tuple);
        let lock_key = i64::from_be_bytes(digest[..8].try_into().expect("SHA256 has eight bytes"));
        let workspace_id = primary_workspace_id(self.pool()).await?;
        if workspace_id == domain::SYSTEM_SCOPE_ID {
            return Err(ControlPlaneError::InvalidInput("default_member_role").into());
        }
        let mut tx = self.pool().begin().await?;
        // Serialize same-identity registrations before any user is inserted. Different
        // identities still rely on the users.email unique constraint, never email lookup.
        sqlx::query("select pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut *tx)
            .await?;
        let existing_user_id: Option<Uuid> = sqlx::query_scalar(
            "select user_id from user_auth_identities where connection_id = $1 and issuer is not distinct from $2 and realm is not distinct from $3 and subject_type = $4 and subject_value = $5 and verified = true"
        ).bind(identity.connection_id()).bind(identity.issuer()).bind(identity.realm()).bind(identity.subject_type()).bind(identity.subject_value())
            .fetch_optional(&mut *tx).await?;
        if let Some(user_id) = existing_user_id {
            tx.commit().await?;
            return self
                .find_user_by_id(user_id)
                .await?
                .ok_or_else(|| anyhow!("external identity user missing"));
        }
        let user_id = Uuid::now_v7();
        let result = insert_external_member(&mut tx, workspace_id, user_id, input).await;
        if let Err(error) = result {
            let unique_conflict = error
                .downcast_ref::<sqlx::Error>()
                .and_then(sqlx::Error::as_database_error)
                .and_then(|error| error.code())
                .as_deref()
                == Some("23505");
            tx.rollback().await?;
            if unique_conflict {
                // An explicit binding may race outside the registration advisory lock.
                // Return its identity owner only after all provisional user writes rolled back.
                if let Some(user) = self
                    .find_user_for_verified_external_identity(identity)
                    .await?
                {
                    return Ok(user);
                }
                return Err(
                    ControlPlaneError::Conflict("external_registration_identity_exists").into(),
                );
            }
            return Err(error);
        }
        tx.commit().await?;
        self.find_user_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("externally registered member missing after creation"))
    }
}

async fn insert_external_member(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    user_id: Uuid,
    input: &CreateExternalRegisteredMemberInput,
) -> Result<()> {
    let default_role: (Uuid, String, Uuid) = sqlx::query_as(
        "select id, code, scope_id from roles where scope_kind = 'workspace' and workspace_id = $1 and scope_id = $1 and is_default_member_role = true and system_kind is null and code not in ('root', 'system') order by id limit 1 for share"
    ).bind(workspace_id).fetch_optional(&mut **tx).await?.ok_or(ControlPlaneError::NotFound("default_member_role"))?;
    let account = format!("external_{}", user_id.simple());
    sqlx::query(
        "insert into users (id, account, email, phone, password_hash, name, nickname, avatar_url, introduction, default_display_role, email_login_enabled, phone_login_enabled, status, session_version, created_by, updated_by) values ($1, $2, $3, null, '!external-identity-no-password', $2, $2, null, '', $4, false, false, 'active', 1, null, null)"
    ).bind(user_id).bind(&account).bind(&input.email).bind(&default_role.1).execute(&mut **tx).await?;
    sqlx::query("insert into workspace_memberships (id, workspace_id, user_id, introduction, created_by, updated_by) values ($1, $2, $3, '', null, null)")
        .bind(Uuid::now_v7()).bind(workspace_id).bind(user_id).execute(&mut **tx).await?;
    sqlx::query("insert into user_role_bindings (id, user_id, role_id, scope_id, created_by, updated_by) values ($1, $2, $3, $4, null, null)")
        .bind(Uuid::now_v7()).bind(user_id).bind(default_role.0).bind(default_role.2).execute(&mut **tx).await?;
    let identity = &input.identity;
    sqlx::query(
        "insert into user_auth_identities (id, user_id, connection_id, subject_type, subject_value, issuer, realm, profile, verified, metadata, created_by, updated_by) values ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, null, null)"
    ).bind(Uuid::now_v7()).bind(user_id).bind(identity.connection_id()).bind(identity.subject_type()).bind(identity.subject_value())
        .bind(identity.issuer()).bind(identity.realm()).bind(identity.profile()).bind(identity.metadata()).execute(&mut **tx).await?;
    let audit = &input.audit;
    sqlx::query(
        "insert into audit_logs (id, workspace_id, scope_id, actor_user_id, target_type, target_id, event_code, payload, created_by, updated_by, created_at, updated_at) values ($1, $2, $2, $3, $4, $5, $6, $7, $3, $3, $8, $8)"
    ).bind(audit.id).bind(workspace_id).bind(audit.actor_user_id).bind(&audit.target_type).bind(user_id).bind(&audit.event_code).bind(&audit.payload).bind(audit.created_at).execute(&mut **tx).await?;
    Ok(())
}
