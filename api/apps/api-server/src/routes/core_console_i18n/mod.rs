use access_control::{
    ConsoleLocaleCatalogContribution, ConsoleLocaleText, ConsoleOperationOwner,
    ConsoleOtherPolicyGroupDisplay, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
};
use domain::CatalogLocale;

use crate::{app_state::ApiState, error_response::ApiError};

#[derive(Debug, Clone, Copy)]
pub(super) struct CoreConsoleDisplayText {
    pub(super) reference: &'static str,
    pub(super) msgid: &'static str,
}

impl CoreConsoleDisplayText {
    pub(super) const fn new(msgid: &'static str) -> Self {
        Self {
            reference: msgid,
            msgid,
        }
    }

    pub(super) const fn referenced(reference: &'static str, msgid: &'static str) -> Self {
        Self { reference, msgid }
    }
}

mod catalog;

pub(crate) fn core_console_locale_catalog_contribution() -> ConsoleLocaleCatalogContribution {
    let texts = [catalog::TEXTS]
        .into_iter()
        .flat_map(|texts| texts.iter())
        .map(|text| ConsoleLocaleText {
            reference: text.reference.to_string(),
            en_us: text.msgid.to_string(),
            zh_hans: text.msgid.to_string(),
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
        label_ref: other_policy_group_msgids(group_id).0.to_string(),
        description_ref: other_policy_group_msgids(group_id).1.to_string(),
    })
    .collect()
}

fn other_policy_group_msgids(group_id: &str) -> (&'static str, &'static str) {
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
pub(crate) fn core_console_display_inventory() -> Vec<(&'static str, &'static str)> {
    catalog::TEXTS
        .iter()
        .map(|text| (text.module, text.msgid))
        .collect()
}

pub(super) async fn resolve_core_console_display(
    state: &ApiState,
    locale: &CatalogLocale,
    reference: &str,
) -> Result<String, ApiError> {
    let Some(text) = catalog::TEXTS
        .iter()
        .find(|text| text.reference == reference || text.msgid == reference)
    else {
        return Ok(reference.to_string());
    };
    crate::app_state::resolve_request_text(state, locale, text.msgid).await
}
