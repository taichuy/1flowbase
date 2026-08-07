impl PgControlPlaneStore {
    async fn create_assistant_conversation(
        &self,
        input: &control_plane::application_public_api::run_service::CreateAssistantConversationInput,
    ) -> Result<control_plane::application_public_api::run_service::AssistantConversationRecord>
    {
        let row = sqlx::query(
            r#"
            insert into assistant_conversations (
                conversation_id,
                scope_id,
                application_id,
                created_by,
                seed_legacy_flow_run_id
            ) values ($1, $2, $3, $4, $5)
            returning conversation_id, scope_id, application_id, created_by, created_at, updated_at
            "#,
        )
        .bind(input.conversation_id)
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.actor_user_id)
        .bind(input.seed_legacy_flow_run_id)
        .fetch_one(self.pool())
        .await?;

        Ok(assistant_conversation_record_from_row(row))
    }

    async fn get_assistant_conversation(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Option<control_plane::application_public_api::run_service::AssistantConversationRecord>>
    {
        let row = sqlx::query(
            r#"
            select conversation_id, scope_id, application_id, created_by, created_at, updated_at
            from assistant_conversations
            where conversation_id = $1
              and scope_id = $2
              and application_id = $3
              and created_by = $4
            "#,
        )
        .bind(conversation_id)
        .bind(workspace_id)
        .bind(application_id)
        .bind(actor_user_id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(assistant_conversation_record_from_row))
    }

    async fn list_assistant_conversations(
        &self,
        input: &control_plane::application_public_api::run_service::ListAssistantConversationsInput,
    ) -> Result<control_plane::application_public_api::run_service::AssistantConversationPage> {
        let page = input.page.max(1);
        let page_size = input.page_size.clamp(1, 50);
        let offset = (page - 1) * page_size;
        let total = sqlx::query_scalar::<_, i64>(
            r#"
            select
                (
                    select count(*)::bigint
                    from assistant_conversations conversations
                    where conversations.scope_id = $1
                      and conversations.application_id = $2
                      and conversations.created_by = $3
                )
                +
                (
                    select count(*)::bigint
                    from flow_runs runs
                    join applications on applications.id = runs.application_id
                    where applications.workspace_id = $1
                      and runs.application_id = $2
                      and runs.created_by = $3
                      and runs.run_mode = 'assistant_execution'
                      and runs.compatibility_mode = 'embedded_assistant'
                      and runs.assistant_conversation_id is null
                )
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;
        let rows = sqlx::query(
            r#"
            with conversation_items as (
                select
                    conversations.conversation_id,
                    null::uuid as legacy_flow_run_id,
                    latest.id as latest_flow_run_id,
                    nullif(btrim(coalesce(latest.title, seed.title)), '') as title,
                    conversations.created_at,
                    greatest(
                        conversations.updated_at,
                        coalesce(latest.updated_at, conversations.updated_at)
                    ) as updated_at
                from assistant_conversations conversations
                left join flow_runs seed
                  on seed.id = conversations.seed_legacy_flow_run_id
                left join lateral (
                    select id, title, updated_at
                    from flow_runs runs
                    where runs.assistant_conversation_id = conversations.conversation_id
                      and runs.run_mode = 'assistant_execution'
                      and runs.compatibility_mode = 'embedded_assistant'
                    order by runs.updated_at desc, runs.id desc
                    limit 1
                ) latest on true
                where conversations.scope_id = $1
                  and conversations.application_id = $2
                  and conversations.created_by = $3
            ), legacy_items as (
                select
                    null::uuid as conversation_id,
                    runs.id as legacy_flow_run_id,
                    runs.id as latest_flow_run_id,
                    nullif(btrim(runs.title), '') as title,
                    runs.created_at,
                    runs.updated_at
                from flow_runs runs
                join applications on applications.id = runs.application_id
                where applications.workspace_id = $1
                  and runs.application_id = $2
                  and runs.created_by = $3
                  and runs.run_mode = 'assistant_execution'
                  and runs.compatibility_mode = 'embedded_assistant'
                  and runs.assistant_conversation_id is null
            )
            select *
            from (
                select * from conversation_items
                union all
                select * from legacy_items
            ) items
            order by updated_at desc, coalesce(conversation_id, legacy_flow_run_id) desc
            limit $4 offset $5
            "#,
        )
        .bind(input.workspace_id)
        .bind(input.application_id)
        .bind(input.actor_user_id)
        .bind(page_size)
        .bind(offset)
        .fetch_all(self.pool())
        .await?;

        let items = rows
            .into_iter()
            .map(|row| {
                Ok(
                    control_plane::application_public_api::run_service::AssistantConversationSummary {
                        conversation_id: row.try_get("conversation_id")?,
                        legacy_flow_run_id: row.try_get("legacy_flow_run_id")?,
                        latest_flow_run_id: row.try_get("latest_flow_run_id")?,
                        title: row.try_get("title")?,
                        created_at: row.try_get("created_at")?,
                        updated_at: row.try_get("updated_at")?,
                    },
                )
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(
            control_plane::application_public_api::run_service::AssistantConversationPage {
                items,
                total,
                page,
                page_size,
            },
        )
    }

    async fn list_assistant_conversation_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        conversation_id: Uuid,
    ) -> Result<Vec<control_plane::application_public_api::run_service::AssistantConversationMessage>>
    {
        let rows = sqlx::query(
            r#"
            with visible_runs as (
                select seed.id, 0 as source_order
                from assistant_conversations conversations
                join flow_runs seed
                  on seed.id = conversations.seed_legacy_flow_run_id
                join applications on applications.id = seed.application_id
                where conversations.conversation_id = $1
                  and conversations.scope_id = $2
                  and conversations.application_id = $3
                  and conversations.created_by = $4
                  and applications.workspace_id = $2
                  and seed.application_id = $3
                  and seed.created_by = $4
                  and seed.run_mode = 'assistant_execution'
                  and seed.compatibility_mode = 'embedded_assistant'
                  and seed.assistant_conversation_id is null
                union all
                select runs.id, 1 as source_order
                from flow_runs runs
                join assistant_conversations conversations
                  on conversations.conversation_id = runs.assistant_conversation_id
                where conversations.conversation_id = $1
                  and conversations.scope_id = $2
                  and conversations.application_id = $3
                  and conversations.created_by = $4
                  and runs.run_mode = 'assistant_execution'
                  and runs.compatibility_mode = 'embedded_assistant'
            ), message_rows as (
                select
                    items.flow_run_id,
                    items.query,
                    items.answer,
                    items.started_at,
                    items.updated_at,
                    visible_runs.source_order
                from application_run_conversation_message_items items
                join visible_runs on visible_runs.id = items.flow_run_id
                where items.is_current
            )
            select *
            from (
                select
                    flow_run_id::text || ':user' as id,
                    flow_run_id,
                    'user'::text as role,
                    query as content,
                    coalesce(started_at, updated_at) as created_at,
                    0 as message_order,
                    source_order
                from message_rows
                where nullif(btrim(query), '') is not null
                union all
                select
                    flow_run_id::text || ':assistant' as id,
                    flow_run_id,
                    'assistant'::text as role,
                    answer as content,
                    coalesce(updated_at, started_at) as created_at,
                    1 as message_order,
                    source_order
                from message_rows
                where nullif(btrim(answer), '') is not null
            ) messages
            order by created_at asc, source_order asc, flow_run_id asc, message_order asc
            "#,
        )
        .bind(conversation_id)
        .bind(workspace_id)
        .bind(application_id)
        .bind(actor_user_id)
        .fetch_all(self.pool())
        .await?;

        assistant_conversation_messages_from_rows(rows)
    }

    async fn list_assistant_legacy_snapshot_messages(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        flow_run_id: Uuid,
    ) -> Result<Vec<control_plane::application_public_api::run_service::AssistantConversationMessage>>
    {
        let rows = sqlx::query(
            r#"
            with visible_run as (
                select runs.id
                from flow_runs runs
                join applications on applications.id = runs.application_id
                where runs.id = $1
                  and applications.workspace_id = $2
                  and runs.application_id = $3
                  and runs.created_by = $4
                  and runs.run_mode = 'assistant_execution'
                  and runs.compatibility_mode = 'embedded_assistant'
                  and runs.assistant_conversation_id is null
            ), message_row as (
                select
                    items.flow_run_id,
                    items.query,
                    items.answer,
                    items.started_at,
                    items.updated_at
                from application_run_conversation_message_items items
                join visible_run on visible_run.id = items.flow_run_id
                where items.is_current
            )
            select *
            from (
                select
                    flow_run_id::text || ':user' as id,
                    flow_run_id,
                    'user'::text as role,
                    query as content,
                    coalesce(started_at, updated_at) as created_at,
                    0 as message_order
                from message_row
                where nullif(btrim(query), '') is not null
                union all
                select
                    flow_run_id::text || ':assistant' as id,
                    flow_run_id,
                    'assistant'::text as role,
                    answer as content,
                    coalesce(updated_at, started_at) as created_at,
                    1 as message_order
                from message_row
                where nullif(btrim(answer), '') is not null
            ) messages
            order by created_at asc, flow_run_id asc, message_order asc
            "#,
        )
        .bind(flow_run_id)
        .bind(workspace_id)
        .bind(application_id)
        .bind(actor_user_id)
        .fetch_all(self.pool())
        .await?;

        assistant_conversation_messages_from_rows(rows)
    }
}

fn assistant_conversation_record_from_row(
    row: sqlx::postgres::PgRow,
) -> control_plane::application_public_api::run_service::AssistantConversationRecord {
    control_plane::application_public_api::run_service::AssistantConversationRecord {
        conversation_id: row.get("conversation_id"),
        workspace_id: row.get("scope_id"),
        application_id: row.get("application_id"),
        created_by: row.get("created_by"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn assistant_conversation_messages_from_rows(
    rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Vec<control_plane::application_public_api::run_service::AssistantConversationMessage>> {
    rows.into_iter()
        .map(|row| {
            Ok(
                control_plane::application_public_api::run_service::AssistantConversationMessage {
                    id: row.try_get("id")?,
                    flow_run_id: row.try_get("flow_run_id")?,
                    role: row.try_get("role")?,
                    content: row.try_get("content")?,
                    created_at: row.try_get("created_at")?,
                },
            )
        })
        .collect()
}
