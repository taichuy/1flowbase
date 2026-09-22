impl PgControlPlaneStore {
    async fn application_run_monitoring_costs(
        &self,
        application_id: Uuid,
        from: Option<OffsetDateTime>,
        to: Option<OffsetDateTime>,
    ) -> Result<control_plane_contracts::ports::ApplicationRunMonitoringCosts> {
        let row = sqlx::query(&application_run_monitoring_logs_query("select sum(total_cost)::double precision as total_cost, count(total_cost)::bigint as cost_recorded_count, count(*) filter (where cost_incomplete)::bigint as cost_missing_count from monitoring_logs"))
            .bind(application_id).bind(from).bind(to).fetch_one(self.pool()).await?;
        Ok(
            control_plane_contracts::ports::ApplicationRunMonitoringCosts {
                total_cost: row.get("total_cost"),
                cost_recorded_count: row.get("cost_recorded_count"),
                cost_missing_count: row.get("cost_missing_count"),
            },
        )
    }
    async fn application_run_monitoring_models(
        &self,
        application_id: Uuid,
        from: Option<OffsetDateTime>,
        to: Option<OffsetDateTime>,
    ) -> Result<Vec<control_plane_contracts::ports::ApplicationRunMonitoringModelUsage>> {
        let rows = sqlx::query(&application_run_monitoring_logs_query("select requested_model_id, count(*)::bigint as task_count, coalesce(sum(total_tokens),0)::bigint as total_tokens, sum(total_cost)::double precision as total_cost from monitoring_logs group by requested_model_id order by task_count desc, requested_model_id nulls last"))
            .bind(application_id).bind(from).bind(to).fetch_all(self.pool()).await?;
        Ok(rows
            .into_iter()
            .map(
                |row| control_plane_contracts::ports::ApplicationRunMonitoringModelUsage {
                    requested_model_id: row.get("requested_model_id"),
                    task_count: row.get("task_count"),
                    total_tokens: row.get("total_tokens"),
                    total_cost: row.get("total_cost"),
                },
            )
            .collect())
    }
    async fn application_run_monitoring_users(
        &self,
        application_id: Uuid,
        from: Option<OffsetDateTime>,
        to: Option<OffsetDateTime>,
    ) -> Result<Vec<control_plane_contracts::ports::ApplicationRunMonitoringUserUsage>> {
        let rows = sqlx::query(&application_run_monitoring_logs_query("select user_id, name, count(*)::bigint as task_count, coalesce(sum(total_tokens),0)::bigint as total_tokens, sum(total_cost)::double precision as total_cost from monitoring_logs group by user_id, name order by task_count desc, user_id nulls last"))
            .bind(application_id).bind(from).bind(to).fetch_all(self.pool()).await?;
        Ok(rows
            .into_iter()
            .map(
                |row| control_plane_contracts::ports::ApplicationRunMonitoringUserUsage {
                    user_id: row.get("user_id"),
                    name: row.get("name"),
                    task_count: row.get("task_count"),
                    total_tokens: row.get("total_tokens"),
                    total_cost: row.get("total_cost"),
                },
            )
            .collect())
    }
}
