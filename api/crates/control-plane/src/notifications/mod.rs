//! Workspace notification instances and committed inbox messages. Managed providers execute
//! through a host-owned port; the business service does not own worker or broadcast runtimes.
use crate::{
    errors::ControlPlaneError as Error,
    ports::{
        NotificationMessageListInput, NotificationRepository, PluginRepository,
        RoleConsolePolicyReader, SaveNotificationInstanceInput,
    },
};
use anyhow::Result;
use async_trait::async_trait;
use domain::{
    ActorContext, NotificationInstanceRecord, NotificationInstanceStatus, NotificationMessagePage,
    SystemNotificationRecord,
};
use extension_contracts::{ContributionId, NotificationTypeDescriptor};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeSet, sync::Arc};
use uuid::Uuid;

mod authorization;
mod managed_send;

pub use domain::{NotificationSendResult, NotificationSource};
pub use extension_contracts::{NotificationAuthorizationOperation, NotificationAuthorizationView};
pub const NOTIFICATIONS_SETTINGS_FEATURE: &str = "system.notifications";

#[async_trait]
pub trait NotificationSenderPort: Send + Sync {
    async fn send(
        &self,
        actor: &ActorContext,
        source: NotificationSource,
        instance_id: Uuid,
        input: Value,
    ) -> Result<NotificationSendResult>;
}
#[async_trait]
pub trait NotificationProviderPort: Send + Sync {
    async fn available(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        contribution_id: &ContributionId,
    ) -> Result<bool>;
    async fn send(
        &self,
        actor: &ActorContext,
        installation_id: Uuid,
        descriptor: &NotificationTypeDescriptor,
        config: Value,
        input: Value,
    ) -> Result<Value>;
}
pub trait NotificationEventPublisher: Send + Sync {
    fn publish(&self, workspace_id: Uuid, recipient_user_ids: &[Uuid]);
}

