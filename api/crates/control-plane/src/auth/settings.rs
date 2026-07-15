use std::collections::HashSet;

use access_control::SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_ID;
use anyhow::Result;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{AuthRepository, AuthenticatorSettingsRepository, RoleConsolePolicyReader},
};

use super::AuthenticatorRegistry;

pub struct AuthCenterSettingsOverview {
    pub default_authenticator_id: Uuid,
    pub supported_auth_types: Vec<String>,
    pub authenticators: Vec<domain::AuthenticatorRecord>,
}

pub struct CreateAuthCenterAuthenticatorCommand {
    pub auth_type: String,
    pub title: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub sort_order: Option<i32>,
}

pub struct CopyAuthCenterAuthenticatorCommand {
    pub source_id: Uuid,
    pub title: String,
    pub sort_order: Option<i32>,
}

pub struct UpdateAuthCenterAuthenticatorCommand {
    pub authenticator_id: Uuid,
    pub title: String,
    pub enabled: bool,
    pub description: Option<Option<String>>,
}

pub struct AuthCenterSettingsService<R> {
    repository: R,
}

const AUTH_CENTER_OVERVIEW_VIEW_OPERATION_ID: &str = "auth_center.overview.view";
const AUTH_CENTER_AUTHENTICATOR_CREATE_OPERATION_ID: &str = "auth_center.authenticators.create";
const AUTH_CENTER_AUTHENTICATOR_COPY_OPERATION_ID: &str = "auth_center.authenticators.copy";
const AUTH_CENTER_AUTHENTICATOR_DELETE_OPERATION_ID: &str = "auth_center.authenticators.delete";
const AUTH_CENTER_AUTHENTICATOR_ENABLE_OPERATION_ID: &str = "auth_center.authenticators.enable";
const AUTH_CENTER_AUTHENTICATOR_ORDER_OPERATION_ID: &str = "auth_center.authenticators.order";
const AUTH_CENTER_AUTHENTICATOR_UPDATE_OPERATION_ID: &str = "auth_center.authenticators.update";

impl<R> AuthCenterSettingsService<R>
where
    R: AuthRepository + AuthenticatorSettingsRepository + RoleConsolePolicyReader,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn overview(
        &self,
        actor: &domain::ActorContext,
    ) -> Result<AuthCenterSettingsOverview> {
        self.ensure_console_operation(actor, AUTH_CENTER_OVERVIEW_VIEW_OPERATION_ID)
            .await?;
        Ok(AuthCenterSettingsOverview {
            default_authenticator_id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            supported_auth_types: supported_auth_types(),
            authenticators: self.repository.list_authenticators().await?,
        })
    }

    pub async fn create_authenticator(
        &self,
        actor: &domain::ActorContext,
        command: CreateAuthCenterAuthenticatorCommand,
    ) -> Result<domain::AuthenticatorRecord> {
        self.ensure_console_operation(actor, AUTH_CENTER_AUTHENTICATOR_CREATE_OPERATION_ID)
            .await?;
        validate_supported_auth_type(&command.auth_type)?;
        validate_authenticator_title(&command.title)?;
        let sort_order = match command.sort_order {
            Some(sort_order) => sort_order,
            None => self.next_sort_order().await?,
        };
        let authenticator = domain::AuthenticatorRecord {
            id: Uuid::now_v7(),
            auth_type: command.auth_type,
            title: command.title,
            enabled: command.enabled,
            is_builtin: false,
            sort_order,
            options: new_authenticator_options(command.description),
        };
        self.repository.create_authenticator(&authenticator).await?;
        Ok(authenticator)
    }

    pub async fn copy_authenticator(
        &self,
        actor: &domain::ActorContext,
        command: CopyAuthCenterAuthenticatorCommand,
    ) -> Result<domain::AuthenticatorRecord> {
        self.ensure_console_operation(actor, AUTH_CENTER_AUTHENTICATOR_COPY_OPERATION_ID)
            .await?;
        validate_authenticator_title(&command.title)?;
        let source = self
            .repository
            .find_authenticator(command.source_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("authenticator"))?;
        validate_supported_auth_type(&source.auth_type)?;
        let sort_order = match command.sort_order {
            Some(sort_order) => sort_order,
            None => self.next_sort_order().await?,
        };
        let authenticator = domain::AuthenticatorRecord {
            id: Uuid::now_v7(),
            auth_type: source.auth_type,
            title: command.title,
            enabled: false,
            is_builtin: false,
            sort_order,
            options: source.options,
        };
        self.repository.create_authenticator(&authenticator).await?;
        Ok(authenticator)
    }

    pub async fn delete_authenticator(
        &self,
        actor: &domain::ActorContext,
        authenticator_id: Uuid,
    ) -> Result<()> {
        self.ensure_console_operation(actor, AUTH_CENTER_AUTHENTICATOR_DELETE_OPERATION_ID)
            .await?;
        self.repository
            .delete_authenticator_if_unbound(authenticator_id)
            .await
    }

    pub async fn reorder_authenticators(
        &self,
        actor: &domain::ActorContext,
        ids: &[Uuid],
    ) -> Result<AuthCenterSettingsOverview> {
        self.ensure_console_operation(actor, AUTH_CENTER_AUTHENTICATOR_ORDER_OPERATION_ID)
            .await?;
        let existing = self.repository.list_authenticators().await?;
        validate_reorder_ids(ids, &existing)?;
        self.repository.update_authenticator_order(ids).await?;
        Ok(AuthCenterSettingsOverview {
            default_authenticator_id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            supported_auth_types: supported_auth_types(),
            authenticators: self.repository.list_authenticators().await?,
        })
    }

    pub async fn enable_authenticator(
        &self,
        actor: &domain::ActorContext,
        authenticator_id: Uuid,
    ) -> Result<domain::AuthenticatorRecord> {
        self.ensure_console_operation(actor, AUTH_CENTER_AUTHENTICATOR_ENABLE_OPERATION_ID)
            .await?;
        let mut authenticator = self
            .repository
            .find_authenticator(authenticator_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("authenticator"))?;
        authenticator.enabled = true;
        self.repository
            .update_authenticator_config(&authenticator)
            .await?;
        Ok(authenticator)
    }

    pub async fn update_authenticator(
        &self,
        actor: &domain::ActorContext,
        command: UpdateAuthCenterAuthenticatorCommand,
    ) -> Result<domain::AuthenticatorRecord> {
        self.ensure_console_operation(actor, AUTH_CENTER_AUTHENTICATOR_UPDATE_OPERATION_ID)
            .await?;
        validate_authenticator_title(&command.title)?;
        let mut authenticator = self
            .repository
            .find_authenticator(command.authenticator_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("authenticator"))?;
        authenticator.title = command.title;
        authenticator.enabled = command.enabled;
        if let Some(description) = command.description {
            upsert_description(&mut authenticator.options, description);
        }
        self.repository
            .update_authenticator_config(&authenticator)
            .await?;
        Ok(authenticator)
    }

    async fn next_sort_order(&self) -> Result<i32> {
        Ok(self
            .repository
            .list_authenticators()
            .await?
            .into_iter()
            .map(|authenticator| authenticator.sort_order)
            .max()
            .unwrap_or(0)
            + 10)
    }

    async fn ensure_console_operation(
        &self,
        actor: &domain::ActorContext,
        operation_id: &str,
    ) -> Result<()> {
        if actor.is_root {
            return Ok(());
        }
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor.user_id, actor.current_workspace_id)
            .await?;
        let operation_id = domain::ConsoleOperationId::try_from(operation_id)
            .expect("compiled auth-center operation id must be valid");
        if domain::effective_console_simple_operation(
            &policies,
            &auth_center_console_group(),
            &operation_id,
        ) {
            Ok(())
        } else {
            Err(ControlPlaneError::PermissionDenied("permission_denied").into())
        }
    }
}

