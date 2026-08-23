use crate::{
    validate_ui_code_template, validate_ui_component_record_fields, UiComponentRecordOrigin,
    UiComponentRecordUpstream, UiManagementInvariantError,
};

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
