use crate::{
    validate_ui_code_template, validate_ui_component_contract, FrontendComponentContract,
    FrontendComponentExample, UiComponentLocator, UiManagementInvariantError,
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
fn ac_004_managed_component_contract_must_match_real_export() {
    let locator = UiComponentLocator {
        provider_code: "1flowbase".to_string(),
        contribution_code: "frontstage.js-ui-block".to_string(),
        module_source: "antd".to_string(),
        export_name: "Button".to_string(),
    };
    let contract = FrontendComponentContract {
        component_code: "button".to_string(),
        export_name: "Table".to_string(),
        upstream: None,
        description: "Controlled button".to_string(),
        props: Vec::new(),
        limitations: vec!["Uses the registered host export.".to_string()],
        examples: vec![FrontendComponentExample {
            title: "Primary action".to_string(),
            code: "<Button>Save</Button>".to_string(),
        }],
        insert_snippet: "<Button>Save</Button>".to_string(),
    };

    assert_eq!(
        validate_ui_component_contract(&locator, &contract),
        Err(UiManagementInvariantError::ComponentExportMismatch)
    );
}