fn auth_center_console_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(SYSTEM_AUTH_CENTER_SETTINGS_FEATURE_ID)
        .expect("compiled auth-center settings feature id must be valid")
}

fn supported_auth_types() -> Vec<String> {
    AuthenticatorRegistry::new().supported_auth_types()
}

fn validate_supported_auth_type(auth_type: &str) -> Result<()> {
    if supported_auth_types()
        .iter()
        .any(|supported| supported == auth_type)
    {
        Ok(())
    } else {
        Err(ControlPlaneError::InvalidInput("auth_type").into())
    }
}

fn validate_authenticator_title(title: &str) -> Result<()> {
    if title.trim().is_empty() {
        return Err(ControlPlaneError::InvalidInput("title").into());
    }
    Ok(())
}

fn validate_reorder_ids(
    requested_ids: &[Uuid],
    existing_authenticators: &[domain::AuthenticatorRecord],
) -> Result<()> {
    let mut seen = HashSet::new();
    for id in requested_ids {
        if !seen.insert(*id) {
            return Err(ControlPlaneError::InvalidInput("authenticator_order_duplicate").into());
        }
    }
    let existing_ids = existing_authenticators
        .iter()
        .map(|authenticator| authenticator.id)
        .collect::<HashSet<_>>();
    if requested_ids.iter().any(|id| !existing_ids.contains(id)) {
        return Err(ControlPlaneError::InvalidInput("authenticator_order_unknown").into());
    }
    if requested_ids.len() != existing_authenticators.len() {
        return Err(ControlPlaneError::InvalidInput("authenticator_order_missing").into());
    }
    Ok(())
}

fn new_authenticator_options(description: Option<String>) -> Value {
    let mut options = Map::new();
    if let Some(description) = description {
        options.insert("description".to_string(), Value::String(description));
    }
    options.insert(
        "config_form_schema".to_string(),
        json!([
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
        ]),
    );
    options.insert("extension_config".to_string(), Value::Object(Map::new()));
    Value::Object(options)
}

fn upsert_description(options: &mut Value, description: Option<String>) {
    if !options.is_object() {
        *options = Value::Object(Map::new());
    }
    if let Some(values) = options.as_object_mut() {
        match description {
            Some(description) => {
                values.insert("description".to_string(), Value::String(description));
            }
            None => {
                values.remove("description");
            }
        }
    }
}
