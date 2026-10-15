//! Presentation-only package dictionaries. Business schemas and values are never overrides.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{invalid, ContractResult, NotificationTypeDescriptor};
use crate::PluginFormSchema;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationTypeLocale {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_form: Option<NotificationFormLocale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_form: Option<NotificationFormLocale>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authorization: Option<NotificationAuthorizationLocale>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationFormLocale {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, NotificationFieldLocale>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationFieldLocale {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAuthorizationLocale {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub actions: BTreeMap<String, NotificationAuthorizationActionLocale>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotificationAuthorizationActionLocale {
    pub label: String,
}

fn nonempty(values: &[Option<&str>]) -> ContractResult {
    if values.iter().flatten().any(|value| value.trim().is_empty()) {
        return Err(invalid("i18n presentation strings must be nonempty"));
    }
    Ok(())
}

fn validate_form(locale: &NotificationFormLocale, form: &PluginFormSchema) -> ContractResult {
    nonempty(&[locale.title.as_deref(), locale.description.as_deref()])?;
    for (key, field) in &locale.fields {
        if !form.fields.iter().any(|field| &field.key == key) {
            return Err(invalid("i18n field must reference a canonical form field"));
        }
        nonempty(&[
            field.label.as_deref(),
            field.description.as_deref(),
            field.placeholder.as_deref(),
        ])?;
    }
    Ok(())
}

pub(super) fn validate(descriptor: &NotificationTypeDescriptor) -> ContractResult {
    for (locale, translation) in &descriptor.i18n {
        if !matches!(locale.as_str(), "zh_Hans" | "en_US") {
            return Err(invalid("i18n locale must be zh_Hans or en_US"));
        }
        nonempty(&[
            translation.display_name.as_deref(),
            translation.description.as_deref(),
        ])?;
        if let Some(form) = &translation.config_form {
            validate_form(form, &descriptor.config_form)?;
        }
        if let Some(form) = &translation.input_form {
            validate_form(form, &descriptor.input_form)?;
        }
        if let Some(authorization) = &translation.authorization {
            for (code, action) in &authorization.actions {
                if !descriptor.authorization.as_ref().is_some_and(|canonical| {
                    canonical.actions.iter().any(|action| &action.code == code)
                }) {
                    return Err(invalid(
                        "i18n action must reference a canonical authorization action",
                    ));
                }
                nonempty(&[Some(action.label.as_str())])?;
            }
        }
    }
    Ok(())
}
