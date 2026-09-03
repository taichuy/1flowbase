use super::*;

#[async_trait]
impl AuthRepository for ApplicationPublicApiTestRepository {
    async fn find_authentication_connection(
        &self,
        _id: Uuid,
    ) -> Result<Option<domain::AuthenticationConnectionRecord>> {
        anyhow::bail!("find_authentication_connection not implemented")
    }

    async fn find_user_for_verified_external_identity(
        &self,
        _identity: &domain::VerifiedExternalIdentity,
    ) -> Result<Option<domain::UserRecord>> {
        anyhow::bail!("find_user_for_verified_external_identity not implemented")
    }

    async fn bind_verified_external_identity(
        &self,
        _user_id: Uuid,
        _identity: &domain::VerifiedExternalIdentity,
        _audit: &domain::AuditLogRecord,
    ) -> Result<domain::UserAuthIdentity> {
        anyhow::bail!("bind_verified_external_identity not implemented")
    }

    async fn find_login_entry(&self, _id: Uuid) -> Result<Option<domain::LoginEntryRecord>> {
        anyhow::bail!("find_login_entry not implemented")
    }

    async fn find_user_for_password_login(
        &self,
        _connection_id: Uuid,
        _identifier: &str,
    ) -> Result<Option<domain::UserRecord>> {
        anyhow::bail!("find_user_for_password_login not implemented")
    }

    async fn find_user_by_id(&self, _user_id: Uuid) -> Result<Option<domain::UserRecord>> {
        anyhow::bail!("find_user_by_id not implemented")
    }

    async fn default_scope_for_user(&self, _user_id: Uuid) -> Result<domain::ScopeContext> {
        Ok(domain::ScopeContext {
            tenant_id: TEST_TENANT_ID,
            workspace_id: TEST_WORKSPACE_ID,
        })
    }

    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> Result<domain::ActorContext> {
        if actor_user_id == TEST_ROOT_USER_ID {
            return Ok(domain::ActorContext::root_in_scope(
                actor_user_id,
                TEST_TENANT_ID,
                TEST_WORKSPACE_ID,
                "root",
            ));
        }

        let permissions = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .actor_permissions
            .clone();

        Ok(domain::ActorContext::scoped_in_scope(
            actor_user_id,
            TEST_TENANT_ID,
            TEST_WORKSPACE_ID,
            "member",
            permissions,
        ))
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        _display_role: Option<&str>,
    ) -> Result<domain::ActorContext> {
        if user_id == TEST_ROOT_USER_ID {
            return Ok(domain::ActorContext::root_in_scope(
                user_id,
                tenant_id,
                workspace_id,
                "root",
            ));
        }

        let permissions = self
            .inner
            .lock()
            .expect("application public api test repo mutex poisoned")
            .actor_permissions
            .clone();

        Ok(domain::ActorContext::scoped_in_scope(
            user_id,
            tenant_id,
            workspace_id,
            "member",
            permissions,
        ))
    }

    async fn update_password_hash(
        &self,
        _user_id: Uuid,
        _password_hash: &str,
        _actor_id: Uuid,
    ) -> Result<i64> {
        anyhow::bail!("update_password_hash not implemented")
    }

    async fn update_profile(&self, _input: &UpdateProfileInput) -> Result<domain::UserRecord> {
        anyhow::bail!("update_profile not implemented")
    }

    async fn update_user_meta(
        &self,
        _input: &control_plane::ports::UpdateUserMetaInput,
    ) -> Result<domain::UserRecord> {
        anyhow::bail!("update_user_meta not implemented")
    }

    async fn bump_session_version(&self, _user_id: Uuid, _actor_id: Uuid) -> Result<i64> {
        anyhow::bail!("bump_session_version not implemented")
    }

    async fn list_permissions(&self) -> Result<Vec<domain::PermissionDefinition>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, _event: &domain::AuditLogRecord) -> Result<()> {
        Ok(())
    }
}
