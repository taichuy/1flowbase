use super::*;

#[async_trait]
impl AuthRepository for MemoryPluginManagementRepository {
    async fn find_authentication_connection(
        &self,
        _id: Uuid,
    ) -> Result<Option<domain::AuthenticationConnectionRecord>> {
        anyhow::bail!("authentication connections are not used by plugin tests")
    }

    async fn find_user_for_verified_external_identity(
        &self,
        _identity: &domain::VerifiedExternalIdentity,
    ) -> Result<Option<domain::UserRecord>> {
        anyhow::bail!("external identities are not used by plugin tests")
    }

    async fn bind_verified_external_identity(
        &self,
        _user_id: Uuid,
        _identity: &domain::VerifiedExternalIdentity,
        _audit: &domain::AuditLogRecord,
    ) -> Result<domain::UserAuthIdentity> {
        anyhow::bail!("external identities are not used by plugin tests")
    }

    async fn find_login_entry(&self, _id: Uuid) -> Result<Option<LoginEntryRecord>> {
        Ok(None)
    }

    async fn find_user_for_password_login(
        &self,
        _connection_id: Uuid,
        _identifier: &str,
    ) -> Result<Option<UserRecord>> {
        Ok(None)
    }

    async fn find_user_by_id(&self, _user_id: Uuid) -> Result<Option<UserRecord>> {
        Ok(None)
    }

    async fn default_scope_for_user(&self, _user_id: Uuid) -> Result<ScopeContext> {
        Ok(ScopeContext {
            tenant_id: self.actor.tenant_id,
            workspace_id: self.actor.current_workspace_id,
        })
    }

    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        self.load_actor_context(
            actor_user_id,
            self.actor.tenant_id,
            self.actor.current_workspace_id,
            None,
        )
        .await
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        _display_role: Option<&str>,
    ) -> Result<ActorContext> {
        let mut actor = self.actor.clone();
        actor.user_id = user_id;
        actor.tenant_id = tenant_id;
        actor.current_workspace_id = workspace_id;
        Ok(actor)
    }

    async fn update_password_hash(
        &self,
        _user_id: Uuid,
        _password_hash: &str,
        _actor_id: Uuid,
    ) -> Result<i64> {
        Ok(1)
    }

    async fn update_profile(&self, _input: &UpdateProfileInput) -> Result<UserRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_user_meta(
        &self,
        _input: &control_plane::ports::UpdateUserMetaInput,
    ) -> Result<UserRecord> {
        anyhow::bail!("not implemented")
    }

    async fn bump_session_version(&self, _user_id: Uuid, _actor_id: Uuid) -> Result<i64> {
        Ok(1)
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionDefinition>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        if std::mem::take(&mut *self.fail_next_audit.write().await) {
            anyhow::bail!("fixture audit write rejected");
        }
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}
