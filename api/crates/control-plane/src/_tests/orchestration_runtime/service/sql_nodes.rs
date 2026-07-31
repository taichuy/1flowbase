use super::*;

use std::sync::{Arc, Mutex};

use plugin_framework::data_source_contract::{
    DataSourceCreateRecordInput, DataSourceCreateRecordOutput, DataSourceDeleteRecordInput,
    DataSourceDeleteRecordOutput, DataSourceGetRecordInput, DataSourceGetRecordOutput,
    DataSourceListRecordsInput, DataSourceListRecordsOutput, DataSourceUpdateRecordInput,
    DataSourceUpdateRecordOutput, NativeSqlExecutionItem, NativeSqlExecutionOutput,
};
use runtime_core::runtime_engine::{DataSourceRuntimeRecordBackend, RuntimeEngine};

use crate::orchestration_runtime::test_support::{
    InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime,
};

#[derive(Clone, Copy)]
enum SqlPreviewResult {
    Success,
    DataSourceError,
}

struct SqlPreviewBackend {
    result: SqlPreviewResult,
    captured_sql: Mutex<Vec<String>>,
}

impl SqlPreviewBackend {
    fn new(result: SqlPreviewResult) -> Self {
        Self {
            result,
            captured_sql: Mutex::new(Vec::new()),
        }
    }

    fn captured_sql(&self) -> Vec<String> {
        self.captured_sql.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl DataSourceRuntimeRecordBackend for SqlPreviewBackend {
    async fn execute_sql(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: &str,
        sql: &str,
    ) -> anyhow::Result<NativeSqlExecutionOutput> {
        self.captured_sql.lock().unwrap().push(sql.to_string());
        match self.result {
            SqlPreviewResult::Success => Ok(NativeSqlExecutionOutput {
                results: vec![NativeSqlExecutionItem::Completion {
                    affected_rows: 1,
                    native_status: Some("SELECT 1".to_string()),
                }],
            }),
            SqlPreviewResult::DataSourceError => {
                Err(anyhow::anyhow!("syntax error at or near user"))
            }
        }
    }

    async fn list_records(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: Uuid,
        _input: DataSourceListRecordsInput,
    ) -> anyhow::Result<DataSourceListRecordsOutput> {
        Err(anyhow::anyhow!("unused list_records test path"))
    }

    async fn get_record(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: Uuid,
        _input: DataSourceGetRecordInput,
    ) -> anyhow::Result<DataSourceGetRecordOutput> {
        Err(anyhow::anyhow!("unused get_record test path"))
    }

    async fn create_record(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: Uuid,
        _input: DataSourceCreateRecordInput,
    ) -> anyhow::Result<DataSourceCreateRecordOutput> {
        Err(anyhow::anyhow!("unused create_record test path"))
    }

    async fn update_record(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: Uuid,
        _input: DataSourceUpdateRecordInput,
    ) -> anyhow::Result<DataSourceUpdateRecordOutput> {
        Err(anyhow::anyhow!("unused update_record test path"))
    }

    async fn delete_record(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: Uuid,
        _input: DataSourceDeleteRecordInput,
    ) -> anyhow::Result<DataSourceDeleteRecordOutput> {
        Err(anyhow::anyhow!("unused delete_record test path"))
    }
}

fn sql_preview_service(
    result: SqlPreviewResult,
) -> (
    OrchestrationRuntimeService<InMemoryOrchestrationRuntimeRepository, InMemoryProviderRuntime>,
    Arc<SqlPreviewBackend>,
) {
    let repository = InMemoryOrchestrationRuntimeRepository::with_permissions(vec![
        "application.view.all",
        "application.create.all",
    ]);
    let backend = Arc::new(SqlPreviewBackend::new(result));
    let runtime_engine =
        RuntimeEngine::for_tests_with_models_and_data_source_backend(Vec::new(), backend.clone());
    (
        OrchestrationRuntimeService::new(
            repository,
            InMemoryProviderRuntime::default(),
            Arc::new(runtime_engine),
            "test-master-key",
        ),
        backend,
    )
}

fn sql_preview_document(flow_id: Uuid, sql: &str) -> Value {
    json!({
        "schemaVersion": "1flowbase.flow/v2",
        "meta": {
            "flowId": flow_id.to_string(),
            "name": "SQL Preview",
            "description": "",
            "tags": []
        },
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
                    "id": "node-sql",
                    "type": "sql",
                    "alias": "SQL",
                    "description": "",
                    "containerId": null,
                    "position": { "x": 240, "y": 0 },
                    "configVersion": 1,
                    "config": { "data_source_instance_id": "main" },
                    "bindings": {
                        "sql": { "kind": "templated_text", "value": sql }
                    },
                    "outputs": [
                        { "key": "results", "title": "Results", "valueType": "array" }
                    ]
                }
            ],
            "edges": [
                {
                    "id": "edge-start-sql",
                    "source": "node-start",
                    "target": "node-sql",
                    "sourceHandle": null,
                    "targetHandle": null,
                    "containerId": null,
                    "points": []
                }
            ]
        },
        "editor": {
            "viewport": { "x": 0, "y": 0, "zoom": 1 },
            "annotations": [],
            "activeContainerPath": []
        }
    })
}

