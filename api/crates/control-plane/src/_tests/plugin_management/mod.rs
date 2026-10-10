mod catalog;
mod extension_installation;
mod family;
mod install;
mod managed_service_install;
mod package_router;
pub(super) mod support;

#[test]
fn native_http_recovery_release_excludes_pre_contract_hosts() {
    use crate::plugin_management::{
        official_plugin_host_compatibility, validate_plugin_compatibility_requirement,
        PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM,
    };

    let minimum = "0.5.4";
    assert_eq!(
        official_plugin_host_compatibility(minimum, "0.5.3").status,
        PLUGIN_HOST_COMPATIBILITY_BELOW_MINIMUM
    );
    let rejection = validate_plugin_compatibility_requirement(minimum, "0.5.3", None)
        .expect_err("pre-contract host must not install without an explicit override");
    assert!(rejection
        .to_string()
        .contains("plugin_host_version_below_minimum"));
    assert_eq!(
        official_plugin_host_compatibility(minimum, "0.5.4").status,
        "compatible"
    );
    assert!(
        validate_plugin_compatibility_requirement(minimum, "0.5.4", None)
            .expect("paired host meets recovery release minimum")
            .is_none()
    );
}
