use crate::PgControlPlaneStore;
use async_trait::async_trait;
use control_plane_contracts::ports::mcp_oauth::{McpOAuthGrant, McpOAuthRepository};
use serde_json::Value;
use uuid::Uuid;

#[async_trait]
impl McpOAuthRepository for PgControlPlaneStore {
    async fn oauth_authorization_stamp(&self, user_id: Uuid) -> anyhow::Result<Value> {
        Ok(sqlx::query_scalar(r#"with bound as (select role_id from user_role_bindings where user_id=$1)
        select jsonb_build_object(
          'roles',(select jsonb_agg(to_jsonb(r) order by r.id) from roles r where r.id in(select role_id from bound)),
          'permissions',(select jsonb_agg(to_jsonb(p) order by p.role_id,p.permission_id) from role_permissions p where p.role_id in(select role_id from bound)),
          'console_groups',(select jsonb_agg(to_jsonb(p) order by p.id) from role_console_group_policies p where p.role_id in(select role_id from bound)),
          'console_operations',(select jsonb_agg(to_jsonb(p) order by p.group_policy_id,p.operation_id) from role_console_operation_policies p where p.role_id in(select role_id from bound)),
          'data',(select jsonb_agg(to_jsonb(p) order by p.role_id) from role_data_policies p where p.role_id in(select role_id from bound)),
          'models',(select jsonb_agg(to_jsonb(p) order by p.role_id,p.data_model_id) from role_data_model_policies p where p.role_id in(select role_id from bound)))"#)
            .bind(user_id).fetch_one(self.pool()).await?)
    }

    async fn oauth_put(
        &self,
        kind: &str,
        hash: &str,
        value: Value,
        expires: Option<i64>,
    ) -> anyhow::Result<()> {
        // Bounded opportunistic expiry keeps abandoned browser flows and token families finite.
        sqlx::query("delete from mcp_oauth_state where (kind,token_hash) in (select kind,token_hash from mcp_oauth_state where expires_at<=now() limit 100)")
            .execute(self.pool()).await?;
        sqlx::query("delete from mcp_oauth_grants where id in (select id from mcp_oauth_grants where expires_at<=now() limit 100)")
            .execute(self.pool()).await?;
        sqlx::query("insert into mcp_oauth_state(kind,token_hash,payload,expires_at) values($1,$2,$3,to_timestamp($4::double precision)) on conflict(kind,token_hash) do update set payload=excluded.payload,expires_at=excluded.expires_at")
            .bind(kind).bind(hash).bind(value).bind(expires.map(|value| value as f64)).execute(self.pool()).await?;
        Ok(())
    }
    async fn oauth_get(&self, kind: &str, hash: &str) -> anyhow::Result<Option<Value>> {
        Ok(sqlx::query_scalar("select payload from mcp_oauth_state where kind=$1 and token_hash=$2 and (expires_at is null or expires_at>now())")
            .bind(kind).bind(hash).fetch_optional(self.pool()).await?)
    }
    async fn oauth_consume(
        &self,
        kind: &str,
        hash: &str,
        expected: &Value,
    ) -> anyhow::Result<bool> {
        Ok(sqlx::query("delete from mcp_oauth_state where kind=$1 and token_hash=$2 and payload=$3 and expires_at>now()")
            .bind(kind).bind(hash).bind(expected).execute(self.pool()).await?.rows_affected()==1)
    }
    async fn oauth_create_grant(&self, grant: &McpOAuthGrant) -> anyhow::Result<()> {
        sqlx::query("insert into mcp_oauth_grants(id,user_id,api_key_id,payload,expires_at) values($1,$2,$3,$4,to_timestamp($5::double precision))")
            .bind(grant.id).bind(grant.user_id).bind(grant.api_key_id).bind(serde_json::to_value(grant)?).bind(grant.expires_at as f64).execute(self.pool()).await?;
        Ok(())
    }
    async fn oauth_grant(&self, id: Uuid) -> anyhow::Result<Option<McpOAuthGrant>> {
        let value: Option<Value> = sqlx::query_scalar(
            "select payload from mcp_oauth_grants where id=$1 and not revoked and expires_at>now()",
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;
        value
            .map(serde_json::from_value)
            .transpose()
            .map_err(Into::into)
    }
    async fn oauth_revoke_grant(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query("update mcp_oauth_grants set revoked=true where id=$1")
            .bind(id)
            .execute(self.pool())
            .await?;
        Ok(())
    }
    async fn oauth_refresh_grant(&self, hash: &str) -> anyhow::Result<Option<Uuid>> {
        Ok(
            sqlx::query_scalar("select grant_id from mcp_oauth_refresh_tokens where token_hash=$1")
                .bind(hash)
                .fetch_optional(self.pool())
                .await?,
        )
    }
    async fn oauth_rotate_refresh(
        &self,
        grant: Uuid,
        old: Option<&str>,
        new: &str,
        expires: i64,
    ) -> anyhow::Result<bool> {
        let mut tx = self.pool().begin().await?;
        let active: Option<bool> = sqlx::query_scalar(
            "select not revoked and expires_at>now() from mcp_oauth_grants where id=$1 for update",
        )
        .bind(grant)
        .fetch_optional(&mut *tx)
        .await?;
        if active != Some(true) {
            return Ok(false);
        }
        if let Some(old) = old {
            let count=sqlx::query("update mcp_oauth_refresh_tokens set consumed=true where token_hash=$1 and grant_id=$2 and not consumed and expires_at>now()")
                .bind(old).bind(grant).execute(&mut *tx).await?.rows_affected();
            if count != 1 {
                sqlx::query("update mcp_oauth_grants set revoked=true where id=$1")
                    .bind(grant)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(false);
            }
        }
        sqlx::query("insert into mcp_oauth_refresh_tokens(token_hash,grant_id,expires_at) values($1,$2,to_timestamp($3::double precision))")
            .bind(new).bind(grant).bind(expires as f64).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(true)
    }
}
