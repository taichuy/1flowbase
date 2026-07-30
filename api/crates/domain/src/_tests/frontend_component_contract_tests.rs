use crate::{
    FrontendBlockCodeModule, FrontendComponentContract, FrontendComponentExample,
    FrontendComponentProp, FrontendComponentUpstream, FrontendModuleAsset, FrontendModuleAssetRole,
    FrontendModuleBinding,
};

#[test]
fn d2_ac_001_module_export_set_is_serialized_as_domain_truth() {
    let module = FrontendBlockCodeModule {
        source: "@1flowbase/block-sdk".into(),
        version: "1.0.0".into(),
        exports: vec!["blockSdkVersion".into()],
        binding: FrontendModuleBinding::Fetched,
        assets: vec![FrontendModuleAsset {
            path: "browser-assets/block-sdk.js".into(),
            role: FrontendModuleAssetRole::BrowserModule,
            media_type: "text/javascript; charset=utf-8".into(),
            sha256: "89d33c09ed7013cf4f60f07b5b4b511686e57e011867ec7656f8bc3538c0298f".into(),
        }],
        type_declarations: "declare module '@1flowbase/block-sdk' {}".into(),
        components: vec![],
    };

    let value = serde_json::to_value(module).unwrap();
    assert_eq!(value["exports"], serde_json::json!(["blockSdkVersion"]));
    assert_eq!(value["binding"], "fetched");
    assert_eq!(value["assets"][0]["role"], "browser_module");
    assert_eq!(
        value["assets"][0]["media_type"],
        "text/javascript; charset=utf-8"
    );
}

#[test]
fn d2_ac_001_renders_standard_react_typescript_component_contract() {
    let contract = FrontendComponentContract {
        component_code: "button".into(),
        export_name: "Button".into(),
        upstream: Some(FrontendComponentUpstream {
            package: "antd".into(),
            component: "Button".into(),
            version: "5.x".into(),
        }),
        description: "Ant Design Button React component.".into(),
        props: vec![FrontendComponentProp {
            name: "actionId".into(),
            type_name: "string".into(),
            required: false,
            description: "点击后发送的区块 action 标识。".into(),
        }],
        limitations: vec!["不支持 React onClick。".into()],
        examples: vec![FrontendComponentExample {
            title: "触发保存操作".into(),
            code: "<Button actionId=\"save\">保存</Button>".into(),
        }],
        insert_snippet: "<Button actionId=\"save\">保存</Button>".into(),
    };

    let declaration = contract.typescript_declaration("@1flowbase/native-components");

    assert!(declaration.contains("declare module '@1flowbase/native-components'"));
    assert!(declaration.contains("export interface ButtonProps {"));
    assert!(declaration.contains("import('react').ComponentType<ButtonProps>"));
    assert!(!declaration.contains("FacadeCommonProps"));
    assert!(declaration.contains("readonly actionId?: string;"));
    assert!(declaration.contains("@remarks"));
    assert!(declaration.contains("不支持 React onClick"));
    assert!(declaration.contains("@example 触发保存操作"));
    assert!(declaration.contains("@see antd@5.x Button"));
    assert!(!declaration.contains("@1flowbase-component"));
}

#[test]
fn d2_ac_001_renders_default_react_export_declaration() {
    let contract = FrontendComponentContract {
        component_code: "default_fixture".into(),
        export_name: "default".into(),
        upstream: None,
        description: "Default React component.".into(),
        props: vec![],
        limitations: vec!["Host-owned React singleton.".into()],
        examples: vec![],
        insert_snippet: "<DefaultExport />".into(),
    };

    let declaration = contract.typescript_declaration("@acme/default-component");
    assert!(declaration.contains("export interface DefaultExportProps"));
    assert!(declaration
        .contains("const DefaultExport: import('react').ComponentType<DefaultExportProps>"));
    assert!(declaration.contains("export default DefaultExport"));
    assert!(!declaration.contains("export const default"));
}
