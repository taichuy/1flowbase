use plugin_framework::{
    manifest_v1::PluginExecutionMode, parse_legacy_installed_plugin_manifest,
    parse_plugin_manifest, LegacyInstalledManifestEligibility, PluginConsumptionKind,
};

mod js_dependency_and_rejections;
mod node_contribution_and_slots;
mod package_and_frontend;
