use std::sync::Arc;

use anyhow::{anyhow, Result};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    model_definition::ModelDefinitionService,
    ports::{
        ModelDefinitionRepository, OrchestrationRuntimeRepository,
        UpsertDataModelSideEffectReceiptInput,
    },
};

const WORKFLOW_LIST_PAGE_SIZE_MAX: i64 = 100;

pub(super) struct DataModelNodeExecution {
    pub(super) output_payload: Value,
    pub(super) error_payload: Option<Value>,
    pub(super) metrics_payload: Value,
    pub(super) waiting_confirmation: Option<DataModelSideEffectConfirmation>,
}

pub(super) struct DataModelSideEffectConfirmation {
    pub(super) idempotency_key: String,
    pub(super) payload_hash: String,
    pub(super) request_payload: Value,
    pub(super) expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DataModelSideEffectPolicy {
    Disabled,
    ConfirmEachRun,
    AllowWithIdempotency,
}

pub(super) struct DataModelRunContext {
    pub(super) workspace_id: Uuid,
    pub(super) application_id: Uuid,
    pub(super) draft_id: Uuid,
    pub(super) flow_run_id: Uuid,
    pub(super) node_run_id: Uuid,
}

pub(super) async fn execute_data_model_node<R>(
    repository: R,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    actor: &domain::ActorContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    resolved_inputs: &Map<String, Value>,
    run_context: &DataModelRunContext,
) -> DataModelNodeExecution
where
    R: ModelDefinitionRepository + OrchestrationRuntimeRepository + Clone,
{
    let started_at = OffsetDateTime::now_utc();
    let result = execute_data_model_node_inner(
        repository,
        runtime_engine,
        actor,
        node,
        resolved_inputs,
        run_context,
    )
    .await;

    build_data_model_node_execution(started_at, result)
}

pub(super) async fn execute_confirmed_data_model_side_effect<R>(
    repository: R,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    actor: &domain::ActorContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    run_context: &DataModelRunContext,
    confirmation_payload: &Value,
) -> DataModelNodeExecution
where
    R: ModelDefinitionRepository + OrchestrationRuntimeRepository + Clone,
{
    let started_at = OffsetDateTime::now_utc();
    let result = execute_confirmed_data_model_side_effect_inner(
        repository,
        runtime_engine,
        actor,
        node,
        run_context,
        confirmation_payload,
    )
    .await;

    build_data_model_node_execution(started_at, result)
}

fn build_data_model_node_execution(
    started_at: OffsetDateTime,
    result: Result<DataModelExecutionResult>,
) -> DataModelNodeExecution {
    let elapsed_ms = (OffsetDateTime::now_utc() - started_at)
        .whole_milliseconds()
        .max(0);

    match result {
        Ok(DataModelExecutionResult::Completed {
            output_payload,
            side_effect_receipt,
            replayed,
        }) => DataModelNodeExecution {
            output_payload,
            error_payload: None,
            metrics_payload: json!({
                "runtime": "data_model",
                "duration_ms": elapsed_ms,
                "side_effect_receipt": side_effect_receipt.unwrap_or(Value::Null),
                "side_effect_replayed": replayed,
            }),
            waiting_confirmation: None,
        },
        Ok(DataModelExecutionResult::WaitingConfirmation(confirmation)) => DataModelNodeExecution {
            output_payload: json!({}),
            error_payload: None,
            metrics_payload: json!({
                "runtime": "data_model",
                "duration_ms": elapsed_ms,
                "waiting": "data_model_side_effect_confirmation",
                "idempotency_key": confirmation.idempotency_key,
                "payload_hash": confirmation.payload_hash,
            }),
            waiting_confirmation: Some(confirmation),
        },
        Err(error) => DataModelNodeExecution {
            output_payload: json!({}),
            error_payload: Some(json!({ "message": error.to_string() })),
            metrics_payload: json!({
                "runtime": "data_model",
                "duration_ms": elapsed_ms,
            }),
            waiting_confirmation: None,
        },
    }
}

enum DataModelExecutionResult {
    Completed {
        output_payload: Value,
        side_effect_receipt: Option<Value>,
        replayed: bool,
    },
    WaitingConfirmation(DataModelSideEffectConfirmation),
}

async fn execute_data_model_node_inner<R>(
    repository: R,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    actor: &domain::ActorContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    resolved_inputs: &Map<String, Value>,
    run_context: &DataModelRunContext,
) -> Result<DataModelExecutionResult>
where
    R: ModelDefinitionRepository + OrchestrationRuntimeRepository + Clone,
{
    let model_code = required_config_string(&node.config, "data_model_code")?;
    let action = data_model_action(node)?;
    let runtime = WorkflowDataModelRuntime::new(repository.clone(), runtime_engine);

    match action.as_str() {
        "list" => {
            let query = input_or_config_value(&node.config, resolved_inputs, "query")
                .unwrap_or(Value::Null);
            Ok(DataModelExecutionResult::Completed {
                output_payload: runtime.list(actor.clone(), model_code, query).await?,
                side_effect_receipt: None,
                replayed: false,
            })
        }
        "get" => {
            let record_id = required_record_id(&node.config, resolved_inputs)?;
            Ok(DataModelExecutionResult::Completed {
                output_payload: runtime.get(actor.clone(), model_code, record_id).await?,
                side_effect_receipt: None,
                replayed: false,
            })
        }
        "create" => {
            let payload = input_or_config_value(&node.config, resolved_inputs, "payload")
                .unwrap_or(Value::Null);
            execute_write_with_receipt(
                &runtime,
                &repository,
                actor,
                node,
                run_context,
                &action,
                model_code,
                None,
                payload,
            )
            .await
        }
        "update" => {
            let record_id = required_record_id(&node.config, resolved_inputs)?;
            let payload = input_or_config_value(&node.config, resolved_inputs, "payload")
                .unwrap_or(Value::Null);
            execute_write_with_receipt(
                &runtime,
                &repository,
                actor,
                node,
                run_context,
                &action,
                model_code,
                Some(record_id),
                payload,
            )
            .await
        }
        "delete" => {
            let record_id = required_record_id(&node.config, resolved_inputs)?;
            execute_write_with_receipt(
                &runtime,
                &repository,
                actor,
                node,
                run_context,
                &action,
                model_code,
                Some(record_id),
                Value::Null,
            )
            .await
        }
        other => Err(anyhow!("unsupported data_model action: {other}")),
    }
}

fn data_model_action(node: &orchestration_runtime::compiled_plan::CompiledNode) -> Result<String> {
    match node.node_type.as_str() {
        "data_model_list" => Ok("list".to_string()),
        "data_model_get" => Ok("get".to_string()),
        "data_model_create" => Ok("create".to_string()),
        "data_model_update" => Ok("update".to_string()),
        "data_model_delete" => Ok("delete".to_string()),
        other => Err(anyhow!("unsupported data_model node type: {other}")),
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_write_with_receipt<R>(
    runtime: &WorkflowDataModelRuntime<R>,
    repository: &R,
    actor: &domain::ActorContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    run_context: &DataModelRunContext,
    action: &str,
    model_code: String,
    record_id: Option<String>,
    payload: Value,
) -> Result<DataModelExecutionResult>
where
    R: ModelDefinitionRepository + OrchestrationRuntimeRepository + Clone,
{
    let policy = data_model_side_effect_policy(node)?;
    if policy == DataModelSideEffectPolicy::Disabled {
        return Err(anyhow!(
            "DATA_MODEL_SIDE_EFFECT_DISABLED: data_model {action} requires explicit side-effect policy"
        ));
    }

    if action != "delete" {
        ensure_object_payload(&payload)?;
    }

    let request_payload = json!({
        "action": action,
        "model_code": model_code,
        "record_id": record_id,
        "payload": payload,
    });
    let payload_hash = stable_json_hash(&request_payload);
    let idempotency_key = side_effect_idempotency_key(run_context, node, action, &payload_hash);

    if policy == DataModelSideEffectPolicy::ConfirmEachRun {
        return Ok(DataModelExecutionResult::WaitingConfirmation(
            DataModelSideEffectConfirmation {
                idempotency_key,
                payload_hash,
                request_payload,
                expires_at: OffsetDateTime::now_utc() + time::Duration::minutes(10),
            },
        ));
    }

    execute_write_with_idempotency(
        runtime,
        repository,
        actor,
        node,
        run_context,
        action,
        model_code,
        record_id,
        payload,
        idempotency_key,
        payload_hash,
    )
    .await
}

async fn execute_confirmed_data_model_side_effect_inner<R>(
    repository: R,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    actor: &domain::ActorContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    run_context: &DataModelRunContext,
    confirmation_payload: &Value,
) -> Result<DataModelExecutionResult>
where
    R: ModelDefinitionRepository + OrchestrationRuntimeRepository + Clone,
{
    let request_payload = confirmation_payload
        .get("request_payload")
        .ok_or_else(|| anyhow!("data_model side-effect confirmation is missing request payload"))?;
    let request = parse_side_effect_request(request_payload)?;
    let node_action = data_model_action(node)?;
    if request.action != node_action {
        return Err(anyhow!(
            "data_model side-effect confirmation action does not match node"
        ));
    }
    let config_model_code = required_config_string(&node.config, "data_model_code")?;
    if request.model_code != config_model_code {
        return Err(anyhow!(
            "data_model side-effect confirmation model does not match node"
        ));
    }
    let payload_hash = stable_json_hash(request_payload);
    let expected_payload_hash = required_payload_string(confirmation_payload, "payload_hash")?;
    if expected_payload_hash != payload_hash {
        return Err(anyhow!(
            "data_model side-effect confirmation payload hash mismatch"
        ));
    }
    let idempotency_key =
        side_effect_idempotency_key(run_context, node, &request.action, &payload_hash);
    let expected_idempotency_key =
        required_payload_string(confirmation_payload, "idempotency_key")?;
    if expected_idempotency_key != idempotency_key {
        return Err(anyhow!(
            "data_model side-effect confirmation idempotency key mismatch"
        ));
    }

    let runtime = WorkflowDataModelRuntime::new(repository.clone(), runtime_engine);
    execute_write_with_idempotency(
        &runtime,
        &repository,
        actor,
        node,
        run_context,
        &request.action,
        request.model_code,
        request.record_id,
        request.payload,
        idempotency_key,
        payload_hash,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn execute_write_with_idempotency<R>(
    runtime: &WorkflowDataModelRuntime<R>,
    repository: &R,
    actor: &domain::ActorContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    run_context: &DataModelRunContext,
    action: &str,
    model_code: String,
    record_id: Option<String>,
    payload: Value,
    idempotency_key: String,
    payload_hash: String,
) -> Result<DataModelExecutionResult>
where
    R: ModelDefinitionRepository + OrchestrationRuntimeRepository + Clone,
{
    let claim = repository
        .claim_data_model_side_effect_receipt(&UpsertDataModelSideEffectReceiptInput {
            workspace_id: run_context.workspace_id,
            application_id: run_context.application_id,
            draft_id: run_context.draft_id,
            flow_run_id: run_context.flow_run_id,
            node_run_id: run_context.node_run_id,
            node_id: node.node_id.clone(),
            action: action.to_string(),
            model_code: model_code.clone(),
            record_id: None,
            deleted_id: None,
            affected_count: 0,
            idempotency_key: idempotency_key.clone(),
            payload_hash: payload_hash.clone(),
            actor_user_id: actor.user_id,
            scope_id: actor.current_workspace_id,
            status: "pending".to_string(),
            output_payload: json!({}),
        })
        .await?;
    if !claim.claimed {
        let receipt = claim.record;
        if receipt.status != "succeeded" {
            return Err(anyhow!(
                "DATA_MODEL_SIDE_EFFECT_RECEIPT_NOT_REPLAYABLE: data_model {action} receipt status is {}",
                receipt.status
            ));
        }
        return Ok(DataModelExecutionResult::Completed {
            output_payload: receipt.output_payload.clone(),
            side_effect_receipt: Some(receipt_payload(&receipt)),
            replayed: true,
        });
    }

    let write_result = match action {
        "create" => {
            runtime
                .create(actor.clone(), model_code.clone(), payload.clone())
                .await
        }
        "update" => {
            let record_id = record_id
                .clone()
                .ok_or_else(|| anyhow!("data_model config.record_id is required"))?;
            runtime
                .update(
                    actor.clone(),
                    model_code.clone(),
                    record_id,
                    payload.clone(),
                )
                .await
        }
        "delete" => {
            let record_id = record_id
                .clone()
                .ok_or_else(|| anyhow!("data_model config.record_id is required"))?;
            runtime
                .delete(actor.clone(), model_code.clone(), record_id)
                .await
        }
        other => return Err(anyhow!("unsupported data_model write action: {other}")),
    };
    let output_payload = match write_result {
        Ok(output_payload) => output_payload,
        Err(error) => {
            let _ = repository
                .upsert_data_model_side_effect_receipt(&UpsertDataModelSideEffectReceiptInput {
                    workspace_id: run_context.workspace_id,
                    application_id: run_context.application_id,
                    draft_id: run_context.draft_id,
                    flow_run_id: run_context.flow_run_id,
                    node_run_id: run_context.node_run_id,
                    node_id: node.node_id.clone(),
                    action: action.to_string(),
                    model_code: model_code.clone(),
                    record_id: None,
                    deleted_id: None,
                    affected_count: 0,
                    idempotency_key: idempotency_key.clone(),
                    payload_hash: payload_hash.clone(),
                    actor_user_id: actor.user_id,
                    scope_id: actor.current_workspace_id,
                    status: "failed".to_string(),
                    output_payload: json!({ "error": error.to_string() }),
                })
                .await;
            return Err(error);
        }
    };

    let receipt = repository
        .upsert_data_model_side_effect_receipt(&UpsertDataModelSideEffectReceiptInput {
            workspace_id: run_context.workspace_id,
            application_id: run_context.application_id,
            draft_id: run_context.draft_id,
            flow_run_id: run_context.flow_run_id,
            node_run_id: run_context.node_run_id,
            node_id: node.node_id.clone(),
            action: action.to_string(),
            model_code,
            record_id: output_record_id(&output_payload),
            deleted_id: output_payload
                .get("deleted_id")
                .and_then(Value::as_str)
                .map(str::to_string),
            affected_count: output_payload
                .get("affected_count")
                .and_then(Value::as_i64)
                .unwrap_or(1),
            idempotency_key,
            payload_hash,
            actor_user_id: actor.user_id,
            scope_id: actor.current_workspace_id,
            status: "succeeded".to_string(),
            output_payload: output_payload.clone(),
        })
        .await?;

    Ok(DataModelExecutionResult::Completed {
        output_payload,
        side_effect_receipt: Some(receipt_payload(&receipt)),
        replayed: false,
    })
}

struct DataModelSideEffectRequest {
    action: String,
    model_code: String,
    record_id: Option<String>,
    payload: Value,
}

fn parse_side_effect_request(value: &Value) -> Result<DataModelSideEffectRequest> {
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("data_model side-effect request payload must be object"))?;
    let record_id = match object.get("record_id") {
        Some(Value::String(value)) if !value.trim().is_empty() => Some(value.trim().to_string()),
        _ => None,
    };

    Ok(DataModelSideEffectRequest {
        action: required_payload_string(value, "action")?,
        model_code: required_payload_string(value, "model_code")?,
        record_id,
        payload: object.get("payload").cloned().unwrap_or(Value::Null),
    })
}

fn required_payload_string(value: &Value, key: &'static str) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("data_model side-effect payload {key} is required"))
}

fn side_effect_idempotency_key(
    run_context: &DataModelRunContext,
    node: &orchestration_runtime::compiled_plan::CompiledNode,
    action: &str,
    payload_hash: &str,
) -> String {
    format!(
        "data_model:{}:{}:{}:{}:{}:{}:{}",
        run_context.workspace_id,
        run_context.application_id,
        run_context.draft_id,
        run_context.flow_run_id,
        node.node_id,
        action,
        payload_hash
    )
}

fn data_model_side_effect_policy(
    node: &orchestration_runtime::compiled_plan::CompiledNode,
) -> Result<DataModelSideEffectPolicy> {
    match node
        .config
        .get("side_effect_policy")
        .and_then(Value::as_str)
        .unwrap_or("disabled")
    {
        "disabled" => Ok(DataModelSideEffectPolicy::Disabled),
        "confirm_each_run" => Ok(DataModelSideEffectPolicy::ConfirmEachRun),
        "allow_with_idempotency" => Ok(DataModelSideEffectPolicy::AllowWithIdempotency),
        other => Err(anyhow!(
            "unsupported data_model side_effect_policy: {other}"
        )),
    }
}

fn stable_json_hash(value: &Value) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn output_record_id(output_payload: &Value) -> Option<String> {
    output_payload
        .get("record")
        .and_then(|record| record.get("id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn receipt_payload(receipt: &domain::DataModelSideEffectReceiptRecord) -> Value {
    json!({
        "id": receipt.id,
        "idempotency_key": receipt.idempotency_key,
        "payload_hash": receipt.payload_hash,
        "status": receipt.status,
        "action": receipt.action,
        "model_code": receipt.model_code,
        "record_id": receipt.record_id,
        "deleted_id": receipt.deleted_id,
        "affected_count": receipt.affected_count,
    })
}

#[derive(Clone)]
pub(super) struct WorkflowDataModelRuntime<R> {
    repository: R,
    runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
}

impl<R> WorkflowDataModelRuntime<R>
where
    R: ModelDefinitionRepository + Clone,
{
    pub(super) fn new(
        repository: R,
        runtime_engine: Arc<runtime_core::runtime_engine::RuntimeEngine>,
    ) -> Self {
        Self {
            repository,
            runtime_engine,
        }
    }

    pub(super) async fn list(
        &self,
        actor: domain::ActorContext,
        model_code: String,
        query: Value,
    ) -> Result<Value> {
        let options = ListOptions::from_value(query)?;
        if let Some(metadata) = self.runtime_model_metadata(&actor, &model_code) {
            validate_list_options(&metadata, &options)?;
        }
        let scope_grant = self
            .scope_grant(
                &actor,
                &model_code,
                runtime_core::runtime_acl::RuntimeDataAction::View,
            )
            .await?;
        let result = self
            .runtime_engine
            .list_records(runtime_core::runtime_engine::RuntimeListInput {
                actor,
                model_code,
                scope_grant,
                filter: options.filter,
                sorts: options.sorts,
                expand_relations: options.expand_relations,
                page: options.page,
                page_size: options.page_size,
            })
            .await?;

        Ok(json!({
            "records": result.items,
            "total": result.total,
        }))
    }

    pub(super) async fn get(
        &self,
        actor: domain::ActorContext,
        model_code: String,
        record_id: String,
    ) -> Result<Value> {
        let scope_grant = self
            .scope_grant(
                &actor,
                &model_code,
                runtime_core::runtime_acl::RuntimeDataAction::View,
            )
            .await?;
        let record = self
            .runtime_engine
            .get_record(runtime_core::runtime_engine::RuntimeGetInput {
                actor,
                model_code,
                record_id,
                scope_grant,
            })
            .await?
            .ok_or_else(|| anyhow!("runtime record not found"))?;

        Ok(json!({ "record": record }))
    }

    pub(super) async fn create(
        &self,
        actor: domain::ActorContext,
        model_code: String,
        payload: Value,
    ) -> Result<Value> {
        ensure_object_payload(&payload)?;
        let scope_grant = self
            .scope_grant(
                &actor,
                &model_code,
                runtime_core::runtime_acl::RuntimeDataAction::Create,
            )
            .await?;
        let record = self
            .runtime_engine
            .create_record(runtime_core::runtime_engine::RuntimeCreateInput {
                actor,
                model_code,
                payload,
                scope_grant,
            })
            .await?;

        Ok(json!({ "record": record }))
    }

    pub(super) async fn update(
        &self,
        actor: domain::ActorContext,
        model_code: String,
        record_id: String,
        payload: Value,
    ) -> Result<Value> {
        ensure_object_payload(&payload)?;
        let scope_grant = self
            .scope_grant(
                &actor,
                &model_code,
                runtime_core::runtime_acl::RuntimeDataAction::Update,
            )
            .await?;
        let record = self
            .runtime_engine
            .update_record(runtime_core::runtime_engine::RuntimeUpdateInput {
                actor,
                model_code,
                record_id,
                payload,
                scope_grant,
            })
            .await?;

        Ok(json!({ "record": record }))
    }

    pub(super) async fn delete(
        &self,
        actor: domain::ActorContext,
        model_code: String,
        record_id: String,
    ) -> Result<Value> {
        let scope_grant = self
            .scope_grant(
                &actor,
                &model_code,
                runtime_core::runtime_acl::RuntimeDataAction::Delete,
            )
            .await?;
        self.runtime_engine
            .delete_record(runtime_core::runtime_engine::RuntimeDeleteInput {
                actor,
                model_code,
                record_id: record_id.clone(),
                scope_grant,
            })
            .await?;

        Ok(json!({
            "deleted_id": record_id,
            "affected_count": 1,
        }))
    }

    fn runtime_model_metadata(
        &self,
        actor: &domain::ActorContext,
        model_code: &str,
    ) -> Option<runtime_core::model_metadata::ModelMetadata> {
        self.runtime_engine
            .registry()
            .get(
                domain::DataModelScopeKind::Workspace,
                actor.current_workspace_id,
                model_code,
            )
            .or_else(|| {
                self.runtime_engine.registry().get(
                    domain::DataModelScopeKind::System,
                    domain::SYSTEM_SCOPE_ID,
                    model_code,
                )
            })
    }

    async fn scope_grant(
        &self,
        actor: &domain::ActorContext,
        model_code: &str,
        action: runtime_core::runtime_acl::RuntimeDataAction,
    ) -> Result<Option<runtime_core::runtime_acl::RuntimeScopeGrant>> {
        let Some(model) = self
            .runtime_engine
            .registry()
            .get(
                domain::DataModelScopeKind::Workspace,
                actor.current_workspace_id,
                model_code,
            )
            .or_else(|| {
                self.runtime_engine.registry().get(
                    domain::DataModelScopeKind::System,
                    domain::SYSTEM_SCOPE_ID,
                    model_code,
                )
            })
        else {
            return Ok(None);
        };

        ModelDefinitionService::new(self.repository.clone())
            .load_runtime_scope_grant(actor, model.model_id, action)
            .await
    }
}

#[derive(Debug)]
struct ListOptions {
    filter: domain::ResourceFilterExpr,
    sorts: Vec<runtime_core::runtime_engine::RuntimeSortInput>,
    expand_relations: Vec<String>,
    page: i64,
    page_size: i64,
}

impl ListOptions {
    fn from_value(value: Value) -> Result<Self> {
        let object = match value {
            Value::Null => Map::new(),
            Value::Object(object) => object,
            _ => return Err(anyhow!("data_model list query must be object")),
        };

        let page = optional_integer(object.get("page"), "page", 1)?.max(1);
        let page_size = optional_integer(object.get("page_size"), "page_size", 20)?
            .clamp(1, WORKFLOW_LIST_PAGE_SIZE_MAX);

        Ok(Self {
            filter: parse_filters(object.get("filters"))?,
            sorts: parse_sorts(object.get("sorts"))?,
            expand_relations: parse_string_list(object.get("expand_relations"))?,
            page,
            page_size,
        })
    }
}

fn parse_filters(value: Option<&Value>) -> Result<domain::ResourceFilterExpr> {
    let Some(value) = value else {
        return Ok(domain::ResourceFilterExpr::All(vec![]));
    };
    let entries = value
        .as_array()
        .ok_or_else(|| anyhow!("data_model list filters must be array"))?;

    let filters = entries
        .iter()
        .map(|entry| {
            let object = entry
                .as_object()
                .ok_or_else(|| anyhow!("data_model list filter must be object"))?;
            let operator = required_string(object, "operator")?;
            Ok(domain::ResourceFilterExpr::Field {
                field: required_string(object, "field_code")?,
                operator: parse_workflow_filter_operator(&operator)?,
                value: object.get("value").cloned().unwrap_or(Value::Null),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(domain::ResourceFilterExpr::all(filters))
}

fn parse_sorts(
    value: Option<&Value>,
) -> Result<Vec<runtime_core::runtime_engine::RuntimeSortInput>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| anyhow!("data_model list sorts must be array"))?;

    entries
        .iter()
        .map(|entry| {
            let object = entry
                .as_object()
                .ok_or_else(|| anyhow!("data_model list sort must be object"))?;
            let direction = object
                .get("direction")
                .and_then(Value::as_str)
                .unwrap_or("asc")
                .to_ascii_lowercase();
            ensure_supported_sort_direction(&direction)?;
            Ok(runtime_core::runtime_engine::RuntimeSortInput {
                field_code: required_string(object, "field_code")?,
                direction,
            })
        })
        .collect()
}

fn parse_string_list(value: Option<&Value>) -> Result<Vec<String>> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .ok_or_else(|| anyhow!("data_model list expand_relations must be array"))?;

    entries
        .iter()
        .map(|entry| {
            entry
                .as_str()
                .map(str::to_string)
                .ok_or_else(|| anyhow!("data_model list expand_relations item must be string"))
        })
        .collect()
}

fn optional_integer(value: Option<&Value>, key: &'static str, default_value: i64) -> Result<i64> {
    match value {
        Some(value) => value
            .as_i64()
            .ok_or_else(|| anyhow!("data_model list {key} must be integer")),
        None => Ok(default_value),
    }
}

fn validate_list_options(
    metadata: &runtime_core::model_metadata::ModelMetadata,
    options: &ListOptions,
) -> Result<()> {
    validate_filter_fields(metadata, &options.filter)?;

    for sort in &options.sorts {
        if metadata.field_by_code(&sort.field_code).is_none() {
            return Err(anyhow!("undeclared sort field: {}", sort.field_code));
        }
        ensure_supported_sort_direction(&sort.direction)?;
    }

    for relation_code in &options.expand_relations {
        let field = metadata
            .field_by_code(relation_code)
            .ok_or_else(|| anyhow!("undeclared relation code: {relation_code}"))?;
        if !matches!(
            field.field_kind,
            domain::ModelFieldKind::ManyToOne | domain::ModelFieldKind::OneToMany
        ) {
            return Err(anyhow!("unsupported relation expansion"));
        }
    }

    Ok(())
}

fn parse_workflow_filter_operator(operator: &str) -> Result<domain::ResourceFilterOperator> {
    match operator {
        "eq" => Ok(domain::ResourceFilterOperator::Eq),
        "ne" => Ok(domain::ResourceFilterOperator::Ne),
        "gt" => Ok(domain::ResourceFilterOperator::Gt),
        "gte" => Ok(domain::ResourceFilterOperator::Gte),
        "lt" => Ok(domain::ResourceFilterOperator::Lt),
        "lte" => Ok(domain::ResourceFilterOperator::Lte),
        _ => Err(anyhow!("data_model list filter operator is unsupported")),
    }
}

fn validate_filter_fields(
    metadata: &runtime_core::model_metadata::ModelMetadata,
    filter: &domain::ResourceFilterExpr,
) -> Result<()> {
    match filter {
        domain::ResourceFilterExpr::All(items) | domain::ResourceFilterExpr::Any(items) => {
            for item in items {
                validate_filter_fields(metadata, item)?;
            }
        }
        domain::ResourceFilterExpr::Field { field, .. } => {
            if metadata.field_by_code(field).is_none() {
                return Err(anyhow!("undeclared field code: {}", field));
            }
        }
    }
    Ok(())
}

fn ensure_supported_sort_direction(direction: &str) -> Result<()> {
    match direction.to_ascii_lowercase().as_str() {
        "asc" | "desc" => Ok(()),
        _ => Err(anyhow!("data_model list sort direction is unsupported")),
    }
}

fn required_string(object: &Map<String, Value>, key: &'static str) -> Result<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("data_model list {key} is required"))
}

fn ensure_object_payload(payload: &Value) -> Result<()> {
    if payload.is_object() {
        Ok(())
    } else {
        Err(anyhow!("data_model payload must be object"))
    }
}

fn required_config_string(config: &Value, key: &'static str) -> Result<String> {
    config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow!("data_model config.{key} is required"))
}

fn required_record_id(config: &Value, resolved_inputs: &Map<String, Value>) -> Result<String> {
    input_or_config_value(config, resolved_inputs, "record_id")
        .and_then(|value| value.as_str().map(str::to_string))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("data_model config.record_id is required"))
}

fn input_or_config_value(
    config: &Value,
    resolved_inputs: &Map<String, Value>,
    key: &str,
) -> Option<Value> {
    resolved_inputs
        .get(key)
        .cloned()
        .or_else(|| config.get(key).cloned())
}
