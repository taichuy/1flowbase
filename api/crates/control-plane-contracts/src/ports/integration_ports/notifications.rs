use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

pub struct SaveNotificationInstanceInput {
    pub record: domain::NotificationInstanceRecord,
    pub secret_config_json: Value,
    pub master_key: String,
    pub create: bool,
    /// Only used on create; ordinary config/name/status updates never replace authorization.
    pub initial_authorization: Option<extension_contracts::NotificationAuthorizationView>,
}
// Credentials deliberately have no Debug implementation.

pub struct NotificationAuthorizationSnapshot {
    pub authorization: Option<extension_contracts::NotificationAuthorizationView>,
    pub managed_secrets: std::collections::BTreeMap<String, Value>,
    pub revision: i64,
}

pub struct SaveNotificationAuthorizationInput {
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub expected_revision: i64,
    pub authorization: extension_contracts::NotificationAuthorizationView,
    pub managed_secrets: std::collections::BTreeMap<String, Value>,
    pub master_key: String,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationMessageListInput {
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
    pub unread_only: bool,
    pub instance_id: Option<Uuid>,
    pub read_status: Option<domain::NotificationReadStatus>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationLogListInput {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub instance_id: Option<Uuid>,
    pub type_code: Option<String>,
    pub status: Option<String>,
    pub source: Option<domain::NotificationSource>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationChannelListInput {
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
    pub read_status: Option<domain::NotificationReadStatus>,
}
#[async_trait]
pub trait NotificationRepository: Send + Sync {
    async fn notification_authorization_view(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<extension_contracts::NotificationAuthorizationView>>;
    async fn resolve_notification_authorization(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<NotificationAuthorizationSnapshot>;
    /// Compare-and-swap the complete encrypted state; None means deleted or superseded.
    /// No provider I/O or transaction lock is held by this operation.
    async fn save_notification_authorization(
        &self,
        input: SaveNotificationAuthorizationInput,
    ) -> anyhow::Result<Option<i64>>;
    async fn create_notification_send_log(
        &self,
        log: domain::NotificationSendLog,
    ) -> anyhow::Result<()>;
    async fn complete_notification_send_log(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        status: &str,
        failure_code: Option<&str>,
    ) -> anyhow::Result<()>;
    async fn list_notification_send_logs(
        &self,
        workspace_id: Uuid,
        input: NotificationLogListInput,
    ) -> anyhow::Result<domain::NotificationSendLogPage>;
    async fn get_notification_send_log(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<domain::NotificationSendLog>>;
    async fn list_notification_channels(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        input: NotificationChannelListInput,
    ) -> anyhow::Result<domain::NotificationChannelPage>;
    async fn mark_system_notification_unread(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<domain::SystemNotificationRecord>>;
    async fn mark_notification_channel_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        instance_id: Uuid,
    ) -> anyhow::Result<u64>;
    async fn bootstrap_system_notification_instances(&self) -> anyhow::Result<()>;
    async fn list_notification_instances(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::NotificationInstanceRecord>>;
    async fn get_notification_instance(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<domain::NotificationInstanceRecord>>;
    async fn save_notification_instance(
        &self,
        input: SaveNotificationInstanceInput,
    ) -> anyhow::Result<domain::NotificationInstanceRecord>;
    async fn delete_notification_instance(
        &self,
        workspace_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<bool>;
    async fn resolve_notification_secret_config(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        master_key: &str,
    ) -> anyhow::Result<Value>;
    async fn notification_recipient_options(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::NotificationRecipientOption>>;
    /// The adapter commits one row per recipient in one transaction, checking membership and
    /// locking the instance so disable/delete cannot race a successful station send.
    async fn commit_system_notifications(
        &self,
        workspace_id: Uuid,
        instance_id: Uuid,
        recipients: &[Uuid],
        send_log_id: Uuid,
        title: &str,
        content: &str,
    ) -> anyhow::Result<Vec<domain::SystemNotificationRecord>>;
    async fn list_system_notifications(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        input: NotificationMessageListInput,
    ) -> anyhow::Result<domain::NotificationMessagePage>;
    async fn system_notification_unread_count(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<i64>;
    async fn mark_system_notification_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        id: Uuid,
    ) -> anyhow::Result<Option<domain::SystemNotificationRecord>>;
    async fn mark_all_system_notifications_read(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
    ) -> anyhow::Result<u64>;
}
