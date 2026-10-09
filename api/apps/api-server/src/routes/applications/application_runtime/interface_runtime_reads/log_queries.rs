use super::*;

impl ApplicationRuntimeReadsAdapter {
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
