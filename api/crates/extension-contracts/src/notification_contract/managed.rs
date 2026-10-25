//! Managed communications operations inside Capability Execute's input_payload/result.
//! Host encrypts managed secrets independently of config and never projects patches to clients.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    identity, invalid, ContractResult, NotificationAttachmentDownloadOutcome,
    NotificationAttachmentReference, NotificationReceiveOutcome, NotificationTypeDescriptor,
    NOTIFICATION_MAX_ATTACHMENT_CHUNK_BYTES, NOTIFICATION_MAX_RECEIVE_MESSAGES,
    NOTIFICATION_MAX_RECEIVE_WAIT_SECONDS,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAuthorizationDescriptor {
    pub actions: Vec<NotificationAuthorizationAction>,
    pub managed_secret_keys: Vec<String>,
    /// Declares that an authorized account also has an independent delivery window.
    #[serde(default)]
    pub supports_delivery_window: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAuthorizationAction {
    pub code: String,
    pub label: String,
    pub kind: NotificationAuthorizationActionKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAuthorizationActionKind {
    QrCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationAuthorizationOperation {
    Begin { action: String },
    Poll,
    Cancel,
    Maintain,
}

impl<'de> Deserialize<'de> for NotificationAuthorizationOperation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Serde unit variants ignore extra fields even with deny_unknown_fields.
        // Empty struct wire variants retain the public enum while rejecting injection.
        #[derive(Deserialize)]
        #[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
        enum WireOperation {
            Begin { action: String },
            Poll {},
            Cancel {},
            Maintain {},
        }
        Ok(match WireOperation::deserialize(deserializer)? {
            WireOperation::Begin { action } => Self::Begin { action },
            WireOperation::Poll {} => Self::Poll,
            WireOperation::Cancel {} => Self::Cancel,
            WireOperation::Maintain {} => Self::Maintain,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationAuthorizationStatus {
    Unauthorized,
    Pending,
    Authorized,
    Invalid,
}

/// Provider-owned evidence and deadline; Host projects expiry without defining its duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationDeliveryWindowStatus {
    Unknown,
    WaitingForMessage,
    Active,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationDeliveryWindowView {
    pub status: NotificationDeliveryWindowStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_received_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
}

impl NotificationDeliveryWindowView {
    pub fn validate(&self) -> ContractResult {
        let has_pair = match (self.last_received_at_ms, self.expires_at_ms) {
            (None, None) => false,
            (Some(last), Some(expiry)) if last > 0 && expiry > last => true,
            _ => {
                return Err(invalid(
                    "delivery window requires positive ordered timestamp pair",
                ))
            }
        };
        match self.status {
            NotificationDeliveryWindowStatus::Active
            | NotificationDeliveryWindowStatus::Expired
                if !has_pair =>
            {
                Err(invalid(
                    "active or expired delivery window requires timestamps",
                ))
            }
            NotificationDeliveryWindowStatus::WaitingForMessage if has_pair => {
                Err(invalid("waiting delivery window cannot carry timestamps"))
            }
            _ => Ok(()),
        }
    }
}

/// Canonical client DTO. Enabled/disabled is an independent administrator setting.
/// No credential, recipient id, context token or provider error belongs in this view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAuthorizationView {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_window: Option<NotificationDeliveryWindowView>,
    pub status: NotificationAuthorizationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_action: Option<NotificationAuthorizationUserAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAuthorizationUserAction {
    pub kind: NotificationAuthorizationActionKind,
    /// Displayable QR URL only. Internal polling credentials belong in managed_secret_patch.
    pub qr_code_url: String,
    pub poll_interval_seconds: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Host-injected secrets are separate from user config and never client supplied.
/// Includes encrypted QR credentials, bot token, bound scanned user id and provider context. Cursor is a separate Host-owned field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationRuntimeInput {
    /// Public evidence injected by Host, separate from both config and managed secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivery_window: Option<NotificationDeliveryWindowView>,
    pub operation: NotificationOperation,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub managed_secrets: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationOperation {
    Authorize {
        operation: NotificationAuthorizationOperation,
    },
    Send {
        input_payload: Value,
    },
    Receive {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cursor: Option<Value>,
        max_messages: u32,
        wait_seconds: u32,
    },
    DownloadAttachment {
        attachment: NotificationAttachmentReference,
        offset: u64,
        max_bytes: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationFailureCode {
    AuthorizationRequired,
    AuthorizationInvalid,
    RecipientNotReady,
    SubscriptionRequired,
    SubscriptionExpired,
    SubscriptionUnavailable,
    ProviderRejected,
    ProviderResultUnknown,
}

/// Unknown is a separate terminal delivery state, never a confirmed failure or success.
/// Only a provider-confirmed success may return a schema-checked result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationSendOutcome {
    Succeeded {
        result: Value,
    },
    Failed {
        failure_code: NotificationFailureCode,
    },
    Unknown {
        failure_code: NotificationFailureCode,
    },
}

impl NotificationSendOutcome {
    pub fn unknown() -> Self {
        Self::Unknown {
            failure_code: NotificationFailureCode::ProviderResultUnknown,
        }
    }

    pub fn validate(&self, descriptor: &NotificationTypeDescriptor) -> ContractResult {
        match self {
            Self::Succeeded { result } => descriptor.validate_output(result),
            Self::Failed {
                failure_code: NotificationFailureCode::ProviderResultUnknown,
            } => Err(invalid("ambiguous send outcome must be unknown")),
            Self::Unknown { failure_code }
                if *failure_code != NotificationFailureCode::ProviderResultUnknown =>
            {
                Err(invalid("unknown outcome requires provider_result_unknown"))
            }
            _ => Ok(()),
        }
    }
}

/// Host-only result envelope. Persist the patch encrypted before returning only the public view.
/// Null patch values delete keys; omitted keys stay unchanged. Never log this envelope.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum NotificationRuntimeResult {
    Receive {
        outcome: NotificationReceiveOutcome,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authorization: Option<NotificationAuthorizationView>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        managed_secret_patch: BTreeMap<String, Value>,
    },
    DownloadAttachment {
        outcome: NotificationAttachmentDownloadOutcome,
    },
    Authorization {
        authorization: NotificationAuthorizationView,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        managed_secret_patch: BTreeMap<String, Value>,
    },
    Send {
        outcome: NotificationSendOutcome,
        /// Omitted means no change; recipient_not_ready must not revoke account authorization.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        authorization: Option<NotificationAuthorizationView>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        managed_secret_patch: BTreeMap<String, Value>,
    },
}

impl NotificationAuthorizationDescriptor {
    pub fn validate(&self) -> ContractResult {
        let mut actions = BTreeSet::new();
        let mut keys = BTreeSet::new();
        if self.actions.is_empty()
            || self.actions.iter().any(|action| {
                !identity(&action.code)
                    || action.code.contains('/')
                    || action.label.trim().is_empty()
                    || !actions.insert(&action.code)
            })
        {
            return Err(invalid("authorization requires unique labeled actions"));
        }
        if self.managed_secret_keys.is_empty()
            || self
                .managed_secret_keys
                .iter()
                .any(|key| !identity(key) || key.contains('/') || !keys.insert(key))
        {
            return Err(invalid("authorization requires unique managed secret keys"));
        }
        Ok(())
    }

    pub fn validate_secret_keys(&self, secrets: &BTreeMap<String, Value>) -> ContractResult {
        if secrets
            .keys()
            .any(|key| !self.managed_secret_keys.contains(key))
        {
            return Err(invalid("undeclared managed secret key"));
        }
        Ok(())
    }
}

impl NotificationAuthorizationView {
    pub fn validate(&self) -> ContractResult {
        if let Some(window) = &self.delivery_window {
            window.validate()?;
        }
        if (self.status == NotificationAuthorizationStatus::Pending) != self.user_action.is_some() {
            return Err(invalid("only pending authorization requires a user action"));
        }
        if let Some(action) = &self.user_action {
            let url = url::Url::parse(&action.qr_code_url)
                .map_err(|_| invalid("QR action requires a displayable URL"))?;
            if url.scheme() != "https"
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
                || action.poll_interval_seconds == 0
            {
                return Err(invalid(
                    "QR action requires a displayable URL and positive polling interval",
                ));
            }
        }
        Ok(())
    }
}

impl NotificationRuntimeInput {
    pub fn validate(&self, descriptor: &NotificationTypeDescriptor) -> ContractResult {
        descriptor.validate()?;
        if let Some(authorization) = &descriptor.authorization {
            authorization.validate_secret_keys(&self.managed_secrets)?;
        } else if !self.managed_secrets.is_empty() {
            return Err(invalid("managed secrets require declared authorization"));
        }
        if let Some(window) = &self.delivery_window {
            window.validate()?;
        }
        match &self.operation {
            NotificationOperation::Authorize { operation } => {
                let authorization = descriptor
                    .authorization
                    .as_ref()
                    .ok_or_else(|| invalid("authorization capability is not declared"))?;
                if let NotificationAuthorizationOperation::Begin { action } = operation {
                    if !authorization
                        .actions
                        .iter()
                        .any(|declared| &declared.code == action)
                    {
                        return Err(invalid("undeclared notification authorization action"));
                    }
                }
                Ok(())
            }
            NotificationOperation::Receive {
                cursor,
                max_messages,
                wait_seconds,
            } => {
                if descriptor.receive.is_none()
                    || *max_messages == 0
                    || *max_messages > NOTIFICATION_MAX_RECEIVE_MESSAGES
                    || *wait_seconds > NOTIFICATION_MAX_RECEIVE_WAIT_SECONDS
                    || cursor.as_ref().is_some_and(|value| {
                        serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > 16_384)
                    })
                {
                    return Err(invalid("receive capability or polling budget is invalid"));
                }
                Ok(())
            }
            NotificationOperation::DownloadAttachment {
                attachment,
                offset,
                max_bytes,
            } => {
                if descriptor.receive.is_none()
                    || *max_bytes == 0
                    || *max_bytes > NOTIFICATION_MAX_ATTACHMENT_CHUNK_BYTES
                    || offset.checked_add(*max_bytes as u64).is_none()
                {
                    return Err(invalid(
                        "attachment download capability or budget is invalid",
                    ));
                }
                attachment.validate()
            }
            NotificationOperation::Send { input_payload } => {
                descriptor.validate_input(input_payload)
            }
        }
    }
}

impl NotificationRuntimeResult {
    pub fn validate_for(
        &self,
        input: &NotificationRuntimeInput,
        descriptor: &NotificationTypeDescriptor,
    ) -> ContractResult {
        input.validate(descriptor)?;
        let validate_patch = |patch: &BTreeMap<String, Value>| -> ContractResult {
            if descriptor.receive.is_some() && patch.contains_key("cursor") {
                return Err(invalid(
                    "cursor may only be returned as a durable inbox candidate",
                ));
            }
            match &descriptor.authorization {
                Some(declared) => declared.validate_secret_keys(patch),
                None if patch.is_empty() => Ok(()),
                None => Err(invalid(
                    "managed secret patch requires declared authorization",
                )),
            }
        };
        match (self, &input.operation) {
            (
                Self::Receive {
                    outcome,
                    authorization,
                    managed_secret_patch,
                },
                NotificationOperation::Receive { max_messages, .. },
            ) => {
                if let NotificationReceiveOutcome::Succeeded { batch } = outcome {
                    batch.validate(*max_messages)?;
                }
                if let Some(view) = authorization {
                    view.validate()?;
                }
                validate_patch(managed_secret_patch)
            }
            (
                Self::DownloadAttachment { outcome },
                NotificationOperation::DownloadAttachment {
                    offset, max_bytes, ..
                },
            ) => outcome.validate(*offset, *max_bytes),
            (
                Self::Authorization {
                    authorization,
                    managed_secret_patch,
                },
                NotificationOperation::Authorize { .. },
            ) => {
                authorization.validate()?;
                validate_patch(managed_secret_patch)
            }
            (
                Self::Send {
                    outcome,
                    authorization,
                    managed_secret_patch,
                },
                NotificationOperation::Send { .. },
            ) => {
                outcome.validate(descriptor)?;
                if let Some(view) = authorization {
                    view.validate()?;
                    let expected = match outcome {
                        NotificationSendOutcome::Succeeded { .. } => {
                            Some(NotificationAuthorizationStatus::Authorized)
                        }
                        NotificationSendOutcome::Failed {
                            failure_code: NotificationFailureCode::AuthorizationRequired,
                        } => Some(NotificationAuthorizationStatus::Unauthorized),
                        NotificationSendOutcome::Failed {
                            failure_code: NotificationFailureCode::AuthorizationInvalid,
                        } => Some(NotificationAuthorizationStatus::Invalid),
                        NotificationSendOutcome::Failed {
                            failure_code:
                                NotificationFailureCode::RecipientNotReady
                                | NotificationFailureCode::SubscriptionRequired
                                | NotificationFailureCode::SubscriptionExpired
                                | NotificationFailureCode::SubscriptionUnavailable,
                        } => Some(NotificationAuthorizationStatus::Authorized),
                        _ => None,
                    };
                    if expected.is_some_and(|status| view.status != status) {
                        return Err(invalid("send outcome conflicts with authorization status"));
                    }
                } else if matches!(
                    outcome,
                    NotificationSendOutcome::Failed {
                        failure_code: NotificationFailureCode::AuthorizationRequired
                            | NotificationFailureCode::AuthorizationInvalid
                    }
                ) {
                    return Err(invalid(
                        "authorization failure requires an updated public status",
                    ));
                }
                validate_patch(managed_secret_patch)
            }
            _ => Err(invalid(
                "notification response does not match requested operation",
            )),
        }
    }
}
