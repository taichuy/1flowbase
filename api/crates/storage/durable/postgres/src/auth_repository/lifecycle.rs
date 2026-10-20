//! Transactional execution of the control-plane identity lifecycle commands.
use super::*;
use control_plane_contracts::ports::{
    AuthenticationMethodsSnapshot, ExternalIdentityBinding, ExternalIdentityLifecycleRepository,
    PasswordAvailability,
};
use sha2::{Digest, Sha256};

pub(crate) async fn lock_canonical_identity(
    tx: &mut Transaction<'_, Postgres>,
    identity: &VerifiedExternalIdentity,
) -> Result<()> {
    let tuple = serde_json::to_vec(&(
        identity.authority_namespace(),
        identity.issuer().unwrap_or(""),
        identity.realm().unwrap_or(""),
        identity.subject_type(),
        identity.subject_value(),
    ))?;
    let digest = Sha256::digest(tuple);
    let key = i64::from_be_bytes(digest[..8].try_into().expect("SHA256 has eight bytes"));
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn binding(row: sqlx::postgres::PgRow) -> ExternalIdentityBinding {
    ExternalIdentityBinding {
        identity_id: row.get("id"),
        user_id: row.get("user_id"),
        connection_id: row.get("connection_id"),
        authority_namespace: row.get("authority_namespace"),
        subject_type: row.get("subject_type"),
        subject_value: row.get("subject_value"),
        issuer: row.get("issuer"),
        realm: row.get("realm"),
        state: row.get("state"),
        auth_type: row.get("auth_type"),
        profile: row.get("profile"),
        bound_at: row.get("bound_at"),
        unlinked_at: row.get("unlinked_at"),
        entry_enabled: row.get("entry_enabled"),
    }
}

const BINDING_SELECT: &str = "select i.*, c.auth_type, exists(select 1 from login_entries e join authentication_connections ec on ec.id=e.connection_id where ec.auth_type=c.auth_type and e.enabled) as entry_enabled from user_auth_identities i join authentication_connections c on c.id=i.connection_id";

pub(crate) fn identity_audit_payload(
    identity: &VerifiedExternalIdentity,
    owner: Uuid,
    old_state: Option<&str>,
    new_state: &str,
) -> serde_json::Value {
    serde_json::json!({"connection_id":identity.connection_id(), "authority_namespace":identity.authority_namespace(),
        "issuer":identity.issuer(), "realm":identity.realm(), "subject_type":identity.subject_type(),
        "subject_value":identity.subject_value(), "old_owner_user_id":old_state.map(|_| owner),
        "new_owner_user_id":owner, "old_state":old_state, "new_state":new_state})
}

pub(crate) async fn write_audit(
    tx: &mut Transaction<'_, Postgres>,
    audit: &AuditLogRecord,
    target_id: Uuid,
    payload: &serde_json::Value,
) -> Result<()> {
    sqlx::query("insert into audit_logs (id,workspace_id,scope_id,actor_user_id,target_type,target_id,event_code,payload,created_by,updated_by,created_at,updated_at) values ($1,$2,$2,$3,$4,$5,$6,$7,$3,$3,$8,$8)")
        .bind(audit.id).bind(audit.workspace_id).bind(audit.actor_user_id).bind(&audit.target_type)
        .bind(target_id).bind(&audit.event_code).bind(payload).bind(audit.created_at).execute(&mut **tx).await?;
    Ok(())
}

pub(crate) async fn bind_identity(
    store: &PgControlPlaneStore,
    user_id: Uuid,
    identity: &VerifiedExternalIdentity,
    audit: &AuditLogRecord,
) -> Result<UserAuthIdentity> {
    let mut tx = store.pool().begin().await?;
    lock_canonical_identity(&mut tx, identity).await?;
    let existing: Option<(Uuid, Uuid, String)> = sqlx::query_as("select id,user_id,state from user_auth_identities where authority_namespace=$1 and coalesce(issuer,'')=$2 and coalesce(realm,'')=$3 and subject_type=$4 and subject_value=$5")
        .bind(identity.authority_namespace()).bind(identity.issuer().unwrap_or("")).bind(identity.realm().unwrap_or(""))
        .bind(identity.subject_type()).bind(identity.subject_value()).fetch_optional(&mut *tx).await?;
    if existing
        .as_ref()
        .is_some_and(|(_, owner, _)| *owner != user_id)
    {
        return Err(ControlPlaneError::Conflict("external_identity_already_bound").into());
    }
    sqlx::query("select id from users where id=$1 for update")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("user"))?;
    let identity_id = existing
        .as_ref()
        .map(|(id, _, _)| *id)
        .unwrap_or_else(Uuid::now_v7);
    let old_state = existing.as_ref().map(|(_, _, state)| state.as_str());
    if old_state != Some("bound") {
        sqlx::query("insert into user_auth_identities (id,user_id,connection_id,authority_namespace,subject_type,subject_value,issuer,realm,profile,verified,metadata,created_by,updated_by) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,true,$10,$11,$11) on conflict (id) do update set state='bound',unlinked_at=null,bound_at=now(),connection_id=excluded.connection_id,profile=excluded.profile,metadata=excluded.metadata,verified=true,updated_by=excluded.updated_by,updated_at=now()")
            .bind(identity_id).bind(user_id).bind(identity.connection_id()).bind(identity.authority_namespace())
            .bind(identity.subject_type()).bind(identity.subject_value()).bind(identity.issuer()).bind(identity.realm())
            .bind(identity.profile()).bind(identity.metadata()).bind(audit.actor_user_id).execute(&mut *tx).await?;
        let mut payload = identity_audit_payload(identity, user_id, old_state, "bound");
        payload["identity_id"] = serde_json::json!(identity_id);
        write_audit(&mut tx, audit, identity_id, &payload).await?;
    }
    tx.commit().await?;
    Ok(UserAuthIdentity {
        user_id,
        connection_id: identity.connection_id(),
        subject_type: identity.subject_type().into(),
        subject_value: identity.subject_value().into(),
        issuer: identity.issuer().map(str::to_owned),
        realm: identity.realm().map(str::to_owned),
        profile: identity.profile().clone(),
    })
}

