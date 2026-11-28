use anyhow::Result;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

// NULL requests the selectable pool; a non-NULL code requests only that role.
// Prefer a system binding if a workspace role has the same code.
pub(super) const BOUND_ROLES_IN_WORKSPACE_SQL: &str = r#"
    select id, code from (
        select distinct on (r.code) r.id, r.code, r.scope_kind
        from roles r
        where ($3::text is null or r.code = $3) and (
            ((r.scope_kind = 'system' or (r.workspace_id = $2 and r.code <> 'root'))
             and exists (select 1 from user_role_bindings direct_binding
                         where direct_binding.user_id = $1 and direct_binding.role_id = r.id))
            or
            (r.scope_kind = 'workspace' and r.workspace_id = $2 and r.scope_id = $2
             and r.code <> 'root' and r.system_kind is null
             and exists (
                 select 1 from user_department_bindings membership
                 join departments department on department.id = membership.department_id
                     and department.scope_id = membership.scope_id
                 join department_role_bindings grant_binding
                     on grant_binding.department_id = department.id
                    and grant_binding.scope_id = department.scope_id
                 where membership.user_id = $1 and membership.scope_id = $2
                   and grant_binding.role_id = r.id
             ))
        )
        order by r.code, r.scope_kind
    ) bound
    order by scope_kind, code
    "#;

use crate::repositories::PgControlPlaneStore;

impl PgControlPlaneStore {
    /// Only the selected role contributes permissions, regardless of binding source.
    /// Read the current bindings so revocation takes effect without copying role grants.
    pub(crate) async fn effective_role_ids(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        active_role_code: &str,
    ) -> Result<Vec<Uuid>> {
        Ok(sqlx::query_scalar(BOUND_ROLES_IN_WORKSPACE_SQL)
            .bind(user_id)
            .bind(workspace_id)
            .bind(active_role_code)
            .fetch_all(self.pool())
            .await?)
    }
}

/// Same durable relation query as effective_role_ids, inside the caller's data transaction.
pub(crate) async fn effective_role_ids_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    workspace_id: Uuid,
    active_role_code: &str,
) -> Result<Vec<Uuid>> {
    Ok(sqlx::query_scalar(BOUND_ROLES_IN_WORKSPACE_SQL)
        .bind(user_id)
        .bind(workspace_id)
        .bind(active_role_code)
        .fetch_all(&mut **transaction)
        .await?)
}
