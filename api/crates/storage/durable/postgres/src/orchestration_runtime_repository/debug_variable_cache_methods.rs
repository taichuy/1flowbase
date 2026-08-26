impl PgControlPlaneStore {
    async fn upsert_debug_variable_cache_entry(
        &self,
        input: &UpsertDebugVariableCacheEntryInput,
    ) -> Result<DebugVariableCacheEntry> {
        let row = sqlx::query(
            r#"
            insert into debug_variable_cache_entries (
                id,
                workspace_id,
                application_id,
                flow_draft_id,
                actor_user_id,
                node_id,
                variable_key,
                value
            ) values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (
                application_id,
                flow_draft_id,
                actor_user_id,
                node_id,
                variable_key
            ) do update set
                value = excluded.value,
                updated_at = now()
            returning
                node_id,
                variable_key,
                value
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.draft_id)
        .bind(input.actor_user_id)
        .bind(&input.node_id)
        .bind(&input.variable_key)
        .bind(&input.value)
        .fetch_one(self.pool())
        .await?;

        Ok(DebugVariableCacheEntry {
            node_id: row.try_get("node_id")?,
            variable_key: row.try_get("variable_key")?,
            value: row.try_get("value")?,
        })
    }

    async fn list_debug_variable_cache_entries(
        &self,
        application_id: Uuid,
        draft_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<Vec<DebugVariableCacheEntry>> {
        let rows = sqlx::query(
            r#"
            select
                node_id,
                variable_key,
                value
            from debug_variable_cache_entries
            where application_id = $1
              and flow_draft_id = $2
              and actor_user_id = $3
            order by updated_at desc, id desc
            "#,
        )
        .bind(application_id)
        .bind(draft_id)
        .bind(actor_user_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(DebugVariableCacheEntry {
                    node_id: row.try_get("node_id")?,
                    variable_key: row.try_get("variable_key")?,
                    value: row.try_get("value")?,
                })
            })
            .collect()
    }

    async fn delete_debug_variable_cache_entries(
        &self,
        input: &DeleteDebugVariableCacheEntriesInput,
    ) -> Result<()> {
        let Some(keys) = input.keys.as_ref().filter(|keys| !keys.is_empty()) else {
            sqlx::query(
                r#"
                delete from debug_variable_cache_entries
                where application_id = $1
                  and flow_draft_id = $2
                  and actor_user_id = $3
                "#,
            )
            .bind(input.application_id)
            .bind(input.draft_id)
            .bind(input.actor_user_id)
            .execute(self.pool())
            .await?;
            return Ok(());
        };

        let node_ids = keys
            .iter()
            .map(|key| key.node_id.clone())
            .collect::<Vec<_>>();
        let variable_keys = keys
            .iter()
            .map(|key| key.variable_key.clone())
            .collect::<Vec<_>>();

        sqlx::query(
            r#"
            delete from debug_variable_cache_entries
            where application_id = $1
              and flow_draft_id = $2
              and actor_user_id = $3
              and (node_id, variable_key) in (
                select node_id, variable_key
                from unnest($4::text[], $5::text[]) as keys(node_id, variable_key)
              )
            "#,
        )
        .bind(input.application_id)
        .bind(input.draft_id)
        .bind(input.actor_user_id)
        .bind(node_ids)
        .bind(variable_keys)
        .execute(self.pool())
        .await?;

        Ok(())
    }
}
