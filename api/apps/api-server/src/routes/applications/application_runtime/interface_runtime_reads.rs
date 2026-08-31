use std::sync::Arc;

use control_plane::{
    application::ApplicationService,
    errors::ControlPlaneError,
    orchestration_runtime::trace_projection::{
        build_application_run_trace_projection, projection_status_needs_lazy_rebuild,
        APPLICATION_RUN_TRACE_PROJECTION_VERSION,
    },
    ports::{
        ApplicationRunTraceProjectionStatistics, CacheStore,
        GetApplicationRunMonitoringReportInput, ListApplicationConversationRunsPageInput,
        ListApplicationRunConversationMessageItemsPageInput,
        ListApplicationRunTraceChildrenPageInput, ListApplicationRunsPageInput,
        OrchestrationRuntimeRepository,
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use time::OffsetDateTime;
use uuid::Uuid;

use super::*;
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    runtime_activity::ApplicationRuntimeActivityTracker,
};

pub(crate) enum ApplicationRuntimeReadsInput {
    ListRuns {
        application_id: Uuid,
        query: ApplicationRunsQuery,
    },
    ListConversationMessages {
        application_id: Uuid,
        conversation_id: String,
        query: ApplicationConversationMessagesQuery,
    },
    ListRunConversationMessages {
        application_id: Uuid,
        run_id: Uuid,
        query: ApplicationConversationMessagesQuery,
    },
    GetRunOverview {
        application_id: Uuid,
        run_id: Uuid,
    },
    GetTraceTree {
        application_id: Uuid,
        run_id: Uuid,
    },
    GetTraceChildren {
        application_id: Uuid,
        run_id: Uuid,
        query: ApplicationRunTraceNodeChildrenQuery,
    },
    GetResumeTimeline {
        application_id: Uuid,
        run_id: Uuid,
    },
    GetResumeTimelineSummary {
        application_id: Uuid,
        run_id: Uuid,
    },
    GetRunNodeLastRun {
        application_id: Uuid,
        run_id: Uuid,
        node_id: String,
    },
    GetMonitoringReport {
        application_id: Uuid,
        query: application_monitoring::ApplicationRunMonitoringQuery,
    },
    GetRuntimeActivity {
        application_id: Uuid,
    },
    GetRuntimeDebugStream {
        application_id: Uuid,
        run_id: Uuid,
        query: RuntimeDebugStreamQuery,
    },
    GetNodeLastRun {
        application_id: Uuid,
        node_id: String,
    },
}

impl InterfaceContract for ApplicationRuntimeReadsInput {
    const CONTRACT_ID: &'static str = "console-application-runtime-reads-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ApplicationRuntimeReadsOutput {
    Runs(FlowRunSummaryPageResponse),
    ConversationMessages(ApplicationConversationMessagesPageResponse),
    RunOverview(ApplicationRunOverviewResponse),
    TraceTree(ApplicationRunTraceTreeResponse),
    TraceChildren(ApplicationRunTraceNodeChildrenResponse),
    ResumeTimeline(ApplicationRunResumeTimelineResponse),
    ResumeTimelineSummary(ApplicationRunResumeTimelineSummaryResponse),
    RunNodeLastRun(Option<NodeLastRunResponse>),
    MonitoringReport(application_monitoring::ApplicationRunMonitoringReportResponse),
    RuntimeActivity(crate::runtime_activity::ApplicationRuntimeActivitySnapshot),
    RuntimeDebugStream(RuntimeDebugStreamResponse),
    NodeLastRun(Option<NodeLastRunResponse>),
}

impl InterfaceContract for ApplicationRuntimeReadsOutput {
    const CONTRACT_ID: &'static str = "console-application-runtime-reads-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationRuntimeReadsAdapter {
    store: MainDurableStore,
    cache: Arc<dyn CacheStore>,
    runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    process_started_at: OffsetDateTime,
}

pub(crate) fn runtime_reads_port(
    store: MainDurableStore,
    cache: Arc<dyn CacheStore>,
    runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    process_started_at: OffsetDateTime,
) -> Arc<dyn ConsoleInterfacePort<ApplicationRuntimeReadsInput, ApplicationRuntimeReadsOutput>> {
    Arc::new(ApplicationRuntimeReadsAdapter {
        store,
        cache,
        runtime_activity,
        process_started_at,
    })
}

impl ApplicationRuntimeReadsAdapter {
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

    async fn list_runs(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        query: ApplicationRunsQuery,
    ) -> Result<FlowRunSummaryPageResponse, ApiError> {
        let application = self.visible_application(actor, application_id).await?;
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
        let created_after = application_runs_created_after(&query);
        let sort_by = normalize_application_run_sort_by(query.sort_by.as_deref()).to_string();
        let sort_order =
            normalize_application_run_sort_order(query.sort_order.as_deref()).to_string();
        let refresh_cache = should_refresh_application_run_logs(query.cache_mode.as_deref());
        let cache_key = application_log_cache::summary_page_cache_key(
            actor.current_workspace_id,
            application_id,
            &query,
            page,
            page_size,
            &sort_by,
            &sort_order,
        );
        if !refresh_cache {
            if let Some(cached) = application_log_cache::read::<FlowRunSummaryPageResponse>(
                self.cache.as_ref(),
                &cache_key,
            )
            .await
            {
                return Ok(cached);
            }
        }
        let runs_page = <_ as OrchestrationRuntimeRepository>::list_application_run_logs_page(
            &self.store,
            application_id,
            ListApplicationRunsPageInput {
                page,
                page_size,
                created_after,
                sort_by: Some(sort_by),
                sort_order: Some(sort_order),
            },
        )
        .await?;
        let items = runs_page
            .items
            .into_iter()
            .map(|log_summary| {
                let statistics = application_logs::ApplicationRunStatisticsResponse {
                    count_tokens_input_tokens: log_summary.count_tokens_input_tokens,
                    total_tokens: log_summary.total_tokens,
                    input_tokens: log_summary.input_tokens,
                    output_tokens: log_summary.output_tokens,
                    input_cache_hit_tokens: log_summary.input_cache_hit_tokens,
                    input_cache_hit_rate: application_logs::input_cache_hit_rate_for_response(
                        log_summary.total_tokens,
                        log_summary.input_cache_hit_tokens,
                    ),
                    unique_node_count: log_summary.unique_node_count,
                    tool_callback_count: log_summary.tool_callback_count,
                };
                to_flow_run_summary_response(&application, log_summary.run, statistics)
            })
            .collect();
        let response = FlowRunSummaryPageResponse {
            items,
            total: runs_page.total,
            page: runs_page.page,
            page_size: runs_page.page_size,
        };
        if application_log_cache::summary_page_cacheable(&response) {
            application_log_cache::write(
                self.cache.as_ref(),
                &cache_key,
                &response,
                application_log_cache::summary_page_cache_ttl(page),
            )
            .await;
        }
        Ok(response)
    }

    async fn conversation_messages(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        conversation_id: String,
        query: ApplicationConversationMessagesQuery,
    ) -> Result<ApplicationConversationMessagesPageResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let page = <_ as OrchestrationRuntimeRepository>::list_application_conversation_runs_page(
            &self.store,
            application_id,
            ListApplicationConversationRunsPageInput {
                external_conversation_id: conversation_id,
                around_run_id: query.around_run_id,
                before_run_id: parse_optional_uuid_cursor(query.before.as_deref()),
                after_run_id: parse_optional_uuid_cursor(query.after.as_deref()),
                limit: query.limit.unwrap_or(5),
            },
        )
        .await?;
        let items = page
            .items
            .into_iter()
            .map(|run| {
                to_application_conversation_message_summary_response(run, query.around_run_id)
            })
            .collect();
        Ok(ApplicationConversationMessagesPageResponse {
            items,
            page: ApplicationConversationMessagesPageInfoResponse {
                has_before: page.has_before,
                has_after: page.has_after,
                before_cursor: page.before_cursor.map(|value| value.to_string()),
                after_cursor: page.after_cursor.map(|value| value.to_string()),
            },
        })
    }

    async fn run_conversation_messages(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: ApplicationConversationMessagesQuery,
    ) -> Result<ApplicationConversationMessagesPageResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let projection_page = <_ as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
            &self.store,
            application_id,
            run_id,
            ListApplicationRunConversationMessageItemsPageInput {
                before_sequence: parse_run_conversation_message_sequence_cursor(run_id, query.before.as_deref()),
                after_sequence: parse_run_conversation_message_sequence_cursor(run_id, query.after.as_deref()),
                limit: query.limit.unwrap_or(5),
            },
        )
        .await?;
        if projection_page.total_count > 0 {
            return Ok(conversation_messages_from_projection_page(
                run_id,
                projection_page,
            ));
        }
        let current_item =
            <_ as OrchestrationRuntimeRepository>::get_application_run_conversation_current_item(
                &self.store,
                application_id,
                run_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        Ok(conversation_messages_from_current_item(
            run_id,
            current_item,
        ))
    }

    async fn run_overview(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRunOverviewResponse, ApiError> {
        let application = self.visible_application(actor, application_id).await?;
        let overview = <_ as OrchestrationRuntimeRepository>::get_application_run_overview(
            &self.store,
            application_id,
            run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        Ok(to_application_run_overview_response(&application, overview))
    }

    async fn trace_tree(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRunTraceTreeResponse, ApiError> {
        let application = self.visible_application(actor, application_id).await?;
        let status = self.trace_projection_status(application_id, run_id).await?;
        let flow_run = <_ as OrchestrationRuntimeRepository>::get_flow_run(
            &self.store,
            application_id,
            run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        let nodes = if projection_is_succeeded(&status) {
            <_ as OrchestrationRuntimeRepository>::list_application_run_trace_roots(
                &self.store,
                run_id,
            )
            .await?
        } else {
            Vec::new()
        };
        let statistics = if projection_is_succeeded(&status) {
            to_trace_projection_statistics_response(
                <_ as OrchestrationRuntimeRepository>::get_application_run_trace_statistics(
                    &self.store,
                    run_id,
                )
                .await?,
            )
        } else {
            to_trace_projection_statistics_response(ApplicationRunTraceProjectionStatistics {
                total_tokens: None,
                input_tokens: None,
                output_tokens: None,
                input_cache_hit_tokens: None,
                unique_node_count: 0,
                tool_callback_count: 0,
            })
        };
        Ok(ApplicationRunTraceTreeResponse {
            run: application_run_log_response_for_trace_tree(&application, &flow_run),
            statistics,
            flow_run: to_flow_run_response(flow_run),
            answer_snapshot: None,
            projection_status: to_trace_projection_status_response(&status),
            nodes: nodes
                .into_iter()
                .map(to_trace_node_summary_from_projection)
                .collect(),
        })
    }

    async fn trace_children(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: ApplicationRunTraceNodeChildrenQuery,
    ) -> Result<ApplicationRunTraceNodeChildrenResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let status = self.trace_projection_status(application_id, run_id).await?;
        let projection_status = to_trace_projection_status_response(&status);
        let page_size = application_run_trace_children_page_size(query.page_size);
        let parent_trace_node_id = parse_trace_projection_node_id(&query.parent_trace_node_id)?;
        let cursor = parse_application_run_trace_children_cursor(
            query.cursor.as_deref(),
            parent_trace_node_id,
        )?;
        if !projection_is_succeeded(&status) {
            return Ok(ApplicationRunTraceNodeChildrenResponse {
                projection_status,
                items: Vec::new(),
                page_info: ApplicationRunTraceNodeChildrenPageInfoResponse {
                    has_more: false,
                    next_cursor: None,
                    page_size,
                },
            });
        }
        <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node(
            &self.store,
            run_id,
            parent_trace_node_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("trace_node"))?;
        let page = <_ as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
            &self.store,
            ListApplicationRunTraceChildrenPageInput {
                flow_run_id: run_id,
                parent_trace_node_id,
                page_size,
                cursor,
            },
        )
        .await?;
        let next_cursor = page
            .next_cursor
            .as_ref()
            .map(|cursor| {
                encode_application_run_trace_children_cursor(cursor, parent_trace_node_id)
            })
            .transpose()?;
        Ok(ApplicationRunTraceNodeChildrenResponse {
            projection_status,
            items: page
                .items
                .into_iter()
                .map(to_trace_node_summary_from_projection)
                .collect(),
            page_info: ApplicationRunTraceNodeChildrenPageInfoResponse {
                has_more: page.has_more,
                next_cursor,
                page_size: page.page_size,
            },
        })
    }

    async fn resume_timeline(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRunResumeTimelineResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let timeline = <_ as OrchestrationRuntimeRepository>::get_application_run_resume_timeline(
            &self.store,
            application_id,
            run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        Ok(ApplicationRunResumeTimelineResponse {
            flow_run: to_flow_run_response(timeline.flow_run),
            callback_tasks: timeline
                .callback_tasks
                .into_iter()
                .map(to_callback_task_response)
                .collect(),
            events: timeline
                .events
                .into_iter()
                .map(to_run_event_response)
                .collect(),
        })
    }

    async fn resume_timeline_summary(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRunResumeTimelineSummaryResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let timeline =
            <_ as OrchestrationRuntimeRepository>::get_application_run_resume_timeline_summary(
                &self.store,
                application_id,
                run_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        Ok(ApplicationRunResumeTimelineSummaryResponse {
            flow_run_status: timeline.flow_run_status.as_str().to_string(),
            callback_tasks: timeline
                .callback_tasks
                .into_iter()
                .map(|task| ApplicationRunResumeCallbackSummaryResponse {
                    id: task.id.to_string(),
                    callback_kind: task.callback_kind,
                    status: task.status.as_str().to_string(),
                    created_at: application_logs::format_time(task.created_at),
                    completed_at: application_logs::format_optional_time(task.completed_at),
                })
                .collect(),
            events: timeline
                .events
                .into_iter()
                .map(|event| ApplicationRunResumeEventSummaryResponse {
                    id: event.id.to_string(),
                    event_type: event.event_type,
                    description: event.description,
                    created_at: application_logs::format_time(event.created_at),
                })
                .collect(),
        })
    }

    async fn run_node_last_run(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        node_id: String,
    ) -> Result<Option<NodeLastRunResponse>, ApiError> {
        self.visible_application(actor, application_id).await?;
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
        let Some(node_run) = detail
            .node_runs
            .into_iter()
            .rev()
            .find(|candidate| candidate.node_id == node_id)
        else {
            return Ok(None);
        };
        let node_run_id = node_run.id;
        let checkpoints = detail
            .checkpoints
            .into_iter()
            .filter(|checkpoint| checkpoint.node_run_id == Some(node_run_id))
            .collect();
        let events = detail
            .events
            .into_iter()
            .filter(|event| event.node_run_id == Some(node_run_id))
            .collect();
        Ok(Some(to_node_last_run_response(domain::NodeLastRun {
            flow_run: detail.flow_run,
            node_run,
            checkpoints,
            events,
        })))
    }

    async fn monitoring_report(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        query: application_monitoring::ApplicationRunMonitoringQuery,
    ) -> Result<application_monitoring::ApplicationRunMonitoringReportResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        let started_from =
            application_monitoring::parse_optional_time(query.from.as_deref(), "from")?
                .or_else(|| application_monitoring::default_started_from(&query));
        let started_to = application_monitoring::parse_optional_time(query.to.as_deref(), "to")?;
        let bucket = application_monitoring::normalize_monitoring_bucket(
            query.bucket.as_deref(),
            query.time_range_days,
        );
        let report = <_ as OrchestrationRuntimeRepository>::get_application_run_monitoring_report(
            &self.store,
            application_id,
            GetApplicationRunMonitoringReportInput {
                started_from,
                started_to,
                bucket: bucket.to_string(),
                slow_run_threshold_ms: application_monitoring::SLOW_RUN_THRESHOLD_MS,
            },
        )
        .await?;
        Ok(application_monitoring::to_report_response(
            report,
            application_monitoring::ApplicationRunMonitoringMetaResponse {
                started_from: started_from.map(application_logs::format_time),
                started_to: started_to.map(application_logs::format_time),
                bucket: bucket.to_string(),
                slow_run_threshold_ms: application_monitoring::SLOW_RUN_THRESHOLD_MS,
            },
        ))
    }

    async fn runtime_debug_stream(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
        query: RuntimeDebugStreamQuery,
    ) -> Result<RuntimeDebugStreamResponse, ApiError> {
        self.visible_application(actor, application_id).await?;
        <_ as OrchestrationRuntimeRepository>::get_flow_run(&self.store, application_id, run_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("flow_run"))?;

        let page_size = runtime_debug_stream_page_size(query.limit);
        let from_sequence = query.from_sequence.unwrap_or(0).max(0);
        let mut records = <_ as OrchestrationRuntimeRepository>::list_runtime_event_backfill_page(
            &self.store,
            run_id,
            from_sequence,
            page_size + 1,
        )
        .await?;
        let has_more = records.len() > page_size;
        if has_more {
            records.truncate(page_size);
        }
        let next_sequence = records
            .last()
            .map(debug_run_stream::durable_event_stream_sequence);
        let parts = records
            .iter()
            .filter_map(|event| {
                control_plane::runtime_observability::debug_read_model::fold_event_to_debug_part(
                    run_id, event,
                )
            })
            .map(to_runtime_debug_stream_part_response)
            .collect();
        Ok(RuntimeDebugStreamResponse {
            parts,
            page_size: i64::try_from(page_size).unwrap_or(i64::MAX),
            next_sequence,
            has_more,
        })
    }

    async fn node_last_run(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        node_id: String,
    ) -> Result<Option<NodeLastRunResponse>, ApiError> {
        self.visible_application(actor, application_id).await?;
        let Some(last_run) = <_ as OrchestrationRuntimeRepository>::get_latest_node_run(
            &self.store,
            application_id,
            &node_id,
        )
        .await?
        else {
            return Ok(None);
        };
        let runtime_events = <_ as OrchestrationRuntimeRepository>::list_runtime_events(
            &self.store,
            last_run.flow_run.id,
            0,
        )
        .await?;
        Ok(Some(to_node_last_run_response(
            enrich_node_last_run_visible_internal_llm_route_traces(last_run, &runtime_events),
        )))
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationRuntimeReadsInput,
    ) -> Result<ApplicationRuntimeReadsOutput, ApiError> {
        let actor = principal.actor();
        match input {
            ApplicationRuntimeReadsInput::ListRuns {
                application_id,
                query,
            } => Ok(ApplicationRuntimeReadsOutput::Runs(
                self.list_runs(actor, application_id, query).await?,
            )),
            ApplicationRuntimeReadsInput::ListConversationMessages {
                application_id,
                conversation_id,
                query,
            } => Ok(ApplicationRuntimeReadsOutput::ConversationMessages(
                self.conversation_messages(actor, application_id, conversation_id, query)
                    .await?,
            )),
            ApplicationRuntimeReadsInput::ListRunConversationMessages {
                application_id,
                run_id,
                query,
            } => Ok(ApplicationRuntimeReadsOutput::ConversationMessages(
                self.run_conversation_messages(actor, application_id, run_id, query)
                    .await?,
            )),
            ApplicationRuntimeReadsInput::GetRunOverview {
                application_id,
                run_id,
            } => Ok(ApplicationRuntimeReadsOutput::RunOverview(
                self.run_overview(actor, application_id, run_id).await?,
            )),
            ApplicationRuntimeReadsInput::GetTraceTree {
                application_id,
                run_id,
            } => Ok(ApplicationRuntimeReadsOutput::TraceTree(
                self.trace_tree(actor, application_id, run_id).await?,
            )),
            ApplicationRuntimeReadsInput::GetTraceChildren {
                application_id,
                run_id,
                query,
            } => Ok(ApplicationRuntimeReadsOutput::TraceChildren(
                self.trace_children(actor, application_id, run_id, query)
                    .await?,
            )),
            ApplicationRuntimeReadsInput::GetResumeTimeline {
                application_id,
                run_id,
            } => Ok(ApplicationRuntimeReadsOutput::ResumeTimeline(
                self.resume_timeline(actor, application_id, run_id).await?,
            )),
            ApplicationRuntimeReadsInput::GetResumeTimelineSummary {
                application_id,
                run_id,
            } => Ok(ApplicationRuntimeReadsOutput::ResumeTimelineSummary(
                self.resume_timeline_summary(actor, application_id, run_id)
                    .await?,
            )),
            ApplicationRuntimeReadsInput::GetRunNodeLastRun {
                application_id,
                run_id,
                node_id,
            } => Ok(ApplicationRuntimeReadsOutput::RunNodeLastRun(
                self.run_node_last_run(actor, application_id, run_id, node_id)
                    .await?,
            )),
            ApplicationRuntimeReadsInput::GetMonitoringReport {
                application_id,
                query,
            } => Ok(ApplicationRuntimeReadsOutput::MonitoringReport(
                self.monitoring_report(actor, application_id, query).await?,
            )),
            ApplicationRuntimeReadsInput::GetRuntimeActivity { application_id } => {
                self.visible_application(actor, application_id).await?;
                Ok(ApplicationRuntimeReadsOutput::RuntimeActivity(
                    self.runtime_activity
                        .snapshot(application_id, self.process_started_at),
                ))
            }
            ApplicationRuntimeReadsInput::GetRuntimeDebugStream {
                application_id,
                run_id,
                query,
            } => Ok(ApplicationRuntimeReadsOutput::RuntimeDebugStream(
                self.runtime_debug_stream(actor, application_id, run_id, query)
                    .await?,
            )),
            ApplicationRuntimeReadsInput::GetNodeLastRun {
                application_id,
                node_id,
            } => Ok(ApplicationRuntimeReadsOutput::NodeLastRun(
                self.node_last_run(actor, application_id, node_id).await?,
            )),
        }
    }
}

impl ConsoleInterfacePort<ApplicationRuntimeReadsInput, ApplicationRuntimeReadsOutput>
    for ApplicationRuntimeReadsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationRuntimeReadsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeReadsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.logs.list",
        binding_id: "http.console.applications.runtime.logs.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.conversations.messages.list",
        binding_id: "http.console.applications.runtime.conversations.messages.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/conversations/:conversation_id/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.run-conversation.messages.list",
        binding_id: "http.console.applications.runtime.run-conversation.messages.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/conversation/messages",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.run.overview.get",
        binding_id: "http.console.applications.runtime.run.overview.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/overview",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.trace-tree.get",
        binding_id: "http.console.applications.runtime.trace-tree.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/trace-tree",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.trace-tree.children.get",
        binding_id: "http.console.applications.runtime.trace-tree.children.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/trace-tree/nodes",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.resume-timeline.get",
        binding_id: "http.console.applications.runtime.resume-timeline.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/resume-timeline",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.resume-timeline-summary.get",
        binding_id: "http.console.applications.runtime.resume-timeline-summary.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/resume-timeline-summary",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.run-node-last-run.get",
        binding_id: "http.console.applications.runtime.run-node-last-run.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/nodes/:node_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.monitoring.report.get",
        binding_id: "http.console.applications.runtime.monitoring.report.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/monitoring/run-metrics",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.monitoring.activity.get",
        binding_id: "http.console.applications.runtime.monitoring.activity.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/monitoring/runtime-activity",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.debug-stream.get",
        binding_id: "http.console.applications.runtime.debug-stream.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/logs/runs/:run_id/debug-stream",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.runtime.node-last-run.get",
        binding_id: "http.console.applications.runtime.node-last-run.get.v1",
        method: "GET",
        path: "/api/console/applications/:id/orchestration/nodes/:node_id/last-run",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    store: MainDurableStore,
    cache: Arc<dyn CacheStore>,
    runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    process_started_at: OffsetDateTime,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-runtime-reads",
        "graph:console-application-runtime-reads-v1",
        DECLARATIONS,
        runtime_reads_port(store, cache, runtime_activity, process_started_at),
    )
}

#[cfg(test)]
struct UnavailableApplicationRuntimeReadsPort;

#[cfg(test)]
impl ConsoleInterfacePort<ApplicationRuntimeReadsInput, ApplicationRuntimeReadsOutput>
    for UnavailableApplicationRuntimeReadsPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: ApplicationRuntimeReadsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationRuntimeReadsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("application runtime reads fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09r4_registry_freezes_application_runtime_read_bindings() {
        let registry = console_interface::compile_registry(
            "api-server.console-application-runtime-reads",
            "graph:console-application-runtime-reads-v1",
            DECLARATIONS,
            Arc::new(UnavailableApplicationRuntimeReadsPort),
        )
        .unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
