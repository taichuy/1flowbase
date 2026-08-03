use super::*;

pub(super) fn data_policy_scope_from_db(value: String) -> domain::RoleDataPolicyScope {
    domain::RoleDataPolicyScope::from_db(&value)
}

pub(super) fn optional_data_policy_scope_from_db(
    value: Option<String>,
) -> Option<domain::RoleDataPolicyScope> {
    value.map(|scope| domain::RoleDataPolicyScope::from_db(&scope))
}

pub(super) fn default_role_data_policy() -> RoleDataPolicyDefaultsInput {
    RoleDataPolicyDefaultsInput {
        can_view: false,
        can_create: false,
        can_update: false,
        can_delete: false,
        default_view_scope: domain::RoleDataPolicyScope::Own,
        default_update_scope: domain::RoleDataPolicyScope::Own,
        default_delete_scope: domain::RoleDataPolicyScope::Own,
    }
}

pub(super) async fn insert_default_role_data_policy(
    tx: &mut Transaction<'_, Postgres>,
    role_id: Uuid,
) -> Result<()> {
    let policy = default_role_data_policy();
    sqlx::query(
        r#"
        insert into role_data_policies (
            id, role_id, can_view, can_create, can_update, can_delete,
            default_view_scope, default_update_scope, default_delete_scope
        )
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        on conflict (role_id) do nothing
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(role_id)
    .bind(policy.can_view)
    .bind(policy.can_create)
    .bind(policy.can_update)
    .bind(policy.can_delete)
    .bind(policy.default_view_scope.as_str())
    .bind(policy.default_update_scope.as_str())
    .bind(policy.default_delete_scope.as_str())
    .execute(&mut **tx)
    .await?;
    Ok(())
}
