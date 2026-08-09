use crate::routes::model_definitions::{CreateModelDefinitionBody, UpdateModelDefinitionBody};

#[test]
fn ac_003_create_accepts_identity_but_update_rejects_template_mutability() {
    let create = serde_json::from_value::<CreateModelDefinitionBody>(serde_json::json!({
        "scope_kind": "workspace",
        "template_provider": "core",
        "template_code": "general",
        "template_version": "v1",
        "code": "orders",
        "title": "Orders",
        "description": null,
        "status": "published"
    }))
    .expect("create contract must carry template identity");
    assert_eq!(create.template_provider, "core");
    assert_eq!(create.template_code, "general");
    assert_eq!(create.template_version, "v1");

    let update = serde_json::from_value::<UpdateModelDefinitionBody>(serde_json::json!({
        "title": "Renamed",
        "template_provider": "runtime-extension",
        "template_code": "replacement",
        "template_version": "v2"
    }));
    assert!(
        update.is_err(),
        "update contract must reject template identity"
    );
}
