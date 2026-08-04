use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    application_public_api::{
        api_keys::{
            ApplicationApiKeyService, CreateApplicationApiKeyCommand,
            ListApplicationApiKeysCommand, RevokeApplicationApiKeyCommand,
        },
        mapping::{
            validate_application_api_mapping, ApplicationApiMappingConfig,
            ApplicationApiMappingInput, ApplicationApiMappingOutput, ApplicationApiMappingService,
            GetApplicationApiMappingCommand, ReplaceApplicationApiMappingCommand,
            WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod, WorkflowExtensionResponseMode,
        },
        publications::{
            ApplicationPublicationService, LoadActiveApplicationPublicationCommand,
            PublishApplicationCommand, SetApplicationApiEnabledCommand,
        },
        workflow_extension::{
            CreateWorkflowExtensionRunCommand, WorkflowExtensionRequestParameters,
            WorkflowExtensionRunService,
        },
        workflow_schedule::{
            workflow_schedule_cron_matches, DispatchWorkflowScheduleCommand,
            GetWorkflowScheduleTriggerCommand, ReplaceWorkflowScheduleTriggerCommand,
            WorkflowScheduleDispatchOutcome, WorkflowScheduleTriggerService,
            WORKFLOW_SCHEDULE_RUN_QUEUE,
        },
        ApplicationPublicApiTestHarness,
    },
    auth::{hash_api_key_token, ApiKeyService},
    errors::ControlPlaneError,
    ports::{
        ApiKeyRepository, ApplicationJsDependencySelectionRepository, ClaimedTask,
        CreateApiKeyInput, EphemeralInspectionCapabilities, FlowRepository,
        ReplaceApplicationJsDependencySelectionInput, TaskQueue,
    },
};
use std::sync::{Arc, Mutex};
use time::Duration;
use uuid::Uuid;

mod anthropic_compat;
mod client_protocol_envelope;
mod conversations;
mod native_run;
mod openai_compat;
mod publications;
mod published_workflow_operation;
mod resume;
mod run_service;
mod workflow_start_http_inputs;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn other_user_id() -> Uuid {
    Uuid::from_u128(0x22222222222222222222222222222222)
}

fn root_user_id() -> Uuid {
    Uuid::from_u128(0x33333333333333333333333333333333)
}

fn applications_console_group() -> domain::ConsolePolicyGroup {
    domain::ConsolePolicyGroup::settings_feature(
        access_control::SYSTEM_APPLICATIONS_SETTINGS_FEATURE_ID,
    )
    .expect("applications settings feature id must be valid")
}

fn application_operation_id(value: &str) -> domain::ConsoleOperationId {
    domain::ConsoleOperationId::try_from(value).expect("application operation id must be valid")
}

fn application_console_policy(
    operations: Vec<domain::ConsoleOperationPolicy>,
) -> domain::RoleConsolePolicy {
    domain::RoleConsolePolicy::new(
        Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            applications_console_group(),
            operations,
        )],
    )
}

fn application_simple_operation(
    operation_id: &str,
    enabled: bool,
) -> domain::ConsoleOperationPolicy {
    domain::ConsoleOperationPolicy::simple(application_operation_id(operation_id), enabled)
}

fn application_row_operation(
    operation_id: &str,
    scope: domain::ConsoleOperationRowScope,
) -> domain::ConsoleOperationPolicy {
    domain::ConsoleOperationPolicy::row(application_operation_id(operation_id), scope)
}

fn workflow_extension_mapping(slug: &str) -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: Some(WorkflowExtensionApiConfig {
            slug: slug.into(),
            method: WorkflowExtensionHttpMethod::Post,
            response_mode: WorkflowExtensionResponseMode::Async,
        }),
    }
}

fn application_public_api_code_js_dependency_document(flow_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": { "flowId": flow_id.to_string(), "name": "Code Imports", "description": "", "tags": [] },
        "graph": {
            "nodes": [
                {
                    "id": "node-start",
                    "type": "start",
                    "alias": "Start",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 0, "y": 0 },
                    "configVersion": 1,
                    "config": {},
                    "bindings": {},
                    "outputs": []
                },
                {
                    "id": "node-code",
                    "type": "code",
                    "alias": "Code",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": { "imports": ["zod"] },
                    "bindings": {},
                    "outputs": [{ "key": "result", "title": "Result", "valueType": "json" }]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-code",
                    "source": "node-start",
                    "target": "node-code",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": { "viewport": { "x": 0, "y": 0, "zoom": 1 }, "annotations": [], "activeContainerPath": [] }
    })
}

fn workflow_schedule_start_contract_document() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": "1flowbase.flow/v2",
        "graph": {
            "nodes": [{
                "id": "node-workflow-start",
                "type": "workflow_start",
                "config": {
                    "input_fields": [
                        {
                            "key": "customer_id",
                            "valueType": "string",
                            "required": true,
                            "source": "path"
                        },
                        {
                            "key": "attempts",
                            "valueType": "number",
                            "defaultValue": 3,
                            "source": "query"
                        },
                        {
                            "key": "enabled",
                            "valueType": "boolean",
                            "source": "body"
                        }
                    ]
                }
            }],
            "edges": []
        }
    })
}

#[derive(Default)]
struct RecordingTaskQueue {
    enqueued: Mutex<Vec<(String, serde_json::Value, Option<String>)>>,
    attempts: Mutex<usize>,
    failures_remaining: Mutex<usize>,
}

impl RecordingTaskQueue {
    fn failing_once() -> Self {
        Self {
            failures_remaining: Mutex::new(1),
            ..Self::default()
        }
    }

    fn attempt_count(&self) -> usize {
        *self
            .attempts
            .lock()
            .expect("recording task queue attempts mutex poisoned")
    }
}

#[async_trait]
impl TaskQueue for RecordingTaskQueue {
    async fn enqueue(
        &self,
        queue: &str,
        payload: serde_json::Value,
        idempotency_key: Option<&str>,
    ) -> Result<String> {
        *self
            .attempts
            .lock()
            .expect("recording task queue attempts mutex poisoned") += 1;
        let mut failures_remaining = self
            .failures_remaining
            .lock()
            .expect("recording task queue failures mutex poisoned");
        if *failures_remaining > 0 {
            *failures_remaining -= 1;
            anyhow::bail!("injected task queue enqueue failure");
        }
        drop(failures_remaining);

        let mut enqueued = self
            .enqueued
            .lock()
            .expect("recording task queue mutex poisoned");
        if let Some(idempotency_key) = idempotency_key {
            if let Some(index) = enqueued
                .iter()
                .position(|entry| entry.0 == queue && entry.2.as_deref() == Some(idempotency_key))
            {
                return Ok(format!("task-{}", index + 1));
            }
        }
        let task_id = format!("task-{}", enqueued.len() + 1);
        enqueued.push((
            queue.to_string(),
            payload,
            idempotency_key.map(ToOwned::to_owned),
        ));
        Ok(task_id)
    }

    async fn claim(
        &self,
        _queue: &str,
        _worker: &str,
        _visibility_timeout: Duration,
    ) -> Result<Option<ClaimedTask>> {
        Ok(None)
    }

    async fn ack(&self, _queue: &str, _task_id: &str, _worker: &str) -> Result<bool> {
        Ok(false)
    }

    async fn fail(
        &self,
        _queue: &str,
        _task_id: &str,
        _worker: &str,
        _reason: &str,
    ) -> Result<bool> {
        Ok(false)
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }
}

mod api_key_service;
mod mapping_and_publication;
mod operation_permissions;
mod workflow_http_operation;
mod workflow_schedule;