#[derive(Clone, Serialize)]
pub struct NotificationTypeCatalogEntry {
    #[serde(flatten)]
    pub descriptor: NotificationTypeDescriptor,
    pub installation_id: Option<Uuid>,
    pub available: bool,
    pub unavailable_reason: Option<&'static str>,
}
#[derive(Debug, Clone, Serialize)]
pub struct NotificationInstance {
    #[serde(flatten)]
    pub record: NotificationInstanceRecord,
    pub available: bool,
    pub unavailable_reason: Option<&'static str>,
    pub authorization: Option<NotificationAuthorizationView>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateNotificationInstance {
    pub type_code: String,
    pub installation_id: Option<Uuid>,
    pub display_name: String,
    pub config_json: Value,
    #[serde(default)]
    pub status: Option<NotificationInstanceStatus>,
}
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct PatchNotificationInstance {
    pub display_name: Option<String>,
    pub status: Option<NotificationInstanceStatus>,
    pub config_json: Option<Value>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct NotificationInstanceListInput {
    pub display_name: Option<String>,
    pub type_code: Option<String>,
    pub status: Option<NotificationInstanceStatus>,
}

pub struct NotificationService<R> {
    repository: R,
    master_key: String,
    provider: Option<Arc<dyn NotificationProviderPort>>,
    events: Option<Arc<dyn NotificationEventPublisher>>,
}
impl<R> NotificationService<R> {
    pub fn new(repository: R, master_key: String) -> Self {
        Self {
            repository,
            master_key,
            provider: None,
            events: None,
        }
    }
    pub fn with_provider(mut self, provider: Arc<dyn NotificationProviderPort>) -> Self {
        self.provider = Some(provider);
        self
    }
    pub fn with_events(mut self, events: Arc<dyn NotificationEventPublisher>) -> Self {
        self.events = Some(events);
        self
    }
}
impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader,
{
    pub async fn bootstrap(&self) -> Result<()> {
        self.repository
            .bootstrap_system_notification_instances()
            .await
    }
    async fn ensure_actor(&self, actor: &ActorContext) -> Result<()> {
        if !actor.is_root
            && !self
                .repository
                .notification_recipient_options(actor.current_workspace_id)
                .await?
                .iter()
                .any(|member| member.id == actor.user_id)
        {
            return Err(Error::PermissionDenied("workspace_access_denied").into());
        }
        Ok(())
    }
    async fn manage(&self, actor: &ActorContext, operation: &str) -> Result<()> {
        self.ensure_actor(actor).await?;
        let group = domain::ConsolePolicyGroup::settings_feature(NOTIFICATIONS_SETTINGS_FEATURE)?;
        let operation = domain::ConsoleOperationId::try_from(operation)?;
        if !actor.is_root
            && !domain::effective_console_simple_operation(
                &self
                    .repository
                    .load_role_console_policies_for_user(actor)
                    .await?,
                &group,
                &operation,
            )
        {
            return Err(Error::PermissionDenied("permission_denied").into());
        }
        Ok(())
    }
    pub async fn types(&self, actor: &ActorContext) -> Result<Vec<NotificationTypeCatalogEntry>> {
        self.ensure_actor(actor).await?;
        let mut builtin = system_notification_descriptor();
        builtin.input_form.fields[0].options = self
            .repository
            .notification_recipient_options(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|member| extension_contracts::PluginFormOption {
                label: member.display_name,
                value: json!(member.id),
                description: None,
                disabled: None,
            })
            .collect();
        let mut types = vec![NotificationTypeCatalogEntry {
            descriptor: builtin,
            installation_id: None,
            available: true,
            unavailable_reason: None,
        }];
        let assigned = self
            .repository
            .list_assignments(actor.current_workspace_id)
            .await?
            .into_iter()
            .map(|assignment| assignment.installation_id)
            .collect::<BTreeSet<_>>();
        for installation in self.repository.list_installations().await? {
            if !assigned.contains(&installation.id) {
                continue;
            }
            let Some(raw) = installation.metadata_json.get("notification_types") else {
                continue;
            };
            let descriptors: Vec<NotificationTypeDescriptor> = serde_json::from_value(raw.clone())
                .map_err(|_| Error::Conflict("notification_type_metadata"))?;
            for descriptor in descriptors {
                descriptor
                    .validate()
                    .map_err(|_| Error::Conflict("notification_type_metadata"))?;
                let active =
                    installation.desired_state == domain::PluginDesiredState::ActiveRequested;
                let available = if active {
                    match &self.provider {
                        Some(provider) => provider
                            .available(actor, installation.id, &descriptor.contribution_id)
                            .await
                            .unwrap_or(false),
                        None => false,
                    }
                } else {
                    false
                };
                types.push(NotificationTypeCatalogEntry {
                    descriptor,
                    installation_id: Some(installation.id),
                    available,
                    unavailable_reason: (!available).then_some("plugin_unavailable"),
                });
            }
        }
        Ok(types)
    }
    async fn type_entry(
        &self,
        actor: &ActorContext,
        type_code: &str,
        installation_id: Option<Uuid>,
    ) -> Result<NotificationTypeCatalogEntry> {
        self.types(actor)
            .await?
            .into_iter()
            .find(|entry| {
                entry.descriptor.type_code == type_code && entry.installation_id == installation_id
            })
            .ok_or_else(|| Error::Conflict("type_unavailable").into())
    }
    async fn project_instance(
        &self,
        actor: &ActorContext,
        record: NotificationInstanceRecord,
    ) -> Result<NotificationInstance> {
        let entry = self.types(actor).await?.into_iter().find(|entry| {
            entry.descriptor.type_code == record.type_code
                && entry.installation_id == record.installation_id
        });
        let reason = if record.status == NotificationInstanceStatus::Disabled {
            Some("instance_disabled")
        } else {
            entry
                .as_ref()
                .map_or(Some("type_unavailable"), |entry| entry.unavailable_reason)
        };
        let persisted = self
            .repository
            .notification_authorization_view(actor.current_workspace_id, record.id)
            .await?;
        let authorization = match persisted {
            Some(view) => Some(match &entry {
                Some(entry) => authorization::project_for_descriptor(view, &entry.descriptor),
                None => authorization::project_authorization(view),
            }),
            None => entry
                .filter(|entry| entry.descriptor.authorization.is_some())
                .map(|_| authorization::unauthorized()),
        };
        Ok(NotificationInstance {
            authorization,
            record,
            available: reason.is_none(),
            unavailable_reason: reason,
        })
    }
    pub async fn list_instances(&self, actor: &ActorContext) -> Result<Vec<NotificationInstance>> {
        self.query_instances(actor, NotificationInstanceListInput::default())
            .await
    }
    pub async fn query_instances(
        &self,
        actor: &ActorContext,
        input: NotificationInstanceListInput,
    ) -> Result<Vec<NotificationInstance>> {
        self.manage(actor, "notifications.instances.list").await?;
        let display_name = input
            .display_name
            .as_deref()
            .unwrap_or("")
            .trim()
            .to_lowercase();
        let mut output = Vec::new();
        for record in self
            .repository
            .list_notification_instances(actor.current_workspace_id)
            .await?
        {
            if !record.display_name.to_lowercase().contains(&display_name)
                || input
                    .type_code
                    .as_ref()
                    .is_some_and(|code| code != &record.type_code)
                || input.status.is_some_and(|status| status != record.status)
            {
                continue;
            }
            output.push(self.project_instance(actor, record).await?);
        }
        Ok(output)
    }
    pub async fn get_instance(
        &self,
        actor: &ActorContext,
        id: Uuid,
    ) -> Result<NotificationInstance> {
        self.manage(actor, "notifications.instances.get").await?;
        self.project_instance(actor, self.record(actor, id).await?)
            .await
    }
    async fn record(&self, actor: &ActorContext, id: Uuid) -> Result<NotificationInstanceRecord> {
        self.repository
            .get_notification_instance(actor.current_workspace_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("notification_instance").into())
    }
    pub async fn create_instance(
        &self,
        actor: &ActorContext,
        input: CreateNotificationInstance,
    ) -> Result<NotificationInstance> {
        self.manage(actor, "notifications.instances.create").await?;
        let entry = self
            .type_entry(actor, &input.type_code, input.installation_id)
            .await?;
        let name = required_name(&input.display_name)?;
        entry
            .descriptor
            .validate_config(&input.config_json)
            .map_err(|_| Error::InvalidInput("notification_config_schema"))?;
        let (public, secret, names) = split_config(&entry.descriptor, &input.config_json)?;
        let now = time::OffsetDateTime::now_utc();
        let record = NotificationInstanceRecord {
            id: Uuid::now_v7(),
            workspace_id: actor.current_workspace_id,
            type_code: input.type_code,
            installation_id: input.installation_id,
            display_name: name,
            status: input.status.unwrap_or(NotificationInstanceStatus::Enabled),
            config_json: public,
            configured_secret_fields: names,
            created_at: now,
            updated_at: now,
        };
        let record = self
            .repository
            .save_notification_instance(SaveNotificationInstanceInput {
                record,
                secret_config_json: secret,
                master_key: self.master_key.clone(),
                create: true,
                initial_authorization: entry
                    .descriptor
                    .authorization
                    .as_ref()
                    .map(|_| authorization::unauthorized()),
            })
            .await?;
        self.project_instance(actor, record).await
    }
    pub async fn patch_instance(
        &self,
        actor: &ActorContext,
        id: Uuid,
        input: PatchNotificationInstance,
    ) -> Result<NotificationInstance> {
        self.manage(actor, "notifications.instances.update").await?;
        let mut record = self.record(actor, id).await?;
        let mut secret = self
            .repository
            .resolve_notification_secret_config(actor.current_workspace_id, id, &self.master_key)
            .await?;
        if let Some(config) = input.config_json {
            let entry = self
                .type_entry(actor, &record.type_code, record.installation_id)
                .await?;
            let mut complete = record
                .config_json
                .as_object()
                .cloned()
                .ok_or(Error::Conflict("notification_config"))?;
            complete.extend(
                secret
                    .as_object()
                    .cloned()
                    .ok_or(Error::Conflict("notification_config"))?,
            );
            complete.extend(
                config
                    .as_object()
                    .cloned()
                    .ok_or(Error::InvalidInput("config_json"))?,
            );
            let complete = Value::Object(complete);
            entry
                .descriptor
                .validate_config(&complete)
                .map_err(|_| Error::InvalidInput("notification_config_schema"))?;
            let (public, new_secret, names) = split_config(&entry.descriptor, &complete)?;
            record.config_json = public;
            record.configured_secret_fields = names;
            secret = new_secret;
        }
        if let Some(name) = input.display_name {
            record.display_name = required_name(&name)?;
        }
        if let Some(status) = input.status {
            record.status = status;
        }
        let record = self
            .repository
            .save_notification_instance(SaveNotificationInstanceInput {
                record,
                secret_config_json: secret,
                master_key: self.master_key.clone(),
                create: false,
                initial_authorization: None,
            })
            .await?;
        self.project_instance(actor, record).await
    }
    pub async fn delete_instance(&self, actor: &ActorContext, id: Uuid) -> Result<()> {
        self.manage(actor, "notifications.instances.delete").await?;
        if !self
            .repository
            .delete_notification_instance(actor.current_workspace_id, id)
            .await?
        {
            return Err(Error::NotFound("notification_instance").into());
        }
        Ok(())
    }
    pub async fn logs(
        &self,
        actor: &ActorContext,
        input: crate::ports::NotificationLogListInput,
    ) -> Result<domain::NotificationSendLogPage> {
        self.manage(actor, "notifications.logs.list").await?;
        self.repository
            .list_notification_send_logs(actor.current_workspace_id, input)
            .await
    }
    pub async fn log(&self, actor: &ActorContext, id: Uuid) -> Result<domain::NotificationSendLog> {
        self.manage(actor, "notifications.logs.get").await?;
        self.repository
            .get_notification_send_log(actor.current_workspace_id, id)
            .await?
            .ok_or_else(|| Error::NotFound("notification_send_log").into())
    }
    pub async fn channels(
        &self,
        actor: &ActorContext,
        input: crate::ports::NotificationChannelListInput,
    ) -> Result<domain::NotificationChannelPage> {
        self.ensure_actor(actor).await?;
        self.repository
            .list_notification_channels(actor.current_workspace_id, actor.user_id, input)
            .await
    }
    pub async fn unread(&self, actor: &ActorContext, id: Uuid) -> Result<SystemNotificationRecord> {
        self.ensure_actor(actor).await?;
        let record = self
            .repository
            .mark_system_notification_unread(actor.current_workspace_id, actor.user_id, id)
            .await?
            .ok_or(Error::NotFound("notification_message"))?;
        self.publish(actor.current_workspace_id, &[actor.user_id]);
        Ok(record)
    }
    pub async fn read_channel(&self, actor: &ActorContext, instance_id: Uuid) -> Result<u64> {
        self.ensure_actor(actor).await?;
        let count = self
            .repository
            .mark_notification_channel_read(actor.current_workspace_id, actor.user_id, instance_id)
            .await?;
        self.publish(actor.current_workspace_id, &[actor.user_id]);
        Ok(count)
    }
    pub async fn messages(
        &self,
        actor: &ActorContext,
        input: NotificationMessageListInput,
    ) -> Result<NotificationMessagePage> {
        self.ensure_actor(actor).await?;
        self.repository
            .list_system_notifications(actor.current_workspace_id, actor.user_id, input)
            .await
    }
    pub async fn unread_count(&self, actor: &ActorContext) -> Result<i64> {
        self.ensure_actor(actor).await?;
        self.repository
            .system_notification_unread_count(actor.current_workspace_id, actor.user_id)
            .await
    }
    pub async fn read(&self, actor: &ActorContext, id: Uuid) -> Result<SystemNotificationRecord> {
        self.ensure_actor(actor).await?;
        let record = self
            .repository
            .mark_system_notification_read(actor.current_workspace_id, actor.user_id, id)
            .await?
            .ok_or(Error::NotFound("notification_message"))?;
        self.publish(actor.current_workspace_id, &[actor.user_id]);
        Ok(record)
    }
    pub async fn read_all(&self, actor: &ActorContext) -> Result<u64> {
        self.ensure_actor(actor).await?;
        let count = self
            .repository
            .mark_all_system_notifications_read(actor.current_workspace_id, actor.user_id)
            .await?;
        self.publish(actor.current_workspace_id, &[actor.user_id]);
        Ok(count)
    }
    fn publish(&self, workspace_id: Uuid, users: &[Uuid]) {
        if let Some(events) = &self.events {
            events.publish(workspace_id, users);
        }
    }
}

#[async_trait]
impl<R> NotificationSenderPort for NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader,
{
    async fn send(
        &self,
        actor: &ActorContext,
        source: NotificationSource,
        instance_id: Uuid,
        input: Value,
    ) -> Result<NotificationSendResult> {
        self.ensure_actor(actor).await?;
        let log_id = Uuid::now_v7();
        // Snapshot only public instance identity. Never persist raw invocation/provider data.
        let instance = self
            .repository
            .get_notification_instance(actor.current_workspace_id, instance_id)
            .await?;
        self.repository
            .create_notification_send_log(domain::NotificationSendLog {
                id: log_id,
                workspace_id: actor.current_workspace_id,
                actor_user_id: actor.user_id,
                instance_id,
                display_name: instance.as_ref().map(|r| r.display_name.clone()),
                type_code: instance.as_ref().map(|r| r.type_code.clone()),
                source,
                status: "pending".into(),
                failure_code: None,
                title: None,
                content: None,
                recipient_count: None,
                created_at: time::OffsetDateTime::now_utc(),
                completed_at: None,
            })
            .await?;
        let outcome = self
            .send_logged(actor, instance_id, instance, input, log_id)
            .await;
        if let Err(error) = &outcome {
            let terminal = error.downcast_ref::<Error>().and_then(|error| match error {
                Error::NotFound("notification_instance") => Some(("failure", "instance_missing")),
                Error::Conflict("instance_disabled") => Some(("failure", "instance_disabled")),
                Error::Conflict("authorization_required") => {
                    Some(("failure", "authorization_required"))
                }
                Error::Conflict("authorization_invalid") => {
                    Some(("failure", "authorization_invalid"))
                }
                Error::Conflict("subscription_required") => {
                    Some(("failure", "subscription_required"))
                }
                Error::Conflict("subscription_expired") => {
                    Some(("failure", "subscription_expired"))
                }
                Error::UpstreamUnavailable("subscription_unavailable") => {
                    Some(("failure", "subscription_unavailable"))
                }
                Error::Conflict("recipient_not_ready") => Some(("failure", "recipient_not_ready")),
                Error::UpstreamUnavailable("provider_rejected") => {
                    Some(("failure", "provider_rejected"))
                }
                Error::Conflict("type_unavailable") => Some(("failure", "type_unavailable")),
                Error::Conflict("plugin_unavailable") => Some(("failure", "plugin_unavailable")),
                Error::InvalidInput("notification_input_schema" | "notification_recipients") => {
                    Some(("failure", "input_invalid"))
                }
                Error::Conflict("notification_config")
                | Error::InvalidInput("notification_config_schema") => {
                    Some(("failure", "config_invalid"))
                }
                Error::UpstreamUnavailable("notification_provider_failed") => {
                    Some(("unknown", "provider_result_unknown"))
                }
                Error::UpstreamUnavailable("notification_provider_unsafe_output")
                | Error::Conflict("notification_output_schema") => {
                    Some(("unknown", "provider_output_invalid"))
                }
                _ => None,
            });
            if let Some((status, code)) = terminal {
                // Failure to finalize cannot erase a durable pending invocation.
                let _ = self
                    .repository
                    .complete_notification_send_log(
                        actor.current_workspace_id,
                        log_id,
                        status,
                        Some(code),
                    )
                    .await;
            }
        }
        outcome
    }
}
impl<R> NotificationService<R>
where
    R: NotificationRepository + PluginRepository + RoleConsolePolicyReader,
{
    async fn send_logged(
        &self,
        actor: &ActorContext,
        instance_id: Uuid,
        instance: Option<NotificationInstanceRecord>,
        input: Value,
        log_id: Uuid,
    ) -> Result<NotificationSendResult> {
        let instance = instance.ok_or(Error::NotFound("notification_instance"))?;
        if instance.status != NotificationInstanceStatus::Enabled {
            return Err(Error::Conflict("instance_disabled").into());
        }
        let entry = self
            .type_entry(actor, &instance.type_code, instance.installation_id)
            .await?;
        if !entry.available {
            return Err(Error::Conflict("plugin_unavailable").into());
        }
        entry
            .descriptor
            .validate_input(&input)
            .map_err(|_| Error::InvalidInput("notification_input_schema"))?;
        if entry.descriptor.authorization.is_some() {
            return self
                .send_authorizable(actor, &instance, &entry.descriptor, input, log_id)
                .await;
        }
        let result = if instance.type_code == "system" {
            #[derive(Deserialize)]
            struct SystemInput {
                user_ids: Vec<Uuid>,
                title: String,
                content: String,
            }
            let input: SystemInput = serde_json::from_value(input)
                .map_err(|_| Error::InvalidInput("notification_input_schema"))?;
            let users = input
                .user_ids
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let records = self
                .repository
                .commit_system_notifications(
                    actor.current_workspace_id,
                    instance_id,
                    &users,
                    log_id,
                    &input.title,
                    &input.content,
                )
                .await?;
            self.publish(actor.current_workspace_id, &users);
            json!({"notification_ids":records.iter().map(|record|record.id).collect::<Vec<_>>(),"recipient_count":records.len()})
        } else {
            let mut config = instance
                .config_json
                .as_object()
                .cloned()
                .ok_or(Error::Conflict("notification_config"))?;
            let secret = self
                .repository
                .resolve_notification_secret_config(
                    actor.current_workspace_id,
                    instance_id,
                    &self.master_key,
                )
                .await?;
            config.extend(
                secret
                    .as_object()
                    .cloned()
                    .ok_or(Error::Conflict("notification_config"))?,
            );
            let config = Value::Object(config);
            entry
                .descriptor
                .validate_config(&config)
                .map_err(|_| Error::InvalidInput("notification_config_schema"))?;
            let provider = self
                .provider
                .as_ref()
                .ok_or(Error::Conflict("plugin_unavailable"))?;
            let result = provider
                .send(
                    actor,
                    instance
                        .installation_id
                        .ok_or(Error::Conflict("type_unavailable"))?,
                    &entry.descriptor,
                    config,
                    input,
                )
                .await
                .map_err(|_| Error::UpstreamUnavailable("notification_provider_failed"))?;
            if secret.as_object().is_some_and(|fields| {
                fields
                    .values()
                    .filter_map(Value::as_str)
                    .filter(|secret| !secret.is_empty())
                    .any(|secret| contains_secret(&result, secret))
            }) {
                return Err(
                    Error::UpstreamUnavailable("notification_provider_unsafe_output").into(),
                );
            }
            result
        };
        entry
            .descriptor
            .validate_output(&result)
            .map_err(|_| Error::Conflict("notification_output_schema"))?;
        if instance.type_code != "system" {
            self.repository
                .complete_notification_send_log(actor.current_workspace_id, log_id, "success", None)
                .await?;
        }
        Ok(NotificationSendResult {
            instance_id,
            type_code: instance.type_code,
            result,
        })
    }
}

fn required_name(name: &str) -> Result<String> {
    if name.trim().is_empty() {
        Err(Error::InvalidInput("display_name").into())
    } else {
        Ok(name.trim().to_owned())
    }
}
fn split_config(
    descriptor: &NotificationTypeDescriptor,
    config: &Value,
) -> Result<(Value, Value, Vec<String>)> {
    let mut public = config
        .as_object()
        .cloned()
        .ok_or(Error::InvalidInput("config_json"))?;
    let mut secret = serde_json::Map::new();
    for field in descriptor.secret_config_fields() {
        if let Some(value) = public.remove(field) {
            secret.insert(field.to_owned(), value);
        }
    }
    let names = secret.keys().cloned().collect();
    Ok((Value::Object(public), Value::Object(secret), names))
}

pub fn system_notification_descriptor() -> NotificationTypeDescriptor {
    NotificationTypeDescriptor{
        i18n:Default::default(),
        authorization:None,
        type_code:"system".into(),display_name:"System notification".into(),description:"Deliver a message to workspace members' inboxes".into(),
        contribution_id:ContributionId::new("1flowbase.notification.system").expect("builtin contribution"),
        config_schema:json!({"type":"object","properties":{},"additionalProperties":false}),
        config_form:serde_json::from_value(json!({"schema_version":"1.0.0","fields":[]})).expect("builtin config form"),
        input_schema:json!({"type":"object","properties":{"user_ids":{"type":"array","minItems":1,"items":{"type":"string","format":"uuid"}},"title":{"type":"string","minLength":1},"content":{"type":"string","minLength":1}},"required":["user_ids","title","content"],"additionalProperties":false}),
        input_form:serde_json::from_value(json!({"schema_version":"1.0.0","fields":[{"key":"user_ids","label":"Recipients","type":"select","control":"multi-select","required":true},{"key":"title","label":"Title","type":"string","required":true},{"key":"content","label":"Content","type":"string","control":"textarea","required":true}]})).expect("builtin input form"),
        output_schema:json!({"type":"object","properties":{"notification_ids":{"type":"array","items":{"type":"string","format":"uuid"}},"recipient_count":{"type":"integer","minimum":1}},"required":["notification_ids","recipient_count"],"additionalProperties":false}),
    }
}

fn contains_secret(value: &Value, secret: &str) -> bool {
    match value {
        Value::String(value) => value.contains(secret),
        Value::Array(values) => values.iter().any(|value| contains_secret(value, secret)),
        Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| key.contains(secret) || contains_secret(value, secret)),
        _ => false,
    }
}
