/// Persist and retain the body before acquiring the mutable session head lock.
/// Failed publication and superseded observations remain available in this archive.
async fn archive_internal_compaction_content(
    store: &PgControlPlaneStore,
    scope_id: Uuid,
    application_id: Uuid,
    session_key: &str,
    content_kind: &str,
    content: &Value,
) -> Result<Uuid> {
    let mut tx = store.pool().begin().await?;
    let (content_id, _, _) =
        put_canonical_runtime_content_in_transaction(&mut tx, scope_id, application_id, content)
            .await?;
    sqlx::query(
        r#"
        insert into runtime_internal_compaction_contents
            (scope_id, application_id, session_key, content_id, content_kind)
        values ($1, $2, $3, $4, $5)
        on conflict do nothing
        "#,
    )
    .bind(scope_id)
    .bind(application_id)
    .bind(session_key)
    .bind(content_id)
    .bind(content_kind)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(content_id)
}

impl PgControlPlaneStore {
    async fn observe_internal_compaction(
        &self,
        input: &ObserveInternalCompactionInput,
    ) -> Result<InternalCompactionHead> {
        let content_id = archive_internal_compaction_content(
            self,
            input.scope_id,
            input.application_id,
            &input.session_key,
            "source",
            &input.source,
        )
        .await?;

        // Only database work happens under this lock. Canonical serialization and
        // provider work belong before it, and checkpoint body reads happen after it.
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"
            insert into runtime_internal_compaction_heads
                (scope_id, application_id, session_key, source_content_id, source_hashes, prefix_hash)
            values ($1, $2, $3, $4, $5, $6)
            on conflict (scope_id, application_id, session_key) do nothing
            "#,
        )
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(&input.session_key)
        .bind(content_id)
        .bind(&input.source_hashes)
        .bind(&input.prefix_hash)
        .execute(&mut *tx)
        .await?;
        let (source_hashes, prefix_hash, checkpoint_id): (Vec<String>, String, Option<Uuid>) =
            sqlx::query_as(
                r#"
                select source_hashes, prefix_hash, checkpoint_content_id
                  from runtime_internal_compaction_heads
                 where scope_id = $1 and application_id = $2 and session_key = $3
                 for update
                "#,
            )
            .bind(input.scope_id)
            .bind(input.application_id)
            .bind(&input.session_key)
            .fetch_one(&mut *tx)
            .await?;
        let older_prefix = prefix_hash == input.prefix_hash
            && input.source_hashes.len() < source_hashes.len()
            && source_hashes.starts_with(&input.source_hashes);
        if !older_prefix {
            sqlx::query(
                r#"
                update runtime_internal_compaction_heads
                   set source_content_id = $4, source_hashes = $5, prefix_hash = $6,
                       updated_at = now()
                 where scope_id = $1 and application_id = $2 and session_key = $3
                "#,
            )
            .bind(input.scope_id)
            .bind(input.application_id)
            .bind(&input.session_key)
            .bind(content_id)
            .bind(&input.source_hashes)
            .bind(&input.prefix_hash)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        let checkpoint = match checkpoint_id {
            Some(id) => Some(
                sqlx::query_scalar::<_, Value>(
                    r#"
                    select runtime_original_json(content, raw_json_payloads, 'content')
                      from runtime_canonical_contents
                     where id = $1 and scope_id = $2 and application_id = $3
                    "#,
                )
                .bind(id)
                .bind(input.scope_id)
                .bind(input.application_id)
                .fetch_one(self.pool())
                .await?,
            ),
            None => None,
        };
        Ok(InternalCompactionHead {
            checkpoint_id,
            checkpoint,
        })
    }

    async fn commit_internal_compaction(
        &self,
        input: &CommitInternalCompactionInput,
    ) -> Result<Option<Uuid>> {
        let content_id = archive_internal_compaction_content(
            self,
            input.scope_id,
            input.application_id,
            &input.session_key,
            "checkpoint",
            &input.checkpoint,
        )
        .await?;
        let mut tx = self.pool().begin().await?;
        // The terminal-state owner updates this same row. Lock it before the
        // checkpoint head so cancellation cannot commit between this check and flip.
        if let Some(flow_run_id) = input.flow_run_id {
            let status: Option<String> = sqlx::query_scalar(
                r#"
                select status from flow_runs
                 where scope_id = $1 and application_id = $2 and id = $3
                 for update
                "#,
            )
            .bind(input.scope_id)
            .bind(input.application_id)
            .bind(flow_run_id)
            .fetch_optional(&mut *tx)
            .await?;
            let status = status
                .as_deref()
                .map(crate::mappers::orchestration_runtime_mapper::parse_flow_run_status)
                .transpose()?;
            if status.is_none_or(domain::FlowRunStatus::is_terminal) {
                tx.rollback().await?;
                return Ok(None);
            }
        }
        if input.covered_source_hashes.is_empty() {
            tx.rollback().await?;
            return Ok(None);
        }
        let head: Option<(Vec<String>, String, Option<Uuid>)> = sqlx::query_as(
            r#"
            select source_hashes, prefix_hash, checkpoint_content_id
              from runtime_internal_compaction_heads
             where scope_id = $1 and application_id = $2 and session_key = $3
             for update
            "#,
        )
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(&input.session_key)
        .fetch_optional(&mut *tx)
        .await?;
        let matches = head.is_some_and(|(source_hashes, prefix_hash, checkpoint_id)| {
            checkpoint_id == input.expected_checkpoint_id
                && prefix_hash == input.prefix_hash
                && source_hashes.starts_with(&input.covered_source_hashes)
        });
        if !matches {
            tx.rollback().await?;
            return Ok(None);
        }
        // Change only the checkpoint pointer: an observed pure append remains intact.
        sqlx::query(
            r#"
            update runtime_internal_compaction_heads
               set checkpoint_content_id = $4, updated_at = now()
             where scope_id = $1 and application_id = $2 and session_key = $3
            "#,
        )
        .bind(input.scope_id)
        .bind(input.application_id)
        .bind(&input.session_key)
        .bind(content_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(content_id))
    }
}
