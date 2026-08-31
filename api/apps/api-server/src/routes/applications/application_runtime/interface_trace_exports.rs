use std::{
    collections::{HashMap, HashSet},
    future::Future,
    pin::Pin,
    sync::Arc,
};

use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    orchestration_runtime::trace_projection::{
        build_application_run_trace_projection, projection_status_needs_lazy_rebuild,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION,
    },
    ports::{
        ApplicationRunTraceProjectionStatistics, FileManagementRepository,
        GetRuntimeDebugArtifactInput, ListApplicationRunTraceChildrenPageInput,
        OrchestrationRuntimeRepository,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

use super::*;
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationRuntimeTraceExportsInput {
    ExportRun {
        application_id: Uuid,
        run_id: Uuid,
    },
    ExportSelectedRuns {
        application_id: Uuid,
        run_ids: Vec<Uuid>,
    },
}

impl InterfaceContract for ApplicationRuntimeTraceExportsInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-trace-exports-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationRuntimeTraceExportDownload {
    pub(crate) content_type: &'static str,
    pub(crate) filename: String,
    pub(crate) body: Vec<u8>,
}

pub(crate) enum ApplicationRuntimeTraceExportsOutput {
    Download(ApplicationRuntimeTraceExportDownload),
}

impl InterfaceContract for ApplicationRuntimeTraceExportsOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-trace-exports-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
struct TraceExportArtifactReader {
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
}

impl TraceExportArtifactReader {
    async fn load_json(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        artifact_id: Uuid,
    ) -> Result<serde_json::Value, ApiError> {
        let artifact = <_ as OrchestrationRuntimeRepository>::get_runtime_debug_artifact(
            &self.store,
            &GetRuntimeDebugArtifactInput {
                workspace_id,
                application_id,
                artifact_id,
            },
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("runtime_debug_artifact"))?;
        let storage =
            <_ as FileManagementRepository>::get_file_storage(&self.store, artifact.storage_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("file_storage"))?;
        if !storage.enabled {
            return Err(ControlPlaneError::Conflict("file_storage_disabled").into());
        }
        let driver = self
            .file_storage_registry
            .get(&storage.driver_type)
            .ok_or(ControlPlaneError::Conflict("storage_driver_not_registered"))?;
        let object = driver
            .open_read(storage_object::OpenReadInput {
                config_json: &storage.config_json,
                object_path: &artifact.storage_ref,
            })
            .await?;

        serde_json::from_slice(&object.bytes)
            .map_err(|_| ControlPlaneError::Conflict("runtime_debug_artifact_not_json").into())
    }
}

struct ApplicationRuntimeTraceExportsAdapter {
    store: MainDurableStore,
    artifacts: TraceExportArtifactReader,
}

pub(crate) fn trace_exports_port(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
) -> Arc<
    dyn ConsoleInterfacePort<
        ApplicationRuntimeTraceExportsInput,
        ApplicationRuntimeTraceExportsOutput,
    >,
> {
    Arc::new(ApplicationRuntimeTraceExportsAdapter {
        artifacts: TraceExportArtifactReader {
            store: store.clone(),
            file_storage_registry,
        },
        store,
    })
}

impl ApplicationRuntimeTraceExportsAdapter {
    async fn visible_application(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
    ) -> Result<domain::ApplicationRecord, ApiError> {
        Ok(ApplicationService::new(self.store.for_actor(actor.clone()))
            .get_application(actor.user_id, application_id)
            .await?)
    }

    async fn trace_projection_status(
        &self,
        application_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<domain::ApplicationRunTraceProjectionStatusRecord, ApiError> {
        let status =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
                &self.store,
                flow_run_id,
                APPLICATION_RUN_TRACE_PROJECTION_VERSION,
            )
            .await?;
        if let Some(status) = status.as_ref() {
            match status.status {
                domain::ApplicationRunTraceProjectionStatus::Pending
                | domain::ApplicationRunTraceProjectionStatus::Running
                | domain::ApplicationRunTraceProjectionStatus::Failed => return Ok(status.clone()),
                domain::ApplicationRunTraceProjectionStatus::Succeeded
                | domain::ApplicationRunTraceProjectionStatus::Stale
                | domain::ApplicationRunTraceProjectionStatus::Partial => {}
            }
        }
        let source_watermark = <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source_watermark(
            &self.store,
            application_id,
            flow_run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        if !projection_status_needs_lazy_rebuild(status.as_ref(), &source_watermark) {
            return status
                .ok_or_else(|| ControlPlaneError::Conflict("trace_projection_status").into());
        }
        let source =
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_source(
                &self.store,
                application_id,
                flow_run_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        let runtime_events =
            <_ as OrchestrationRuntimeRepository>::list_runtime_events(&self.store, flow_run_id, 0)
                .await?;
        let source = enrich_application_run_detail_visible_internal_llm_route_traces(
            source,
            &runtime_events,
        );
        let projection = build_application_run_trace_projection(&source)?;
        <_ as OrchestrationRuntimeRepository>::replace_application_run_trace_projection(
            &self.store,
            &projection,
        )
        .await?;

        <_ as OrchestrationRuntimeRepository>::get_application_run_trace_projection_status(
            &self.store,
            flow_run_id,
            APPLICATION_RUN_TRACE_PROJECTION_VERSION,
        )
        .await?
        .ok_or_else(|| ControlPlaneError::Conflict("trace_projection_status").into())
    }

    async fn export_run(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRuntimeTraceExportDownload, ApiError> {
        let application = self.visible_application(actor, application_id).await?;
        let document = self
            .build_document(
                actor.current_workspace_id,
                &application,
                application_id,
                run_id,
                OffsetDateTime::now_utc(),
            )
            .await?;
        Ok(ApplicationRuntimeTraceExportDownload {
            content_type: "application/json",
            filename: application_run_export_json_filename(
                &document.title,
                document.started_at,
                run_id,
            ),
            body: serde_json::to_vec_pretty(&document.value)?,
        })
    }

    async fn export_selected_runs(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_ids: Vec<Uuid>,
    ) -> Result<ApplicationRuntimeTraceExportDownload, ApiError> {
        if run_ids.is_empty() {
            return Err(ControlPlaneError::InvalidInput("run_ids").into());
        }
        let application = self.visible_application(actor, application_id).await?;
        let exported_at = OffsetDateTime::now_utc();
        let exported_at_text = application_logs::format_time(exported_at);
        let mut documents = Vec::with_capacity(run_ids.len());
        let mut entry_paths = HashSet::new();
        let mut manifest_runs = Vec::with_capacity(run_ids.len());

        for (index, run_id) in run_ids.into_iter().enumerate() {
            let document = self
                .build_document(
                    actor.current_workspace_id,
                    &application,
                    application_id,
                    run_id,
                    exported_at,
                )
                .await?;
            let entry_path = unique_zip_entry_path(
                application_run_export_zip_entry_name(
                    index + 1,
                    document.started_at,
                    run_id,
                    &document.title,
                ),
                &mut entry_paths,
            );
            manifest_runs.push(ApplicationRunSelectedExportManifestRunResponse {
                run_id: run_id.to_string(),
                title: document.title.clone(),
                started_at: application_logs::format_time(document.started_at),
                filename: entry_path.clone(),
                export_status: document.export_status.clone(),
                export_warning_count: document.export_warning_count,
            });
            documents.push((entry_path, document));
        }

        let export_status = if manifest_runs.iter().any(|run| run.export_warning_count > 0) {
            "complete_with_warnings"
        } else {
            "complete"
        };
        let run_count = manifest_runs.len();
        let selected_run_ids = manifest_runs
            .iter()
            .map(|run| run.run_id.clone())
            .collect::<Vec<_>>();
        let manifest = ApplicationRunSelectedExportManifestResponse {
            export_version: APPLICATION_RUN_TRACE_EXPORT_VERSION,
            exported_at: exported_at_text,
            export_status: export_status.to_string(),
            application_id: application_id.to_string(),
            run_count,
            selected_run_ids,
            entries: manifest_runs,
        };
        let body = build_selected_runs_zip(manifest, documents)?;
        Ok(ApplicationRuntimeTraceExportDownload {
            content_type: "application/zip",
            filename: format!(
                "1flowbase-runs-{}-{}-{}runs.zip",
                short_run_id(application_id),
                format_export_filename_timestamp(exported_at),
                run_count
            ),
            body,
        })
    }

    async fn build_document(
        &self,
        workspace_id: Uuid,
        application: &domain::ApplicationRecord,
        application_id: Uuid,
        run_id: Uuid,
        exported_at: OffsetDateTime,
    ) -> Result<ApplicationRunTraceExportDocument, ApiError> {
        let detail = <_ as OrchestrationRuntimeRepository>::get_application_run_detail(
            &self.store,
            application_id,
            run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        let runtime_events =
            <_ as OrchestrationRuntimeRepository>::list_runtime_events(&self.store, run_id, 0)
                .await?;
        let detail = enrich_application_run_detail_visible_internal_llm_route_traces(
            detail,
            &runtime_events,
        );
        let title = detail.flow_run.title.clone();
        let started_at = detail.flow_run.started_at;
        let trace_tree = self.build_trace_tree(application, &detail.flow_run).await?;
        let detail_response = to_application_run_detail_response(application, detail);
        let response = ApplicationRunTraceExportResponse {
            export_version: APPLICATION_RUN_TRACE_EXPORT_VERSION,
            exported_at: application_logs::format_time(exported_at),
            export_status: "complete".to_string(),
            export_warnings: Vec::new(),
            run: detail_response.run,
            statistics: detail_response.statistics,
            detail: detail_response.detail,
            flow_run: detail_response.flow_run,
            answer_snapshot: detail_response.answer_snapshot,
            node_runs: detail_response.node_runs,
            checkpoints: detail_response.checkpoints,
            callback_tasks: detail_response.callback_tasks,
            events: detail_response.events,
            stitched_trace: detail_response.stitched_trace,
            trace_tree,
        };
        let mut value = serde_json::to_value(response)?;
        self.backfill_node_run_error_payloads(run_id, &mut value)
            .await?;
        let mut warnings = Vec::new();
        let mut artifact_cache = HashMap::new();
        let mut visiting_artifacts = HashSet::new();
        value = materialize_export_artifacts(MaterializeExportArtifactsInput {
            artifacts: &self.artifacts,
            workspace_id,
            application_id,
            value,
            warnings: &mut warnings,
            artifact_cache: &mut artifact_cache,
            visiting_artifacts: &mut visiting_artifacts,
            source: "$".to_string(),
        })
        .await;
        let export_status = if warnings.is_empty() {
            "complete"
        } else {
            "complete_with_warnings"
        };
        let warning_count = warnings.len();
        let object = value
            .as_object_mut()
            .ok_or(ControlPlaneError::Conflict("application_run_export"))?;
        object.insert(
            "export_status".to_string(),
            serde_json::Value::String(export_status.to_string()),
        );
        object.insert(
            "export_warnings".to_string(),
            serde_json::to_value(&warnings)?,
        );

        Ok(ApplicationRunTraceExportDocument {
            title,
            started_at,
            export_status: export_status.to_string(),
            export_warning_count: warning_count,
            value,
        })
    }

    async fn backfill_node_run_error_payloads(
        &self,
        run_id: Uuid,
        value: &mut serde_json::Value,
    ) -> Result<(), ApiError> {
        let error_payloads = archive::load_node_run_error_payloads(&self.store, run_id).await?;
        if error_payloads.is_empty() {
            return Ok(());
        }
        let Some(node_runs) = value
            .get_mut("node_runs")
            .and_then(serde_json::Value::as_array_mut)
        else {
            return Ok(());
        };

        for node_run in node_runs {
            let Some(node_run_object) = node_run.as_object_mut() else {
                continue;
            };
            let Some(node_run_id) = node_run_object
                .get("id")
                .and_then(serde_json::Value::as_str)
            else {
                continue;
            };
            if !node_run_object
                .get("error_payload")
                .is_none_or(serde_json::Value::is_null)
            {
                continue;
            }
            if let Some(error_payload) = error_payloads.get(node_run_id) {
                node_run_object.insert("error_payload".to_string(), error_payload.clone());
            }
        }

        Ok(())
    }

    async fn build_trace_tree(
        &self,
        application: &domain::ApplicationRecord,
        flow_run: &domain::FlowRunRecord,
    ) -> Result<ApplicationRunTraceExportTreeResponse, ApiError> {
        let status = self
            .trace_projection_status(application.id, flow_run.id)
            .await?;
        let projection_status = to_trace_projection_status_response(&status);
        let statistics = if projection_is_succeeded(&status) {
            to_trace_projection_statistics_response(
                <_ as OrchestrationRuntimeRepository>::get_application_run_trace_statistics(
                    &self.store,
                    flow_run.id,
                )
                .await?,
            )
        } else {
            empty_trace_projection_statistics_response()
        };
        let nodes = if projection_is_succeeded(&status) {
            let roots = <_ as OrchestrationRuntimeRepository>::list_application_run_trace_roots(
                &self.store,
                flow_run.id,
            )
            .await?;
            let mut nodes = Vec::with_capacity(roots.len());
            for root in roots {
                nodes.push(self.build_trace_node(flow_run.id, root).await?);
            }
            nodes
        } else {
            Vec::new()
        };

        Ok(ApplicationRunTraceExportTreeResponse {
            run: application_run_log_response_for_trace_tree(application, flow_run),
            statistics,
            flow_run: to_flow_run_response(flow_run.clone()),
            answer_snapshot: None,
            projection_status,
            nodes,
        })
    }

    fn build_trace_node(
        &self,
        flow_run_id: Uuid,
        node: domain::ApplicationRunTraceNodeRecord,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<ApplicationRunTraceExportNodeResponse, ApiError>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            let summary = to_trace_node_summary_from_projection(node.clone());
            let (content_kind, source_refs, detail_refs, payload) = if node.has_content {
                match <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node_content(
                    &self.store,
                    flow_run_id,
                    node.trace_node_id,
                )
                .await?
                {
                    Some(content) => {
                        let detail_refs = content
                            .payload
                            .get("detail_refs")
                            .cloned()
                            .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));
                        (
                            Some(content.content_kind),
                            content.source_refs,
                            detail_refs,
                            trace_node_content_raw_payload_response(content.payload),
                        )
                    }
                    None => (
                        None,
                        serde_json::Value::Array(Vec::new()),
                        serde_json::Value::Array(Vec::new()),
                        serde_json::json!({}),
                    ),
                }
            } else {
                (
                    None,
                    serde_json::Value::Array(Vec::new()),
                    serde_json::Value::Array(Vec::new()),
                    serde_json::json!({}),
                )
            };
            let child_records = self
                .list_trace_children(flow_run_id, node.trace_node_id)
                .await?;
            let mut children = Vec::with_capacity(child_records.len());
            for child in child_records {
                children.push(self.build_trace_node(flow_run_id, child).await?);
            }

            Ok(ApplicationRunTraceExportNodeResponse {
                trace_node_id: summary.trace_node_id,
                stable_locator: summary.stable_locator,
                parent_trace_node_id: summary.parent_trace_node_id,
                node_kind: summary.node_kind,
                flow_run_id: summary.flow_run_id,
                node_run_id: summary.node_run_id,
                callback_task_id: summary.callback_task_id,
                node_id: summary.node_id,
                node_type: summary.node_type,
                node_mode: summary.node_mode,
                node_alias: summary.node_alias,
                status: summary.status,
                started_at: summary.started_at,
                finished_at: summary.finished_at,
                duration_ms: summary.duration_ms,
                metrics_payload: summary.metrics_payload,
                has_children: summary.has_children,
                child_count: summary.child_count,
                has_content: summary.has_content,
                source_flow_run_id: summary.source_flow_run_id,
                source_trace_node_id: summary.source_trace_node_id,
                parent_callback_task_id: summary.parent_callback_task_id,
                parent_tool_call_id: summary.parent_tool_call_id,
                trace_relation_kind: summary.trace_relation_kind,
                content_kind,
                source_refs,
                detail_refs,
                payload,
                children,
            })
        })
    }

    async fn list_trace_children(
        &self,
        flow_run_id: Uuid,
        parent_trace_node_id: Uuid,
    ) -> Result<Vec<domain::ApplicationRunTraceNodeRecord>, ApiError> {
        let mut cursor = None;
        let mut items = Vec::new();

        loop {
            let page =
                <_ as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
                    &self.store,
                    ListApplicationRunTraceChildrenPageInput {
                        flow_run_id,
                        parent_trace_node_id,
                        page_size: APPLICATION_RUN_TRACE_CHILDREN_MAX_PAGE_SIZE,
                        cursor,
                    },
                )
                .await?;
            cursor = page.next_cursor;
            items.extend(page.items);
            if !page.has_more {
                return Ok(items);
            }
        }
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeTraceExportsInput,
    ) -> Result<ApplicationRuntimeTraceExportsOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ApplicationRuntimeTraceExportsInput::ExportRun {
                application_id,
                run_id,
            } => Ok(ApplicationRuntimeTraceExportsOutput::Download(
                self.export_run(actor, application_id, run_id).await?,
            )),
            ApplicationRuntimeTraceExportsInput::ExportSelectedRuns {
                application_id,
                run_ids,
            } => Ok(ApplicationRuntimeTraceExportsOutput::Download(
                self.export_selected_runs(actor, application_id, run_ids)
                    .await?,
            )),
        }
    }
}

