use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgConnection;
use storage_durable::{
    model_metadata::ModelMetadata, runtime_record_repository::OrderedTreeQueryError,
};
use uuid::Uuid;

/// Query identity excludes the page size: clients may change it during traversal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Context {
    pub model_id: Uuid,
    pub table: String,
    pub scope_id: Uuid,
    pub partition_id: Uuid,
    pub query: String,
    pub node_id: Option<Uuid>,
    pub max_depth: Option<u32>,
    pub include_path: bool,
    pub prefix: Option<String>,
}

impl Context {
    pub fn new(metadata: &ModelMetadata, scope_id: Uuid, partition_id: Uuid, query: &str) -> Self {
        Self {
            model_id: metadata.model_id,
            table: metadata.physical_table_name.clone(),
            scope_id,
            partition_id,
            query: query.to_owned(),
            node_id: None,
            max_depth: None,
            include_path: false,
            prefix: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(super) struct Cursor {
    version: u8,
    context: Context,
    pub id: Uuid,
    digest: String,
}

impl Cursor {
    pub fn new(
        context: Context,
        id: Uuid,
        ranks: Vec<String>,
        ids: Vec<Uuid>,
        tree_path: String,
    ) -> Result<Self> {
        Ok(Self {
            version: 1,
            context,
            id,
            digest: key_digest(&ranks, &ids, &tree_path)?,
        })
    }

    pub fn decode(value: Option<&str>, context: &Context) -> Result<Option<Self>> {
        let Some(value) = value else { return Ok(None) };
        let value = value
            .strip_prefix("ot1.")
            .ok_or(OrderedTreeQueryError::InvalidCursor)?;
        let bytes = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| OrderedTreeQueryError::InvalidCursor)?;
        let cursor: Self =
            serde_json::from_slice(&bytes).map_err(|_| OrderedTreeQueryError::InvalidCursor)?;
        if cursor.version != 1
            || &cursor.context != context
            || URL_SAFE_NO_PAD
                .decode(&cursor.digest)
                .map_or(true, |digest| digest.len() != 32)
        {
            return Err(OrderedTreeQueryError::InvalidCursor.into());
        }
        Ok(Some(cursor))
    }

    pub fn encode(&self) -> Result<String> {
        let bytes = serde_json::to_vec(self)?;
        Ok(format!("ot1.{}", URL_SAFE_NO_PAD.encode(bytes)))
    }

    /// The CTE contains the exact membership filters used for fetching the page.
    /// Both queries run in the caller's repeatable-read snapshot.
    pub async fn validate(&self, conn: &mut PgConnection, cte: &str) -> Result<AnchorKey> {
        let row: Option<(Vec<String>, Vec<Uuid>, String)> = sqlx::query_as(&format!(
            "{cte} select rank_path, id_path, tree_path::text from candidates where id = $6"
        ))
        .bind(self.context.scope_id)
        .bind(self.context.partition_id)
        .bind(self.context.node_id)
        .bind(self.context.max_depth.map(i64::from))
        .bind(
            self.context
                .prefix
                .as_ref()
                .map(|prefix| format!("{}%", super::queries::escape_like_prefix(prefix))),
        )
        .bind(self.id)
        .fetch_optional(conn)
        .await?;
        let Some((ranks, ids, tree_path)) = row else {
            return Err(OrderedTreeQueryError::StaleCursor.into());
        };
        if key_digest(&ranks, &ids, &tree_path)? != self.digest {
            return Err(OrderedTreeQueryError::StaleCursor.into());
        }
        Ok(AnchorKey { ranks, ids })
    }
}

pub(super) struct AnchorKey {
    pub ranks: Vec<String>,
    pub ids: Vec<Uuid>,
}

fn key_digest(ranks: &[String], ids: &[Uuid], tree_path: &str) -> Result<String> {
    let bytes = serde_json::to_vec(&(ranks, ids, tree_path))?;
    Ok(URL_SAFE_NO_PAD.encode(Sha256::digest(bytes)))
}
