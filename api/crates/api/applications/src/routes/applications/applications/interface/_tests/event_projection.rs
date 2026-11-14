use super::*;
use serde_json::json;
#[test]
fn event_values_keep_actual_names_and_exclude_environment_and_dependency_variants() {
    let input = ApplicationsInput::Create(serde_json::from_value(json!({"application_type":"agent_flow","name":"actual name","description":"actual description"})).unwrap());
    let validator =
        jsonschema::validator_for(&ApplicationsInput::event_projection_schema().unwrap()).unwrap();
    assert!(validator.is_valid(&input.project_for_event().unwrap()));
    assert!(!validator.is_valid(&input.project_for_managed_hook().unwrap()));
    assert_eq!(
        input.project_for_event().unwrap()["0"]["name"],
        "actual name"
    );
    assert_eq!(
        input.project_for_managed_hook().unwrap()["0"]["name"]["byte_count"],
        11
    );
    assert!(ApplicationsInput::ListEnvironmentVariables {
        application_id: Uuid::nil()
    }
    .project_for_event()
    .is_none());
    assert!(ApplicationsInput::ReplaceJsDependency {
        application_id: Uuid::nil(),
        body: serde_json::from_value(
            json!({"installation_id":"private-installation","alias":"private","target":"private"})
        )
        .unwrap()
    }
    .project_for_event()
    .is_none());
    assert_eq!(
        event_projection::input_schema()["oneOf"][1]["properties"]["0"]["properties"]["name"]
            ["type"],
        "string"
    );
    assert!(!event_projection::OPERATIONS
        .iter()
        .any(|operation| operation.contains("environment") || operation.contains("dependencies")));
}

struct Noop;
impl ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput> for Noop {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        _: ApplicationsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationsOutput> {
        Box::pin(std::future::pending())
    }
}
#[test]
fn application_registry_exposes_only_the_five_approved_operations_and_legacy_compiler_defaults_closed(
) {
    let registry = compile_registry(Arc::new(Noop)).unwrap();
    let mut actual = registry
        .event_capabilities()
        .map(|capability| capability.definition().interface_id().as_str().to_owned())
        .collect::<Vec<_>>();
    actual.sort();
    assert_eq!(
        actual,
        vec![
            "applications.create",
            "applications.delete",
            "applications.get",
            "applications.list",
            "applications.update"
        ]
    );
    assert!(registry
        .definition(
            &interface_runtime::InterfaceId::new("applications.environment-variables.list")
                .unwrap()
        )
        .is_some());
    assert!(registry
        .event_capability(
            &interface_runtime::InterfaceId::new("applications.environment-variables.list")
                .unwrap()
        )
        .is_none());
    let legacy = console_interface::compile_registry(
        "test.applications",
        "graph:legacy-applications",
        DECLARATIONS,
        Arc::new(Noop),
    )
    .unwrap();
    assert_eq!(legacy.event_capabilities().len(), 0);
}

#[test]
fn application_list_output_keeps_actual_names_and_matches_the_declared_schema() {
    let output = ApplicationsOutput::Applications(vec![ApplicationSummaryResponse {
        id: Uuid::now_v7().to_string(),
        application_type: ApplicationTypeDto::AgentFlow,
        name: "actual app".into(),
        description: "actual app description".into(),
        icon: None,
        icon_type: None,
        icon_background: None,
        created_by: Uuid::now_v7().to_string(),
        updated_at: "2026-11-07T00:00:00Z".into(),
        tags: Vec::new(),
    }]);
    let projected = output.project_for_event().unwrap();
    assert_eq!(projected["0"][0]["name"], "actual app");
    let validator =
        jsonschema::validator_for(&ApplicationsOutput::event_projection_schema().unwrap()).unwrap();
    assert!(validator.is_valid(&projected));
    assert!(!validator.is_valid(&output.project_for_managed_hook().unwrap()));
}
