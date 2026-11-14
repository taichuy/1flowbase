use super::*;

test_support_item! { pub(crate), [
/// Constructs private owner values without widening their production visibility.
#[cfg(any(test, feature = "test-support"))]
]
fn root_2014_projection_fixture(
    sentinel: &str,
) -> (
    interface_runtime::ManagedInterfaceProjection,
    interface_runtime::ManagedInterfaceProjection,
) {
    let config = serde_json::json!({"innocent": {"access_key": sentinel, "secret_key": sentinel},
        "endpoint": format!("https://user:{sentinel}@example.invalid"),
        "schema": {"default": sentinel, "examples": [sentinel]}});
    let input = FileStoragesInput::Create(CreateFileStorageBody {
        code: "projection-fixture".into(),
        title: "Storage".into(),
        driver_type: "rustfs".into(),
        enabled: true,
        is_default: false,
        config_json: config.clone(),
        rule_json: config.clone(),
    });
    let output = FileStoragesOutput::Item(FileStorageResponse {
        id: uuid::Uuid::nil().to_string(),
        code: "projection-fixture".into(),
        title: "Storage".into(),
        driver_type: "rustfs".into(),
        enabled: true,
        is_default: false,
        config_json: config.clone(),
        rule_json: config.clone(),
        health_status: "healthy".into(),
        last_health_error: Some(sentinel.into()),
    });
    let input_view = interface_runtime::ManagedInterfaceProjection::from_contract(&input).unwrap();
    let output_view =
        interface_runtime::ManagedInterfaceProjection::from_contract(&output).unwrap();
    match input {
        FileStoragesInput::Create(original) => assert_eq!(original.config_json, config),
        _ => unreachable!(),
    }
    match output {
        FileStoragesOutput::Item(original) => assert_eq!(original.config_json, config),
        _ => unreachable!(),
    }
    (input_view, output_view)
}
}