async fn lock_password_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    expected: &str,
) -> Result<String> {
    let hash: String = sqlx::query_scalar(
        "select password_hash from users where id=$1 and status='active' for update",
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ControlPlaneError::NotFound("user"))?;
    if hash != expected {
        return Err(ControlPlaneError::Conflict("authentication_methods_changed").into());
    }
    Ok(hash)
}

async fn password_login_available(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
) -> Result<bool> {
    Ok(sqlx::query_scalar("select exists(select 1 from user_auth_identities i join users u on u.id=i.user_id where i.user_id=$1 and i.connection_id=$2 and i.subject_type='account' and lower(i.subject_value)=lower(u.account) and i.state='bound' and i.verified) and exists(select 1 from login_entries e where e.connection_id=$2 and e.enabled)")
        .bind(user_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).fetch_one(&mut **tx).await?)
}

#[async_trait]
impl ExternalIdentityLifecycleRepository for PgControlPlaneStore {
    async fn find_external_identity_binding(
        &self,
        identity: &VerifiedExternalIdentity,
    ) -> Result<Option<ExternalIdentityBinding>> {
        let query = format!("{BINDING_SELECT} where i.authority_namespace=$1 and coalesce(i.issuer,'')=$2 and coalesce(i.realm,'')=$3 and i.subject_type=$4 and i.subject_value=$5 and i.verified");
        Ok(sqlx::query(&query)
            .bind(identity.authority_namespace())
            .bind(identity.issuer().unwrap_or(""))
            .bind(identity.realm().unwrap_or(""))
            .bind(identity.subject_type())
            .bind(identity.subject_value())
            .fetch_optional(self.pool())
            .await?
            .map(binding))
    }

    async fn authentication_methods_snapshot(
        &self,
        user_id: Uuid,
    ) -> Result<AuthenticationMethodsSnapshot> {
        let password_hash: String =
            sqlx::query_scalar("select password_hash from users where id=$1")
                .bind(user_id)
                .fetch_optional(self.pool())
                .await?
                .ok_or(ControlPlaneError::NotFound("user"))?;
        let local_account_identity = sqlx::query_scalar("select exists(select 1 from user_auth_identities i join users u on u.id=i.user_id where i.user_id=$1 and i.connection_id=$2 and i.subject_type='account' and lower(i.subject_value)=lower(u.account) and i.state='bound' and i.verified)")
            .bind(user_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).fetch_one(self.pool()).await?;
        let password_entry_enabled = sqlx::query_scalar(
            "select exists(select 1 from login_entries where connection_id=$1 and enabled)",
        )
        .bind(domain::PASSWORD_LOCAL_CONNECTION_ID)
        .fetch_one(self.pool())
        .await?;
        let query = format!("{BINDING_SELECT} where i.user_id=$1 and i.connection_id<>$2 and i.verified order by i.bound_at,i.id");
        let bindings = sqlx::query(&query)
            .bind(user_id)
            .bind(domain::PASSWORD_LOCAL_CONNECTION_ID)
            .fetch_all(self.pool())
            .await?
            .into_iter()
            .map(binding)
            .collect();
        Ok(AuthenticationMethodsSnapshot {
            password_hash,
            local_account_identity,
            password_entry_enabled,
            bindings,
        })
    }

