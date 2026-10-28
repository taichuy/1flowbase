use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationInstanceStatus {
    Enabled,
    Disabled,
}
impl NotificationInstanceStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationInstanceRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub type_code: String,
    pub installation_id: Option<Uuid>,
    pub display_name: String,
    pub status: NotificationInstanceStatus,
    pub config_json: Value,
    pub configured_secret_fields: Vec<String>,
    #[serde(with = "timestamp")]
    pub created_at: OffsetDateTime,
    #[serde(with = "timestamp")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemNotificationRecord {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub instance_id: Uuid,
    pub recipient_user_id: Uuid,
    pub title: String,
    pub content: String,
    #[serde(with = "optional_timestamp")]
    pub read_at: Option<OffsetDateTime>,
    #[serde(with = "timestamp")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationMessagePage {
    pub items: Vec<SystemNotificationRecord>,
    pub next_cursor: Option<String>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSendResult {
    pub instance_id: Uuid,
    pub type_code: String,
    pub result: Value,
}

#[derive(Debug, Clone)]
pub struct NotificationRecipientOption {
    pub id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationSource {
    Api,
    Workflow,
}
impl NotificationSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Workflow => "workflow",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSendLog {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub actor_user_id: Uuid,
    pub instance_id: Uuid,
    pub display_name: Option<String>,
    pub type_code: Option<String>,
    pub source: NotificationSource,
    pub status: String,
    pub failure_code: Option<String>,
    pub title: Option<String>,
    pub content: Option<String>,
    pub recipient_count: Option<i64>,
    pub extra: serde_json::Value,
    #[serde(with = "timestamp")]
    pub created_at: OffsetDateTime,
    #[serde(with = "optional_timestamp")]
    pub completed_at: Option<OffsetDateTime>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationSendLogPage {
    pub items: Vec<NotificationSendLog>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationReadStatus {
    #[default]
    All,
    Unread,
    Read,
}
impl NotificationReadStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Unread => "unread",
            Self::Read => "read",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannel {
    pub instance_id: Uuid,
    pub display_name: String,
    pub type_code: String,
    pub deleted: bool,
    pub latest_title: String,
    pub latest_content: String,
    #[serde(with = "timestamp")]
    pub latest_created_at: OffsetDateTime,
    pub unread_count: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationChannelPage {
    pub items: Vec<NotificationChannel>,
    pub next_cursor: Option<String>,
}
mod timestamp {
    use super::*;
    use serde::{de::Error as _, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        value: &OffsetDateTime,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(
            &value
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(serde::ser::Error::custom)?,
        )
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<OffsetDateTime, D::Error> {
        OffsetDateTime::parse(
            &String::deserialize(deserializer)?,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(D::Error::custom)
    }
}
mod optional_timestamp {
    use super::*;
    use serde::{de::Error as _, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(
        value: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_some(
                &value
                    .format(&time::format_description::well_known::Rfc3339)
                    .map_err(serde::ser::Error::custom)?,
            ),
            None => serializer.serialize_none(),
        }
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<OffsetDateTime>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|value| {
                OffsetDateTime::parse(&value, &time::format_description::well_known::Rfc3339)
                    .map_err(D::Error::custom)
            })
            .transpose()
    }
}

// Dedicated service endpoints own access to these records. Catalog visibility does not grant
// generic record reads, which could otherwise bypass recipient and credential projections.
fn notification_model_contract(
    code: &'static str,
    fields: &'static [&'static str],
) -> crate::BuiltinDataModelContract {
    crate::BuiltinDataModelContract {
        code,
        physical_table_name: code,
        kind: crate::BuiltinDataModelKind::Core,
        system_field_codes: fields,
        capabilities: crate::DataModelCapabilities {
            can_delete: false,
            can_add_user_field: false,
            can_update_lifecycle_status: false,
            record: crate::DataModelRecordCapabilities {
                can_list: false,
                can_get: false,
                can_create: false,
                can_update: false,
                can_delete: false,
            },
        },
    }
}
pub(crate) fn notification_instance_model_contract() -> crate::BuiltinDataModelContract {
    notification_model_contract(
        "notification_instances",
        &[
            "id",
            "workspace_id",
            "type_code",
            "installation_id",
            "display_name",
            "status",
            "config_json",
            "configured_secret_fields",
            "created_at",
            "updated_at",
        ],
    )
}
pub(crate) fn system_notification_model_contract() -> crate::BuiltinDataModelContract {
    notification_model_contract(
        "system_notifications",
        &[
            "id",
            "workspace_id",
            "instance_id",
            "recipient_user_id",
            "title",
            "content",
            "read_at",
            "created_at",
        ],
    )
}

pub(crate) fn notification_send_log_model_contract() -> crate::BuiltinDataModelContract {
    notification_model_contract(
        "notification_send_logs",
        &[
            "id",
            "workspace_id",
            "actor_user_id",
            "instance_id",
            "display_name",
            "type_code",
            "source",
            "status",
            "failure_code",
            "title",
            "content",
            "recipient_count",
            "created_at",
            "completed_at",
            "extra",
        ],
    )
}

mod communications;
pub use communications::*;
mod communication_sessions;
pub use communication_sessions::*;

mod communication_commands;
pub use communication_commands::*;
