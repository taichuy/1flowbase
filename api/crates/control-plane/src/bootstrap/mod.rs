use access_control::permission_catalog;
use anyhow::Result;
use domain::AuthenticatorRecord;

use crate::ports::BootstrapRepository;

#[derive(Debug, Clone)]
pub struct BootstrapConfig {
    pub workspace_name: String,
    pub root_account: String,
    pub root_email: String,
    pub root_password_hash: String,
    pub root_name: String,
    pub root_nickname: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootstrapResult {
    pub workspace_id: uuid::Uuid,
    pub root_user_id: uuid::Uuid,
}

pub struct BootstrapService<R> {
    repository: R,
}

fn password_local_authenticator_options() -> serde_json::Value {
    serde_json::json!({
        "description": "Local password authentication",
        "config_form_schema": [
            {
                "key": "title",
                "label": "Authenticator title",
                "type": "string",
                "required": true
            },
            {
                "key": "description",
                "label": "Description",
                "type": "string",
                "control": "textarea",
                "read_only": false,
                "required": false
            },
            {
                "key": "enabled",
                "label": "Enabled",
                "type": "boolean",
                "control": "switch"
            }
        ],
        "extension_config": {}
    })
}

impl<R> BootstrapService<R>
where
    R: BootstrapRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn run(&self, config: &BootstrapConfig) -> Result<BootstrapResult> {
        self.repository
            .upsert_authenticator(&AuthenticatorRecord {
                id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
                auth_type: "password-local".into(),
                title: "Password".into(),
                enabled: true,
                is_builtin: true,
                sort_order: 0,
                options: password_local_authenticator_options(),
            })
            .await?;
        self.repository
            .upsert_permission_catalog(&permission_catalog())
            .await?;

        let tenant = self.repository.upsert_root_tenant().await?;
        let workspace = self
            .repository
            .upsert_workspace(tenant.id, &config.workspace_name)
            .await?;
        self.repository.upsert_builtin_roles(workspace.id).await?;
        let root_user = self
            .repository
            .upsert_root_user(
                workspace.id,
                &config.root_account,
                &config.root_email,
                &config.root_password_hash,
                &config.root_name,
                &config.root_nickname,
            )
            .await?;

        Ok(BootstrapResult {
            workspace_id: workspace.id,
            root_user_id: root_user.id,
        })
    }
}