    async fn unlink_external_identity_atomic(
        &self,
        user_id: Uuid,
        identity_id: Uuid,
        authorities: &[String],
        password: PasswordAvailability<'_>,
        audit: &AuditLogRecord,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        lock_password_snapshot(&mut tx, user_id, password.expected_hash).await?;
        let query = format!("{BINDING_SELECT} where i.id=$1 and i.user_id=$2 and i.connection_id<>$3 and i.verified for update of i");
        let target = sqlx::query(&query)
            .bind(identity_id)
            .bind(user_id)
            .bind(domain::PASSWORD_LOCAL_CONNECTION_ID)
            .fetch_optional(&mut *tx)
            .await?
            .map(binding)
            .ok_or(ControlPlaneError::NotFound("user_auth_identity"))?;
        if target.state != "bound" {
            return Err(ControlPlaneError::Conflict("external_identity_unlinked").into());
        }
        let alternatives: i64 = sqlx::query_scalar("select count(*) from user_auth_identities i where i.user_id=$1 and i.id<>$2 and i.connection_id<>$3 and i.state='bound' and i.verified and i.authority_namespace=any($4) and exists(select 1 from login_entries e join authentication_connections ec on ec.id=e.connection_id where ec.auth_type=(select auth_type from authentication_connections where id=i.connection_id) and e.enabled)")
            .bind(user_id).bind(identity_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).bind(authorities)
            .fetch_one(&mut *tx).await?;
        if alternatives == 0
            && !(password.hash_usable && password_login_available(&mut tx, user_id).await?)
        {
            return Err(ControlPlaneError::Conflict("last_usable_authentication_method").into());
        }
        sqlx::query("update user_auth_identities set state='unlinked',unlinked_at=now(),updated_at=now(),updated_by=$2 where id=$1")
            .bind(identity_id).bind(audit.actor_user_id).execute(&mut *tx).await?;
        let payload = serde_json::json!({"identity_id":target.identity_id,"connection_id":target.connection_id,
            "authority_namespace":target.authority_namespace,"issuer":target.issuer,"realm":target.realm,
            "subject_type":target.subject_type,"subject_value":target.subject_value,"old_owner_user_id":user_id,
            "new_owner_user_id":user_id,"old_state":"bound","new_state":"unlinked"});
        write_audit(&mut tx, audit, identity_id, &payload).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn initialize_password_atomic(
        &self,
        user_id: Uuid,
        verified_identity_id: Uuid,
        new_password_hash: &str,
        authorities: &[String],
        password: PasswordAvailability<'_>,
        audit: &AuditLogRecord,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        lock_password_snapshot(&mut tx, user_id, password.expected_hash).await?;
        if password.hash_usable {
            return Err(ControlPlaneError::Conflict("password_already_configured").into());
        }
        let query = format!("{BINDING_SELECT} where i.id=$1 and i.user_id=$2 and i.connection_id<>$3 and i.state='bound' and i.verified for update of i");
        let proof = sqlx::query(&query)
            .bind(verified_identity_id)
            .bind(user_id)
            .bind(domain::PASSWORD_LOCAL_CONNECTION_ID)
            .fetch_optional(&mut *tx)
            .await?
            .map(binding)
            .ok_or(ControlPlaneError::PermissionDenied(
                "password_initialization_identity_invalid",
            ))?;
        if !proof.entry_enabled || !authorities.contains(&proof.authority_namespace) {
            return Err(
                ControlPlaneError::PermissionDenied("authentication_provider_unavailable").into(),
            );
        }
        let account: String = sqlx::query_scalar("select account from users where id=$1")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        let previous_local: Option<(Uuid, String)> = sqlx::query_as("select user_id,state from user_auth_identities where connection_id=$1 and subject_type='account' and lower(subject_value)=lower($2)")
            .bind(domain::PASSWORD_LOCAL_CONNECTION_ID).bind(&account).fetch_optional(&mut *tx).await?;
        let (local_identity_id, owner): (Uuid, Uuid) = sqlx::query_as("insert into user_auth_identities (id,user_id,connection_id,subject_type,subject_value,created_by,updated_by) values ($1,$2,$3,'account',$4,$2,$2) on conflict (subject_type,lower(subject_value)) where connection_id='00000000-0000-0000-0000-000000000001'::uuid do update set subject_value=user_auth_identities.subject_value,state='bound',verified=true,unlinked_at=null,updated_by=excluded.updated_by,updated_at=now() returning id,user_id")
            .bind(Uuid::now_v7()).bind(user_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).bind(&account).fetch_one(&mut *tx).await?;
        if owner != user_id {
            return Err(ControlPlaneError::Conflict("password_account_identity_conflict").into());
        }
        sqlx::query("update users set password_hash=$2,session_version=session_version+1,updated_by=$1,updated_at=now() where id=$1")
            .bind(user_id).bind(new_password_hash).execute(&mut *tx).await?;
        let payload = serde_json::json!({"verified_identity_id":verified_identity_id,"authority_namespace":proof.authority_namespace,
            "connection_id":proof.connection_id,"issuer":proof.issuer,"realm":proof.realm,"subject_type":proof.subject_type,
            "subject_value":proof.subject_value,"old_owner_user_id":user_id,"new_owner_user_id":user_id,
            "old_state":"not_configured","new_state":"configured","account":account,
            "local_identity":{"identity_id":local_identity_id,"authority_namespace":format!("connection:{}",domain::PASSWORD_LOCAL_CONNECTION_ID),
                "connection_id":domain::PASSWORD_LOCAL_CONNECTION_ID,"issuer":null,"realm":null,"subject_type":"account",
                "subject_value":account,"old_owner_user_id":previous_local.as_ref().map(|(owner,_)| owner),"new_owner_user_id":user_id,"old_state":previous_local.as_ref().map(|(_,state)| state),"new_state":"bound"}});
        write_audit(&mut tx, audit, user_id, &payload).await?;
        tx.commit().await?;
        Ok(())
    }
}