pub(super) async fn build_application_run_trace_export_document(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    workspace_id: Uuid,
    application: &domain::ApplicationRecord,
    application_id: Uuid,
    run_id: Uuid,
    exported_at: OffsetDateTime,
) -> Result<ApplicationRunTraceExportDocument, ApiError> {
    ApplicationRuntimeTraceExportsAdapter {
        artifacts: TraceExportArtifactReader {
            store: store.clone(),
            file_storage_registry,
        },
        store,
    }
    .build_document(
        workspace_id,
        application,
        application_id,
        run_id,
        exported_at,
    )
    .await
}

impl ConsoleInterfacePort<ApplicationRuntimeTraceExportsInput, ApplicationRuntimeTraceExportsOutput>
    for ApplicationRuntimeTraceExportsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeTraceExportsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeTraceExportsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

struct MaterializeExportArtifactsInput<'a> {
    artifacts: &'a TraceExportArtifactReader,
    workspace_id: Uuid,
    application_id: Uuid,
    value: serde_json::Value,
    warnings: &'a mut Vec<ApplicationRunTraceExportWarningResponse>,
    artifact_cache: &'a mut HashMap<Uuid, serde_json::Value>,
    visiting_artifacts: &'a mut HashSet<Uuid>,
    source: String,
}

fn materialize_export_artifacts<'a>(
    input: MaterializeExportArtifactsInput<'a>,
) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send + 'a>> {
    Box::pin(async move {
        let MaterializeExportArtifactsInput {
            artifacts,
            workspace_id,
            application_id,
            value,
            warnings,
            artifact_cache,
            visiting_artifacts,
            source,
        } = input;

        if let Some(artifact_ref) = runtime_debug_artifact_ref(&value) {
            if let Some(cached) = artifact_cache.get(&artifact_ref).cloned() {
                return cached;
            }
            if !visiting_artifacts.insert(artifact_ref) {
                warnings.push(ApplicationRunTraceExportWarningResponse {
                    code: "runtime_debug_artifact_cycle_skipped".to_string(),
                    source: source.clone(),
                    message: format!(
                        "runtime debug artifact {artifact_ref} was already being materialized"
                    ),
                });
                return value;
            }
            match artifacts
                .load_json(workspace_id, application_id, artifact_ref)
                .await
            {
                Ok(full_value) => {
                    let materialized =
                        materialize_export_artifacts(MaterializeExportArtifactsInput {
                            artifacts,
                            workspace_id,
                            application_id,
                            value: full_value,
                            warnings,
                            artifact_cache,
                            visiting_artifacts,
                            source,
                        })
                        .await;
                    visiting_artifacts.remove(&artifact_ref);
                    artifact_cache.insert(artifact_ref, materialized.clone());
                    return materialized;
                }
                Err(error) => {
                    visiting_artifacts.remove(&artifact_ref);
                    warnings.push(ApplicationRunTraceExportWarningResponse {
                        code: "runtime_debug_artifact_materialize_failed".to_string(),
                        source: source.clone(),
                        message: error.0.to_string(),
                    });
                    return value;
                }
            }
        }

        match value {
            serde_json::Value::Array(items) => {
                let mut materialized = Vec::with_capacity(items.len());
                for (index, item) in items.into_iter().enumerate() {
                    materialized.push(
                        materialize_export_artifacts(MaterializeExportArtifactsInput {
                            artifacts,
                            workspace_id,
                            application_id,
                            value: item,
                            warnings,
                            artifact_cache,
                            visiting_artifacts,
                            source: format!("{source}[{index}]"),
                        })
                        .await,
                    );
                }
                serde_json::Value::Array(materialized)
            }
            serde_json::Value::Object(object) => {
                let mut materialized = serde_json::Map::with_capacity(object.len());
                for (key, item) in object {
                    let child_source = format!("{source}.{key}");
                    let child = materialize_export_artifacts(MaterializeExportArtifactsInput {
                        artifacts,
                        workspace_id,
                        application_id,
                        value: item,
                        warnings,
                        artifact_cache,
                        visiting_artifacts,
                        source: child_source,
                    })
                    .await;
                    materialized.insert(key, child);
                }
                serde_json::Value::Object(materialized)
            }
            value => value,
        }
    })
}

