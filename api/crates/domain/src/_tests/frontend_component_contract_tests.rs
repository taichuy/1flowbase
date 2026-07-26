use crate::{
    FrontendComponentContract, FrontendComponentExample, FrontendComponentProp,
    FrontendComponentUpstream,
};

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
