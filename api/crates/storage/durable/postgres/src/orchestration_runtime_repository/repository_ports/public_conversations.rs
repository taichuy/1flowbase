use super::*;

#[async_trait]
impl ApplicationPublicConversationRepository for PgControlPlaneStore {
    async fn bind_application_public_conversation(
        &self,
        input: &BindApplicationPublicConversationInput,
    ) -> Result<ApplicationPublicConversationRecord> {
        sqlx::query(
            r#"
            insert into application_conversations (
                id,
                scope_id,
                application_id,
                api_key_id,
                external_user,
                external_conversation_id
            )
            select
                $1,
                applications.workspace_id,
                applications.id,
                $2,
                $3,
                $4
            from applications
            where applications.id = $5
            on conflict (application_id, api_key_id, external_user, external_conversation_id)
            do update set updated_at = now()
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .bind(input.application_id)
        .execute(self.pool())
        .await?;

        let row = sqlx::query(
            r#"
            insert into application_public_conversations (
                id,
                application_id,
                api_key_id,
                external_user,
                external_conversation_id
            ) values ($1, $2, $3, $4, $5)
            on conflict (application_id, api_key_id, external_user, external_conversation_id)
            do update set updated_at = now()
            returning
                id,
                application_id,
                api_key_id,
                external_user,
                external_conversation_id,
                created_at,
                updated_at
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.application_id)
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .fetch_one(self.pool())
        .await?;

        Ok(ApplicationPublicConversationRecord {
            id: row.get("id"),
            application_id: row.get("application_id"),
            api_key_id: row.get("api_key_id"),
            external_user: row.get("external_user"),
            external_conversation_id: row.get("external_conversation_id"),
            created_at: row.get::<OffsetDateTime, _>("created_at"),
            updated_at: row.get::<OffsetDateTime, _>("updated_at"),
        })
    }

    async fn list_application_public_conversation_messages(
        &self,
        input: &ListApplicationPublicConversationMessagesInput,
    ) -> Result<Vec<ApplicationPublicConversationMessageRecord>> {
        let limit = input.limit.clamp(1, 200);
        let hidden_internal_run_filter = hidden_anthropic_claude_code_internal_run_sql("runs");
        // SQL safety: The internal-run filter uses a fixed alias; conversation identifiers and limits remain bound.
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            with conversation_messages as (
                select
                    messages.id,
                    messages.flow_run_id,
                    messages.role,
                    messages.content,
                    messages.raw_json_payloads ->> 'content' as content_original,
                    messages.sequence,
                    messages.created_at,
                    coalesce(messages.started_at, messages.created_at) as occurred_at
                from application_conversations conversations
                join application_conversation_messages messages
                  on messages.conversation_id = conversations.id
                join flow_runs runs
                  on runs.id = messages.flow_run_id
                where conversations.application_id = $1
                  and conversations.api_key_id = $2
                  and conversations.external_user = $3
                  and conversations.external_conversation_id = $4
                  and messages.flow_run_id is not null
                  and messages.role in ('user', 'assistant')
                  and btrim(messages.content) <> ''
                  and not ({hidden_internal_run_filter})
            ),
            current_turn_boundaries as (
                select
                    flow_run_id,
                    max(sequence) filter (where role = 'user') as current_user_sequence
                from conversation_messages
                group by flow_run_id
            ),
            turn_messages as (
                select
                    messages.id,
                    messages.role,
                    messages.content,
                    messages.content_original,
                    messages.sequence,
                    messages.created_at,
                    messages.occurred_at
                from conversation_messages messages
                join current_turn_boundaries boundaries
                  on boundaries.flow_run_id = messages.flow_run_id
                where boundaries.current_user_sequence is not null
                  and (
                      (messages.role = 'user'
                       and messages.sequence = boundaries.current_user_sequence)
                      or
                      (messages.role = 'assistant'
                       and messages.sequence > boundaries.current_user_sequence)
                  )
            )
            select role, content, content_original, sequence
            from (
                select
                    id,
                    role,
                    content,
                    content_original,
                    sequence,
                    created_at,
                    occurred_at
                from turn_messages
                order by
                    occurred_at desc,
                    sequence desc,
                    created_at desc,
                    id desc
                limit $5
            ) recent
            order by occurred_at asc, sequence asc, created_at asc, id asc
            "#,
            hidden_internal_run_filter = hidden_internal_run_filter
        )))
        .bind(input.application_id)
        .bind(input.api_key_id)
        .bind(&input.external_user)
        .bind(&input.external_conversation_id)
        .bind(limit)
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                Ok(ApplicationPublicConversationMessageRecord {
                    role: row.get("role"),
                    content: original_required_text(&row, "content")?,
                    sequence: row.get("sequence"),
                })
            })
            .collect::<Result<Vec<_>>>()?)
    }
}