fn runtime_debug_artifact_ref(value: &serde_json::Value) -> Option<Uuid> {
    let object = value.as_object()?;
    if !object
        .get("__runtime_debug_artifact")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        return None;
    }

    object
        .get("artifact_ref")
        .and_then(serde_json::Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
}

fn empty_trace_projection_statistics_response() -> application_logs::ApplicationRunStatisticsResponse
{
    to_trace_projection_statistics_response(ApplicationRunTraceProjectionStatistics {
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
        input_cache_hit_tokens: None,
        unique_node_count: 0,
        tool_callback_count: 0,
    })
}

fn build_selected_runs_zip(
    manifest: ApplicationRunSelectedExportManifestResponse,
    documents: Vec<(String, ApplicationRunTraceExportDocument)>,
) -> Result<Vec<u8>, ApiError> {
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    writer.start_file("manifest.json", options)?;
    std::io::Write::write_all(&mut writer, &serde_json::to_vec_pretty(&manifest)?)?;
    for (entry_path, document) in documents {
        writer.start_file(entry_path, options)?;
        std::io::Write::write_all(&mut writer, &serde_json::to_vec_pretty(&document.value)?)?;
    }

    Ok(writer.finish()?.into_inner())
}

fn application_run_export_json_filename(
    title: &str,
    started_at: OffsetDateTime,
    run_id: Uuid,
) -> String {
    format!(
        "1flowbase-run-{}-{}-{}.json",
        safe_filename_segment(title),
        format_export_filename_timestamp(started_at),
        short_run_id(run_id),
    )
}

