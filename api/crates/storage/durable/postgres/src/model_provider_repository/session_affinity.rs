use super::*;

pub(super) async fn get_binding(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    scope_digest: &str,
) -> Result<Option<Uuid>> {
    Ok(sqlx::query_scalar(
        "select provider_instance_id from model_provider_session_bindings \
         where workspace_id = $1 and scope_digest = $2",
    )
    .bind(workspace_id)
    .bind(scope_digest)
    .fetch_optional(pool)
    .await?)
}

pub(super) async fn bind_session(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
    scope_digest: &str,
    provider_instance_id: Uuid,
) -> Result<Uuid> {
    // ON CONFLICT waits for a concurrent first writer and returns that writer's
    // owner in the same statement. Never replace it with the proposed target.
    Ok(sqlx::query_scalar(
        r#"
        insert into model_provider_session_bindings
            (workspace_id, scope_digest, provider_instance_id)
        values ($1, $2, $3)
        on conflict (workspace_id, scope_digest) do update
            set provider_instance_id = model_provider_session_bindings.provider_instance_id
        returning provider_instance_id
        "#,
    )
    .bind(workspace_id)
    .bind(scope_digest)
    .bind(provider_instance_id)
    .fetch_one(pool)
    .await?)
}
