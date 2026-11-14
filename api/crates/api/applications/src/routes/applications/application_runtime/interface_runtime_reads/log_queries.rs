use super::*;

impl ApplicationRuntimeReadsAdapter {
    pub(super) async fn query_conversation_trajectory(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        log_conversation_id: Uuid,
        query: log_query::conversation_trajectory::ConversationTrajectoryQuery,
    ) -> Result<control_plane::ports::ConversationClientTrajectoryQueryPage, ApiError> {
        self.visible_log_application(actor, application_id, "query_conversation_trajectory")
            .await?;
        let query = control_plane::ports::ConversationClientTrajectoryQuery {
            filter: control_plane::resource_crud::parse_resource_filter_expr(
                query.filter.as_ref().unwrap_or(&serde_json::json!({})),
            )?,
            cursor: query.cursor,
            limit: query.limit.unwrap_or(50),
            keyword: query.keyword,
            search_sections: query.search_sections.unwrap_or_default(),
            focus_record_id: query.focus_record_id,
            focus_step_id: query.focus_step_id,
            index_only: query.index_only.unwrap_or(false),
        };
        Ok(self
            .store
            .query_conversation_client_trajectory(application_id, log_conversation_id, &query)
            .await?)
    }

    pub(super) async fn conversation_trajectory_section(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        log_conversation_id: Uuid,
        step_id: Uuid,
        query: log_query::conversation_trajectory::ConversationTrajectorySectionQuery,
    ) -> Result<control_plane::ports::ClientTrajectorySection, ApiError> {
        self.visible_log_application(actor, application_id, "get_conversation_trajectory_section")
            .await?;
        let limit = query.limit.unwrap_or(8);
        control_plane::ports::log_query_page_lookahead(limit)?;
        Ok(self
            .store
            .read_conversation_client_trajectory_section(
                application_id,
                log_conversation_id,
                step_id,
                query.section.as_deref().unwrap_or("overview"),
                query.cursor,
                limit,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("client_trajectory_section"))?)
    }

    pub(super) async fn current_run_context(
        &self,
        actor: &domain::ActorContext,
        context: crate::routes::mcp_protocol::client_context::McpClientContext,
    ) -> Result<log_query::current_context::CurrentRunContextResponse, ApiError> {
        use log_query::current_context::{context_filter, CurrentRunContextResponse};
        let mut response = CurrentRunContextResponse::from_context(&context);
        if let Some(filter) = context_filter(&context) {
            let applications = ApplicationService::new(self.store.for_actor(actor.clone()))
                .list_readable_applications(actor.user_id, "get_current_run_context")
                .await?;
            let application_ids = applications
                .into_iter()
                .map(|application| application.id)
                .collect::<Vec<_>>();
            let query = control_plane::ports::ApplicationLogRecordsQuery {
                filter: control_plane::resource_crud::parse_resource_filter_expr(&filter)?,
                sort_field: "started_at".into(),
                descending: false,
                cursor: None,
                limit: 2,
            };
            response.resolve(
                self.store
                    .query_application_log_records(
                        actor.current_workspace_id,
                        &application_ids,
                        &query,
                    )
                    .await?,
            );
        }
        Ok(response)
    }

    pub(super) async fn query_log_conversations(
        &self,
        actor: &domain::ActorContext,
        query: log_query::LogConversationsQuery,
    ) -> Result<control_plane::ports::ApplicationLogConversationsPage, ApiError> {
        let page = query.page.unwrap_or(1);
        let page_size = query.page_size.unwrap_or(50);
        if page < 1 || page_size < 1 || (page - 1).checked_mul(page_size).is_none() {
            return Err(ControlPlaneError::InvalidInput(
                "page and page_size must be positive without offset overflow",
            )
            .into());
        }
        let filter = log_query::records_filter(&log_query::LogRecordsQuery {
            filter: query.filter,
            keyword: query.keyword,
            ..Default::default()
        })?;
        let service = ApplicationService::new(self.store.for_actor(actor.clone()));
        let application_ids = match query.application_ids {
            Some(ids) => {
                for id in &ids {
                    service
                        .get_application_for_read_operation(
                            actor.user_id,
                            *id,
                            "query_log_conversations",
                        )
                        .await?;
                }
                ids
            }
            None => service
                .list_readable_applications(actor.user_id, "query_log_conversations")
                .await?
                .into_iter()
                .map(|application| application.id)
                .collect(),
        };
        let query = control_plane::ports::ApplicationLogConversationsQuery {
            filter,
            sort_field: query.sort_field.unwrap_or_else(|| "started_at".into()),
            descending: query.descending.unwrap_or(true),
            page,
            page_size,
        };
        Ok(self
            .store
            .query_application_log_conversations(
                actor.current_workspace_id,
                &application_ids,
                &query,
            )
            .await?)
    }

    pub(super) async fn query_log_records(
        &self,
        actor: &domain::ActorContext,
        query: log_query::LogRecordsQuery,
    ) -> Result<control_plane::ports::ApplicationLogRecordsPage, ApiError> {
        let filter = log_query::records_filter(&query)?;
        let service = ApplicationService::new(self.store.for_actor(actor.clone()));
        let application_ids = match &query.application_ids {
            Some(ids) => {
                // Validate every explicit identity, even if the filter could never match it.
                for id in ids {
                    service
                        .get_application_for_read_operation(actor.user_id, *id, "query_log_records")
                        .await?;
                }
                ids.clone()
            }
            None => service
                .list_readable_applications(actor.user_id, "query_log_records")
                .await?
                .into_iter()
                .map(|application| application.id)
                .collect(),
        };
        let query = control_plane::ports::ApplicationLogRecordsQuery {
            filter,
            sort_field: query.sort_field.unwrap_or_else(|| "started_at".into()),
            descending: query.descending.unwrap_or(true),
            cursor: query.cursor,
            limit: query.limit.unwrap_or(50),
        };
        Ok(self
            .store
            .query_application_log_records(actor.current_workspace_id, &application_ids, &query)
            .await?)
    }

    pub(super) async fn query_record_trajectory(
        &self,
        actor: &domain::ActorContext,
        application_id: Uuid,
        record_id: Uuid,
        query: log_query::TrajectoryQuery,
    ) -> Result<control_plane::ports::RecordClientTrajectoryQueryPage, ApiError> {
        self.visible_log_application(actor, application_id, "query_record_trajectory")
            .await?;
        let query = control_plane::ports::RecordClientTrajectoryQuery {
            filter: control_plane::resource_crud::parse_resource_filter_expr(
                query.filter.as_ref().unwrap_or(&serde_json::json!({})),
            )?,
            cursor: query.cursor,
            limit: query.limit.unwrap_or(50),
            keyword: query.keyword,
            search_sections: query.search_sections.unwrap_or_default(),
        };
        Ok(self
            .store
            .query_record_client_trajectory(application_id, record_id, &query)
            .await?)
    }
}