fn application_run_export_zip_entry_name(
    index: usize,
    started_at: OffsetDateTime,
    run_id: Uuid,
    title: &str,
) -> String {
    format!(
        "runs/{index:03}_{}_{}_{}.json",
        format_export_filename_timestamp(started_at),
        short_run_id(run_id),
        safe_filename_segment(title),
    )
}

fn unique_zip_entry_path(path: String, used: &mut HashSet<String>) -> String {
    if used.insert(path.clone()) {
        return path;
    }

    let stem = path.strip_suffix(".json").unwrap_or(&path);
    for suffix in 2.. {
        let candidate = format!("{stem}-{suffix}.json");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }

    unreachable!("unbounded suffix loop must return a unique zip entry path")
}

fn format_export_filename_timestamp(value: OffsetDateTime) -> String {
    value
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
        .unwrap_or_else(|_| value.to_string())
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect()
}

fn short_run_id(value: Uuid) -> String {
    value.to_string().chars().take(8).collect()
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.trace-export.get",
        binding_id: "http.console.applications.runtime.trace-export.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/export",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.trace-export.selected-runs",
        binding_id: "http.console.applications.runtime.trace-export.selected-runs.v1",
        method: "POST",
        path: "/api/console/applications/:id/logs/runs/export",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-trace-exports",
        "graph:console-application-runtime-trace-exports-v1",
        DECLARATIONS,
        trace_exports_port(store, file_storage_registry),
    )
}
