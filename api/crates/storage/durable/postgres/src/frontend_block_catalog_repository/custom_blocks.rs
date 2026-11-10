use crate::repositories::PgControlPlaneStore;
use anyhow::Result;
use async_trait::async_trait;
use control_plane_contracts::ports::custom_frontend_blocks::{
    CustomFrontendBlock, CustomFrontendBlockBody, CustomFrontendBlockRepository,
};
use sqlx::Row;
use uuid::Uuid;

fn record(row: sqlx::postgres::PgRow) -> CustomFrontendBlock {
    CustomFrontendBlock {
        id: row.get("id"),
        workspace_id: row.get("workspace_id"),
        provider_code: row.get("provider_code"),
        contribution_code: row.get("contribution_code"),
        title: row.get("title"),
        source: row.get("source"),
        language: row.get("language"),
        revision: row.get("revision"),
    }
}

#[async_trait]
impl CustomFrontendBlockRepository for PgControlPlaneStore {
    async fn list_custom_frontend_blocks(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<CustomFrontendBlock>> {
        let rows = sqlx::query("select id, workspace_id, provider_code, contribution_code, title, source, language, revision from custom_frontend_blocks where workspace_id=$1 order by created_at, id")
            .bind(workspace_id).fetch_all(self.pool()).await?;
        Ok(rows.into_iter().map(record).collect())
    }
    async fn create_custom_frontend_block(
        &self,
        workspace_id: Uuid,
        provider_code: &str,
        contribution_code: &str,
        body: &CustomFrontendBlockBody,
        actor_user_id: Uuid,
    ) -> Result<CustomFrontendBlock> {
        let row = sqlx::query("insert into custom_frontend_blocks(id,workspace_id,provider_code,contribution_code,title,source,language,created_by,updated_by) values($1,$2,$3,$4,$5,$6,$7,$8,$8) returning id,workspace_id,provider_code,contribution_code,title,source,language,revision")
            .bind(Uuid::now_v7()).bind(workspace_id).bind(provider_code).bind(contribution_code).bind(&body.title).bind(&body.source).bind(&body.language).bind(actor_user_id).fetch_one(self.pool()).await?;
        Ok(record(row))
    }
    async fn update_custom_frontend_block(
        &self,
        workspace_id: Uuid,
        id: Uuid,
        body: &CustomFrontendBlockBody,
        actor_user_id: Uuid,
    ) -> Result<Option<CustomFrontendBlock>> {
        let row = sqlx::query("update custom_frontend_blocks set title=$3,source=$4,language=$5,revision=revision+1,updated_by=$6,updated_at=now() where workspace_id=$1 and id=$2 returning id,workspace_id,provider_code,contribution_code,title,source,language,revision")
            .bind(workspace_id).bind(id).bind(&body.title).bind(&body.source).bind(&body.language).bind(actor_user_id).fetch_optional(self.pool()).await?;
        Ok(row.map(record))
    }
    async fn delete_custom_frontend_block(&self, workspace_id: Uuid, id: Uuid) -> Result<bool> {
        Ok(
            sqlx::query("delete from custom_frontend_blocks where workspace_id=$1 and id=$2")
                .bind(workspace_id)
                .bind(id)
                .execute(self.pool())
                .await?
                .rows_affected()
                > 0,
        )
    }
}
