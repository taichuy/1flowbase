use crate::{
    validate_ui_code_template, validate_ui_component_record_fields, UiComponentRecordOrigin,
    UiComponentRecordUpstream, UiManagementInvariantError,
};

#[test]
fn frontend_registration_targets_require_independent_block_opt_in() {
    let mut entry: crate::FrontendBlockCatalogEntry = serde_json::from_value(serde_json::json!({
        "installation_id": uuid::Uuid::nil(), "provider_code": "calendar", "plugin_id": "calendar",
        "plugin_version": "1.0.0", "contribution_code": "Calendar", "title": "Calendar",
        "runtime": "native_react", "entry": "default", "ui_registration": "component",
        "code_modules": [], "context_contract": {"primitives": [], "input_schema": {}},
        "permissions": {"network": "none", "storage": "none", "secrets": "none"},
        "ui_capabilities": []
    }))
    .unwrap();
    assert!(!entry.block_registration);
    assert_eq!(entry.registration_targets(), ["component"]);
    entry.ui_registration = "code_template".into();
    assert_eq!(entry.registration_targets(), ["code_template"]);
    entry.block_registration = true;
    assert_eq!(entry.registration_targets(), ["block", "code_template"]);
    entry.ui_registration = "component".into();
    assert_eq!(entry.registration_targets(), ["block", "component"]);
}

#[test]
fn ac_002_ui_code_template_rejects_empty_and_oversized_source() {
    assert_eq!(
        validate_ui_code_template("Default", " "),
        Err(UiManagementInvariantError::EmptyTemplateSource)
    );
    assert_eq!(
        validate_ui_code_template("Default", &"x".repeat(262_145)),
        Err(UiManagementInvariantError::TemplateSourceTooLarge)
    );
}

#[test]
fn wp_d2_component_record_validates_shape_without_interpreting_code() {
    let upstream = UiComponentRecordUpstream {
        identity: "@example/ui".into(),
        version: "2.4.0".into(),
    };
    assert_eq!(
        validate_ui_component_record_fields(
            "Example.Button",
            "Button",
            "A button",
            "import { Button } from '@example/ui';",
            "<Button>{value}</Button>",
            UiComponentRecordOrigin::Custom,
            "local",
            "controls",
            &upstream,
            "1.0.0",
            &["action".into()],
        ),
        Err(UiManagementInvariantError::InvalidComponentCode)
    );

    assert!(validate_ui_component_record_fields(
        "example.button",
        "Button",
        "A button",
        "this string is stored as opaque import code {{",
        "this string is stored as opaque source code }}",
        UiComponentRecordOrigin::Custom,
        "local",
        "controls",
        &upstream,
        "1.0.0",
        &["action".into()],
    )
    .is_ok());
}

#[test]
fn plugin_component_complete_source_can_own_all_imports_without_changing_other_origins() {
    let upstream = UiComponentRecordUpstream {
        identity: "fixture:view".into(),
        version: "1.0.0-beta.1".into(),
    };
    let validate = |origin, imports, version| {
        validate_ui_component_record_fields(
            "plugin.fixture",
            "View",
            "Complete plugin source",
            imports,
            "export default function Block() { return null; }",
            origin,
            "plugin",
            "components",
            &upstream,
            version,
            &[],
        )
    };
    assert!(validate(UiComponentRecordOrigin::Plugin, "", "1.0.0-beta.1").is_ok());
    assert!(validate(
        UiComponentRecordOrigin::Plugin,
        "// Plugin component: fixture:view",
        "1.0.0"
    )
    .is_ok());
    for origin in [
        UiComponentRecordOrigin::Custom,
        UiComponentRecordOrigin::Official,
    ] {
        assert_eq!(
            validate(origin, "", "1.0.0"),
            Err(UiManagementInvariantError::EmptyComponentImportCode)
        );
        assert_eq!(
            validate(origin, "// import owned by source", "1.0.0-beta.1"),
            Err(UiManagementInvariantError::InvalidComponentVersion)
        );
    }
    assert_eq!(
        validate(UiComponentRecordOrigin::Plugin, "", ""),
        Err(UiManagementInvariantError::InvalidComponentVersion)
    );
    assert_eq!(
        UiComponentRecordOrigin::try_from("plugin"),
        Ok(UiComponentRecordOrigin::Plugin)
    );
}
