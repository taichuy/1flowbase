use anyhow::Result;
use uuid::Uuid;

use crate::repositories::PgControlPlaneStore;

impl PgControlPlaneStore {
    /// The selected directly assigned role remains the session / credential identity.
    /// Department grants add permissions, never selectable roles or root authority.
    /// Resolve from durable relations on every authorization read so removal revokes
    /// only that source, without copying or deleting direct role bindings.
    pub(crate) async fn effective_role_ids(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
        active_role_code: &str,
    ) -> Result<Vec<Uuid>> {
        Ok(sqlx::query_scalar(
            r#"
            select r.id from roles r
            where exists (
                select 1 from user_role_bindings active_binding
                join roles active_role on active_role.id = active_binding.role_id
                where active_binding.user_id = $1 and active_role.code = $3
                  and (active_role.scope_kind = 'system' or active_role.workspace_id = $2)
            ) and (
                (r.code = $3 and (r.scope_kind = 'system' or r.workspace_id = $2)
                 and exists (select 1 from user_role_bindings direct_binding
                             where direct_binding.user_id = $1 and direct_binding.role_id = r.id))
                or
                (r.scope_kind = 'workspace' and r.workspace_id = $2 and r.code <> 'root' and r.system_kind is null
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
            order by r.id
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .bind(active_role_code)
        .fetch_all(self.pool())
        .await?)
    }
}