async fn run_sql_preview(
    service: &OrchestrationRuntimeService<
        InMemoryOrchestrationRuntimeRepository,
        InMemoryProviderRuntime,
    >,
    sql: &str,
) -> domain::NodeDebugPreviewResult {
    let seeded = service.seed_application_with_flow("SQL Preview").await;
    service
        .start_node_debug_preview(StartNodeDebugPreviewCommand {
            actor_user_id: seeded.actor_user_id,
            application_id: seeded.application_id,
            node_id: "node-sql".to_string(),
            input_payload: json!({
                "node-start": { "query": "Ada" }
            }),
            document_snapshot: Some(sql_preview_document(seeded.flow_id, sql)),
            debug_session_id: None,
        })
        .await
        .expect("SQL preview should return a persisted Last Run")
}

#[tokio::test]
async fn ac_001_sql_preview_executes_with_persisted_flow_context() {
    let (service, backend) = sql_preview_service(SqlPreviewResult::Success);

    let outcome = run_sql_preview(&service, "select '{{node-start.query}}'").await;

    assert_eq!(backend.captured_sql(), vec!["select 'Ada'"]);
    assert_eq!(outcome.flow_run.status, domain::FlowRunStatus::Succeeded);
    assert_eq!(outcome.node_run.status, domain::NodeRunStatus::Succeeded);
    assert_eq!(outcome.node_run.input_payload["sql"], "select 'Ada'");
    assert_eq!(outcome.node_run.debug_payload["sql"], "select 'Ada'");
    assert!(outcome
        .events
        .iter()
        .any(|event| event.event_type == "node_preview_completed"));
}

#[tokio::test]
async fn ac_002_sql_preview_persists_data_source_error_as_failed_last_run() {
    let (service, backend) = sql_preview_service(SqlPreviewResult::DataSourceError);

    let outcome = run_sql_preview(&service, "select * where user").await;

    assert_eq!(backend.captured_sql(), vec!["select * where user"]);
    assert_eq!(outcome.flow_run.status, domain::FlowRunStatus::Failed);
    assert_eq!(outcome.node_run.status, domain::NodeRunStatus::Failed);
    assert_eq!(outcome.node_run.input_payload["sql"], "select * where user");
    assert_eq!(outcome.node_run.debug_payload["sql"], "select * where user");
    assert!(outcome
        .node_run
        .error_payload
        .as_ref()
        .expect("failed SQL preview should persist its error payload")["message"]
        .as_str()
        .is_some_and(|message| message.contains("syntax error at or near user")));
    assert!(outcome
        .events
        .iter()
        .any(|event| event.event_type == "node_preview_failed"));
}
