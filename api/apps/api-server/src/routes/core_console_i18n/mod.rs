use access_control::{
    ConsoleLocaleCatalogContribution, ConsoleLocaleText, ConsoleOperationOwner,
    ConsoleOtherPolicyGroupDisplay, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
};
use domain::CatalogLocale;

use crate::{app_state::ApiState, error_response::ApiError};

#[derive(Debug, Clone, Copy)]
pub(super) struct CoreConsoleDisplayText {
    pub(super) locale_catalog_ref: &'static str,
    pub(super) key: &'static str,
}

impl CoreConsoleDisplayText {
    pub(super) const fn new(key: &'static str) -> Self {
        Self {
            locale_catalog_ref: key,
            key,
        }
    }

    pub(super) const fn referenced(locale_catalog_ref: &'static str, key: &'static str) -> Self {
        Self {
            locale_catalog_ref,
            key,
        }
    }
}

mod catalog;

pub(crate) fn core_console_locale_catalog_contribution() -> ConsoleLocaleCatalogContribution {
    let texts = [catalog::TEXTS]
        .into_iter()
        .flat_map(|texts| texts.iter())
        .map(|text| ConsoleLocaleText {
            reference: text.locale_catalog_ref.to_string(),
            en_us: text.key.to_string(),
            zh_hans: text.key.to_string(),
        })
        .collect();

    ConsoleLocaleCatalogContribution {
        owner: ConsoleOperationOwner {
            kind: SettingsFeatureOwnerKind::Core,
            owner_id: "boot-core".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        },
        lifecycle: SettingsFeatureLifecycle::Active,
        texts,
        policy_groups: other_policy_group_displays(),
    }
}

fn other_policy_group_displays() -> Vec<ConsoleOtherPolicyGroupDisplay> {
    [
        "core.authenticated",
        "other.agent-flow",
        "other.data-sources",
        "other.frontend-blocks",
        "other.js-dependencies",
        "other.model-providers",
        "other.node-contributions",
        "other.plugins",
        "other.workspace",
    ]
    .into_iter()
    .map(|group_id| ConsoleOtherPolicyGroupDisplay {
        group_id: group_id.to_string(),
        label_ref: other_policy_group_keys(group_id).0.to_string(),
        description_ref: other_policy_group_keys(group_id).1.to_string(),
    })
    .collect()
}

fn other_policy_group_keys(group_id: &str) -> (&'static str, &'static str) {
    match group_id {
        "core.authenticated" => (
            "Signed-in console",
            "Console routes available to every signed-in user",
        ),
        "other.agent-flow" => (
            "Agent Flow",
            "Registered Agent Flow operations outside system settings",
        ),
        "other.data-sources" => (
            "Data source utilities",
            "Registered data source operations outside system settings",
        ),
        "other.frontend-blocks" => (
            "Frontend blocks",
            "Registered frontend block catalog operations",
        ),
        "other.js-dependencies" => (
            "JavaScript dependencies",
            "Registered JavaScript dependency operations",
        ),
        "other.model-providers" => (
            "Model provider utilities",
            "Registered model provider operations outside system settings",
        ),
        "other.node-contributions" => (
            "Node contributions",
            "Registered node contribution catalog operations",
        ),
        "other.plugins" => (
            "Plugins",
            "Registered plugin catalog and lifecycle operations",
        ),
        "other.workspace" => (
            "Current workspace",
            "Registered operations for the current workspace",
        ),
        _ => unreachable!("compiled Core policy group id must be known"),
    }
}

#[cfg(test)]
pub(crate) fn core_console_display_inventory() -> Vec<&'static str> {
    catalog::TEXTS.iter().map(|text| text.key).collect()
}

#[cfg(test)]
pub(crate) fn core_console_static_reference_inventory() -> Vec<&'static str> {
    catalog::TEXTS
        .iter()
        .filter(|text| text.locale_catalog_ref != text.key)
        .map(|text| text.locale_catalog_ref)
        .collect()
}

fn dynamic_display_key(key: &str) -> Option<&'static str> {
    catalog::TEXTS
        .iter()
        .find(|text| text.key == key)
        .map(|text| text.key)
}

pub(super) async fn resolve_core_console_display(
    state: &ApiState,
    locale: &CatalogLocale,
    key: &str,
) -> Result<String, ApiError> {
    let Some(resolved_key) = dynamic_display_key(key) else {
        return Ok(key.to_string());
    };
    crate::app_state::resolve_request_text(state, locale, resolved_key).await
}

#[cfg(test)]
mod tests {
    use super::dynamic_display_key;

    #[test]
    fn dynamic_display_lookup_accepts_only_english_keys() {
        assert_eq!(
            dynamic_display_key("Language catalog"),
            Some("Language catalog")
        );
        assert_eq!(dynamic_display_key("auto.translation_catalog_title"), None);
        assert_eq!(
            dynamic_display_key("console.policy_groups.settings.system.docs.description"),
            None
        );
    }
}
