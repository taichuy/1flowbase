use access_control::{
    ConsoleLocaleCatalogContribution, ConsoleLocaleText, ConsoleOperationOwner,
    ConsoleOtherPolicyGroupDisplay, SettingsFeatureLifecycle, SettingsFeatureOwnerKind,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct CoreConsoleLocaleText {
    pub(super) reference: &'static str,
    pub(super) en_us: &'static str,
    pub(super) zh_hans: &'static str,
}

macro_rules! text {
    ($reference:expr, $en_us:expr, $zh_hans:expr $(,)?) => {
        CoreConsoleLocaleText {
            reference: $reference,
            en_us: $en_us,
            zh_hans: $zh_hans,
        }
    };
}

mod applications_and_data;
mod catalog;
mod identity_and_roles;
mod infrastructure_and_mcp;
mod plugins_and_models;

pub(crate) fn core_console_locale_catalog_contribution() -> ConsoleLocaleCatalogContribution {
    let texts = [
        applications_and_data::TEXTS,
        catalog::TEXTS,
        identity_and_roles::TEXTS,
        infrastructure_and_mcp::TEXTS,
        plugins_and_models::TEXTS,
    ]
    .into_iter()
    .flat_map(|texts| texts.iter())
    .map(|text| ConsoleLocaleText {
        reference: text.reference.to_string(),
        en_us: text.en_us.to_string(),
        zh_hans: text.zh_hans.to_string(),
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
        label_ref: format!("console.policy_groups.other.{group_id}.label"),
        description_ref: format!("console.policy_groups.other.{group_id}.description"),
    })
    .collect()
}
