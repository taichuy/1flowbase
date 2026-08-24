use super::{NetworkEgressPluginFamilyResponse, NetworkEgressPluginInstalledVersionResponse};

#[test]
fn issue_1867_catalog_target_install_status_is_version_specific() {
    let mut family = NetworkEgressPluginFamilyResponse {
        provider_code: "clash-proxy".to_string(),
        display_name: "Clash / Mihomo Proxy".to_string(),
        current_installation_id: "installation-025".to_string(),
        current_version: "0.2.5".to_string(),
        can_uninstall: true,
        installed_versions: vec![NetworkEgressPluginInstalledVersionResponse {
            installation_id: "installation-025".to_string(),
            plugin_version: "0.2.5".to_string(),
            is_current: true,
            can_uninstall: false,
        }],
    };

    assert!(family.contains_installed_version("0.2.5"));
    assert!(!family.contains_installed_version("0.2.8"));

    family
        .installed_versions
        .push(NetworkEgressPluginInstalledVersionResponse {
            installation_id: "installation-028".to_string(),
            plugin_version: "0.2.8".to_string(),
            is_current: false,
            can_uninstall: true,
        });
    assert!(family.contains_installed_version("0.2.8"));
}
