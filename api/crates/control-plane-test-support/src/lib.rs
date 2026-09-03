//! Test-only bootstrap boundary for persistence adapter fixtures.
//!
//! This crate intentionally delegates authorization truth to the control-plane bootstrap entry
//! points. It must remain a dev-dependency of concrete storage adapters.

use control_plane::ports::BootstrapRepository;
use uuid::Uuid;

pub async fn upsert_permission_catalog<R>(repository: &R) -> anyhow::Result<()>
where
    R: BootstrapRepository,
{
    control_plane::bootstrap::upsert_permission_catalog(repository).await
}

pub async fn upsert_builtin_roles<R>(repository: &R, workspace_id: Uuid) -> anyhow::Result<()>
where
    R: BootstrapRepository,
{
    control_plane::bootstrap::upsert_builtin_roles(repository, workspace_id).await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use async_trait::async_trait;
    use control_plane::ports::{BootstrapRepository, WorkspaceBootstrapResult};
    use domain::{LoginEntryRecord, PermissionDefinition, RoleTemplate, TenantRecord, UserRecord};
    use uuid::Uuid;

    #[derive(Debug, PartialEq, Eq)]
    enum BootstrapWrite {
        PermissionCatalog(Vec<PermissionDefinition>),
        RootRoles(Uuid, Vec<RoleTemplate>),
        WorkspaceRoles(Uuid, Vec<RoleTemplate>),
    }

    #[derive(Default)]
    struct RecordingBootstrapRepository {
        writes: Mutex<Vec<BootstrapWrite>>,
    }

    impl RecordingBootstrapRepository {
        fn writes(self) -> Vec<BootstrapWrite> {
            self.writes
                .into_inner()
                .expect("bootstrap write recording lock must not be poisoned")
        }
    }

    #[async_trait]
    impl BootstrapRepository for RecordingBootstrapRepository {
        async fn replace_login_entry_public_ui_block_if_matches(
            &self,
            _login_entry_id: Uuid,
            _expected: &str,
            _replacement: &str,
        ) -> anyhow::Result<bool> {
            unreachable!("authorization fixture does not bootstrap login_entries")
        }

        async fn upsert_login_entry(
            &self,
            _authenticator: &LoginEntryRecord,
        ) -> anyhow::Result<()> {
            unreachable!("authorization fixture does not bootstrap login_entries")
        }

        async fn upsert_permission_catalog(
            &self,
            permissions: &[PermissionDefinition],
        ) -> anyhow::Result<()> {
            self.writes
                .lock()
                .expect("bootstrap write recording lock must not be poisoned")
                .push(BootstrapWrite::PermissionCatalog(permissions.to_vec()));
            Ok(())
        }

        async fn upsert_root_tenant(&self) -> anyhow::Result<TenantRecord> {
            unreachable!("authorization fixture does not bootstrap tenants")
        }

        async fn root_workspace_requires_official_catalog_seed(
            &self,
            _workspace_name: &str,
        ) -> anyhow::Result<bool> {
            unreachable!("authorization fixture does not bootstrap workspaces")
        }

        async fn upsert_workspace_for_bootstrap(
            &self,
            _tenant_id: Uuid,
            _workspace_name: &str,
        ) -> anyhow::Result<WorkspaceBootstrapResult> {
            unreachable!("authorization fixture does not bootstrap workspaces")
        }

        async fn upsert_root_workspace_with_official_catalog_for_bootstrap(
            &self,
            _tenant_id: Uuid,
            _workspace_name: &str,
            _seed: &control_plane::i18n_catalog::VerifiedOfficialCatalogSeed,
        ) -> anyhow::Result<WorkspaceBootstrapResult> {
            unreachable!("authorization fixture does not bootstrap workspaces")
        }

        async fn upsert_root_role(
            &self,
            workspace_id: Uuid,
            templates: &[RoleTemplate],
        ) -> anyhow::Result<()> {
            self.writes
                .lock()
                .expect("bootstrap write recording lock must not be poisoned")
                .push(BootstrapWrite::RootRoles(workspace_id, templates.to_vec()));
            Ok(())
        }

        async fn seed_workspace_role_templates(
            &self,
            workspace_id: Uuid,
            templates: &[RoleTemplate],
        ) -> anyhow::Result<()> {
            self.writes
                .lock()
                .expect("bootstrap write recording lock must not be poisoned")
                .push(BootstrapWrite::WorkspaceRoles(
                    workspace_id,
                    templates.to_vec(),
                ));
            Ok(())
        }

        async fn upsert_root_user(
            &self,
            _workspace_id: Uuid,
            _account: &str,
            _email: &str,
            _password_hash: &str,
            _name: &str,
            _nickname: &str,
        ) -> anyhow::Result<UserRecord> {
            unreachable!("authorization fixture does not bootstrap users")
        }
    }

    #[tokio::test]
    async fn authorization_bootstrap_delegates_canonical_catalog_templates_and_order() {
        let repository = RecordingBootstrapRepository::default();
        let canonical_repository = RecordingBootstrapRepository::default();
        let workspace_id = Uuid::now_v7();

        super::upsert_permission_catalog(&repository).await.unwrap();
        super::upsert_builtin_roles(&repository, workspace_id)
            .await
            .unwrap();

        BootstrapRepository::upsert_permission_catalog(
            &canonical_repository,
            &access_control::permission_catalog(),
        )
        .await
        .unwrap();
        BootstrapRepository::upsert_builtin_roles(
            &canonical_repository,
            workspace_id,
            &access_control::bootstrap_role_templates(),
        )
        .await
        .unwrap();

        assert_eq!(repository.writes(), canonical_repository.writes());
    }
}
