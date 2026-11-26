use interface_runtime::BindingId;

use super::*;

struct UnavailableApplicationsPort;

impl ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput> for UnavailableApplicationsPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: ApplicationsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("applications fixture unavailable").into(),
            ))
        })
    }
}

#[test]
fn schedule_creation_accepts_omitted_or_empty_timezone() {
    for input in [
        serde_json::json!({"cron": "0 9 * * *"}),
        serde_json::json!({"cron": "0 9 * * *", "timezone": ""}),
    ] {
        let config = serde_json::from_value(input).unwrap();
        let parsed = parse_create_workflow_trigger_config(
            Some(domain::WorkflowTriggerType::Schedule),
            Some(config),
        )
        .unwrap();
        assert!(matches!(
            parsed,
            Some(CreateWorkflowTriggerConfig::Schedule { timezone, .. }) if timezone.is_empty()
        ));
    }
}

#[test]
fn f09a_registry_freezes_all_console_application_bindings() {
    let registry = compile_registry(Arc::new(UnavailableApplicationsPort)).unwrap();
    for declaration in DECLARATIONS {
        assert!(registry
            .binding(&BindingId::new(declaration.binding_id).unwrap())
            .is_some());
    }
    assert_eq!(registry.bindings().count(), DECLARATIONS.len());
}

#[test]
fn managed_hook_projection_validates_forty_agent_logs_and_agent_flow_applications() {
    use crate::routes::applications_group::applications::{
        ApplicationEnvironmentVariableBody, ApplicationTagResponse,
    };
    use extension_contracts::ManagedProjectionContract;

    let output_contract = ManagedProjectionContract {
        contract_id: ApplicationsOutput::CONTRACT_ID.into(),
        contract_version: ApplicationsOutput::CONTRACT_VERSION.into(),
        schema: ApplicationsOutput::managed_projection_schema().unwrap(),
    }
    .compile_for_registration()
    .unwrap();

    let applications = (0..40)
        .map(|index| ApplicationSummaryResponse {
            id: Uuid::from_u128((index + 1) as u128).to_string(),
            application_type: match index % 3 {
                0 => ApplicationTypeDto::AgentLogs,
                1 => ApplicationTypeDto::AgentFlow,
                _ => ApplicationTypeDto::Workflow,
            },
            name: format!("App {index}"),
            description: format!("Description {index}"),
            icon: None,
            icon_type: None,
            icon_background: None,
            created_by: "owner".into(),
            updated_at: "2026-08-02T00:00:00Z".into(),
            tags: (0..if index == 0 { 36 } else { 2 })
                .map(|tag_idx| ApplicationTagResponse {
                    id: Uuid::from_u128((1_000 + tag_idx) as u128).to_string(),
                    name: format!("tag-{tag_idx}"),
                })
                .collect(),
        })
        .collect();

    let projected_apps = ApplicationsOutput::Applications(applications)
        .project_for_managed_hook()
        .expect("40 applications must project bounded hook preview instead of failing");
    assert_eq!(projected_apps["0"].as_array().unwrap().len(), 32);
    assert_eq!(
        projected_apps["0"][0]["application_type"],
        serde_json::json!({"variant": "AgentLogs"})
    );
    assert_eq!(
        projected_apps["0"][1]["application_type"],
        serde_json::json!({"variant": "AgentFlow"})
    );
    assert_eq!(projected_apps["0"][0]["tags"].as_array().unwrap().len(), 32);
    output_contract.validate(&projected_apps).unwrap();

    let projected_catalog = ApplicationsOutput::Catalog(ApplicationCatalogResponse {
        collectors: Vec::new(),
        types: vec![
            ApplicationTypeOptionResponse {
                value: ApplicationTypeDto::AgentFlow,
                label: "Agent Flow".into(),
            },
            ApplicationTypeOptionResponse {
                value: ApplicationTypeDto::Workflow,
                label: "Workflow".into(),
            },
            ApplicationTypeOptionResponse {
                value: ApplicationTypeDto::AgentLogs,
                label: "Agent Logs".into(),
            },
        ],
        workflow_triggers: Vec::new(),
        tags: Vec::new(),
    })
    .project_for_managed_hook()
    .unwrap();
    output_contract.validate(&projected_catalog).unwrap();

    let input_contract = ManagedProjectionContract {
        contract_id: ApplicationsInput::CONTRACT_ID.into(),
        contract_version: ApplicationsInput::CONTRACT_VERSION.into(),
        schema: ApplicationsInput::managed_projection_schema().unwrap(),
    }
    .compile_for_registration()
    .unwrap();

    let projected_env_input = ApplicationsInput::ReplaceEnvironmentVariables {
        application_id: Uuid::from_u128(1),
        body: ReplaceApplicationEnvironmentVariablesBody {
            variables: (0..40)
                .map(|idx| ApplicationEnvironmentVariableBody {
                    name: format!("VAR_{idx}"),
                    value_type: "string".into(),
                    value: serde_json::json!("v"),
                    description: String::new(),
                })
                .collect(),
        },
    }
    .project_for_managed_hook()
    .expect("40 environment variables must project bounded hook preview");
    assert_eq!(
        projected_env_input["body"]["variables"]
            .as_array()
            .unwrap()
            .len(),
        32
    );
    input_contract.validate(&projected_env_input).unwrap();
}
