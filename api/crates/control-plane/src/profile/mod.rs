use anyhow::Result;
use domain::UserRecord;
use runtime_profile::{normalize_supported_locale, SUPPORTED_LOCALES};
use uuid::Uuid;

use crate::{
    errors::ControlPlaneError,
    ports::{AuthRepository, UpdateProfileInput, UpdateUserMetaInput},
};

/// Feature-owned preferences are written by the validated assistant settings use case.
pub const ASSISTANT_META_KEY: &str = "embedded_assistant";

pub struct UpdateMeCommand {
    pub actor_user_id: Uuid,
    pub name: String,
    pub nickname: String,
    pub email: String,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub introduction: String,
    pub preferred_locale: Option<String>,
}

pub struct UpdateMeMetaCommand {
    pub actor_user_id: Uuid,
    pub meta_patch: serde_json::Value,
}

pub struct ProfileService<R> {
    repository: R,
}

impl<R> ProfileService<R>
where
    R: AuthRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn update_me(&self, command: UpdateMeCommand) -> Result<UserRecord> {
        let preferred_locale = normalize_locale(command.preferred_locale)?;
        self.repository
            .update_profile(&UpdateProfileInput {
                actor_user_id: command.actor_user_id,
                user_id: command.actor_user_id,
                name: command.name,
                nickname: command.nickname,
                email: command.email,
                phone: command.phone,
                avatar_url: command.avatar_url,
                introduction: command.introduction,
                preferred_locale,
            })
            .await
    }

    pub async fn update_me_preferences(&self, command: UpdateMeMetaCommand) -> Result<UserRecord> {
        if command.meta_patch.get(ASSISTANT_META_KEY).is_some() {
            return Err(ControlPlaneError::InvalidInput(
                "assistant_preferences_require_settings_interface",
            )
            .into());
        }
        self.update_me_meta(command).await
    }

    pub async fn update_me_meta(&self, command: UpdateMeMetaCommand) -> Result<UserRecord> {
        if !command.meta_patch.is_object() {
            return Err(ControlPlaneError::InvalidInput("meta").into());
        }

        let current_user = self
            .repository
            .find_user_by_id(command.actor_user_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("user"))?;
        let mut next_meta = current_user.meta;
        merge_json_patch(&mut next_meta, command.meta_patch);

        self.repository
            .update_user_meta(&UpdateUserMetaInput {
                actor_user_id: command.actor_user_id,
                user_id: command.actor_user_id,
                meta: next_meta,
            })
            .await
    }
}

fn merge_json_patch(target: &mut serde_json::Value, patch: serde_json::Value) {
    let patch_object = match patch {
        serde_json::Value::Object(patch_object) => patch_object,
        value => {
            *target = value;
            return;
        }
    };

    if !target.is_object() {
        *target = serde_json::json!({});
    }

    let target_object = target
        .as_object_mut()
        .expect("target was normalized to an object");
    for (key, value) in patch_object {
        if value.is_null() {
            target_object.remove(&key);
            continue;
        }

        match target_object.get_mut(&key) {
            Some(existing) if existing.is_object() && value.is_object() => {
                merge_json_patch(existing, value);
            }
            _ => {
                target_object.insert(key, value);
            }
        }
    }
}

fn normalize_locale(preferred_locale: Option<String>) -> Result<Option<String>> {
    let supported_locales = SUPPORTED_LOCALES
        .iter()
        .map(|value| value.to_string())
        .collect::<Vec<_>>();
    match preferred_locale {
        Some(locale) => normalize_supported_locale(&locale, &supported_locales)
            .map(Some)
            .ok_or(ControlPlaneError::InvalidInput("unsupported_locale").into()),
        None => Ok(None),
    }
}
