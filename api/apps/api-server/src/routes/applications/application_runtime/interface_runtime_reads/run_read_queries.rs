use super::*;

impl ApplicationRuntimeReadsAdapter {
    pub(super) async fn list_runs(
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
                    invocation_count: log_summary.invocation_count,
                    compaction_count: log_summary.compaction_count,
                    count_tokens_input_tokens: log_summary.count_tokens_input_tokens,
                    total_tokens: log_summary.total_tokens,
                    input_tokens: log_summary.input_tokens,
                    output_tokens: log_summary.output_tokens,
                    input_cache_hit_tokens: log_summary.input_cache_hit_tokens,
                    input_cache_hit_rate: application_logs::input_cache_hit_rate_for_response(
                        log_summary.input_tokens,
                        log_summary.input_cache_hit_tokens,
                    ),
                    unique_node_count: log_summary.unique_node_count,
                    tool_callback_count: log_summary.tool_callback_count,
                };
                let mut response =
                    to_flow_run_summary_response(&application, log_summary.run, statistics);
                response.principal.display_name = log_summary.api_key_name_snapshot;
                response.parent_run_id = log_summary.parent_run_id.map(|id| id.to_string());
                response.caused_by_run_id = log_summary.caused_by_run_id.map(|id| id.to_string());
                response.log_conversation_id =
                    log_summary.log_conversation_id.map(|id| id.to_string());
                response.log_task_run_id = log_summary.log_task_run_id.map(|id| id.to_string());
                response.total_cost = log_summary.total_cost;
                response.call_kind = log_summary.call_kind;
                response.member_run_ids = log_summary
                    .member_run_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect();
                response.parent_task_run_id =
                    log_summary.parent_task_run_id.map(|id| id.to_string());
                response.outcome = log_summary.outcome;
                response.user_input = log_summary.user_input;
                response.final_output = log_summary.final_output;
                response.final_output_run_id =
                    log_summary.final_output_run_id.map(|id| id.to_string());
                response.requested_model_id = log_summary.requested_model_id;
                response.reasoning_effort = log_summary.reasoning_effort;
                response
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

    pub(super) async fn conversation_messages(
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
        let mut items = Vec::new();
        for run in page.items {
            // Each page slot is one business turn; its effective context travels
            // with it, including when the caller prepends an older page.
            let projection = <_ as OrchestrationRuntimeRepository>::list_application_run_conversation_message_items_page(
                &self.store, application_id, run.id,
                ListApplicationRunConversationMessageItemsPageInput {
                    before_sequence: None, after_sequence: None, limit: 1,
                },
            ).await?;
            if projection.total_count > 0 {
                let mut turn = conversation_messages_from_projection_page(run.id, projection);
                let started_at = format_time(run.started_at);
                for item in &mut turn.items {
                    item.started_at = started_at.clone();
                    item.sequence = None;
                }
                items.extend(turn.items);
            } else {
                items.push(to_application_conversation_message_summary_response(
                    run,
                    query.around_run_id,
                ));
            }
        }
        Ok(ApplicationConversationMessagesPageResponse {
            items,
            output_state: None,
            page: ApplicationConversationMessagesPageInfoResponse {
                has_before: page.has_before,
                has_after: page.has_after,
                before_cursor: page.before_cursor.map(|value| value.to_string()),
                after_cursor: page.after_cursor.map(|value| value.to_string()),
                newest_cursor: page.after_cursor.map(|value| value.to_string()),
            },
        })
    }

    pub(super) async fn run_conversation_messages(
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

    pub(super) async fn run_overview(
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
        let statistics = to_trace_projection_statistics_response(
            self.store
                .get_application_run_trace_statistics(run_id)
                .await?,
        );
        Ok(to_application_run_overview_response(
            &application,
            overview,
            statistics,
        ))
    }

    pub(super) async fn trace_tree(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        run_id: Uuid,
    ) -> Result<ApplicationRunTraceTreeResponse, ApiError> {
        let application = self.visible_application(actor, application_id).await?;
        let status = self.trace_projection_status(application_id, run_id).await?;
        let flow_run = <_ as OrchestrationRuntimeRepository>::get_flow_run_metadata(
            &self.store,
            application_id,
            run_id,
        )
        .await?
        .ok_or(ControlPlaneError::NotFound("flow_run"))?;
        let page_size = application_run_trace_children_page_size(None);
        let page = if projection_is_succeeded(&status) {
            Some(
                <_ as OrchestrationRuntimeRepository>::list_application_run_trace_children_page(
                    &self.store,
                    ListApplicationRunTraceChildrenPageInput {
                        flow_run_id: run_id,
                        parent_trace_node_id: Uuid::nil(),
                        page_size,
                        cursor: None,
                    },
                )
                .await?,
            )
        } else {
            None
        };
        let next_cursor = page
            .as_ref()
            .and_then(|page| page.next_cursor.as_ref())
            .map(|cursor| encode_application_run_trace_children_cursor(cursor, Uuid::nil()))
            .transpose()?;
        let page_info = ApplicationRunTraceNodeChildrenPageInfoResponse {
            has_more: page.as_ref().is_some_and(|page| page.has_more),
            next_cursor,
            page_size,
        };
        let nodes = page.map(|page| page.items).unwrap_or_default();
        let statistics = to_trace_projection_statistics_response(
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_statistics(
                &self.store,
                run_id,
            )
            .await?,
        );
        Ok(ApplicationRunTraceTreeResponse {
            run: application_run_log_response_for_metadata(&application, &flow_run),
            statistics,
            flow_run: to_flow_run_metadata_response(flow_run),
            projection_status: to_trace_projection_status_response(&status),
            page_info,
            nodes: nodes
                .into_iter()
                .map(to_trace_node_summary_from_projection)
                .collect(),
        })
    }

    pub(super) async fn trace_children(
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
        let parent_trace_node_id = if query.parent_trace_node_id == "root" {
            Uuid::nil()
        } else {
            parse_trace_projection_node_id(&query.parent_trace_node_id)?
        };
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
        if !parent_trace_node_id.is_nil() {
            <_ as OrchestrationRuntimeRepository>::get_application_run_trace_node(
                &self.store,
                run_id,
                parent_trace_node_id,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("trace_node"))?;
        }
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
}
