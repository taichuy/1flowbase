//! Tiny source digests keyed by a durable node revision; never authorization or source.
use std::{future::Future, sync::Arc};

use anyhow::Result;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::ports::CacheStore;

const TTL: time::Duration = time::Duration::minutes(5);

#[derive(Clone)]
pub struct BlockCodeCache(pub Arc<dyn CacheStore>);

impl BlockCodeCache {
    fn key(node: &domain::FrontstageBlockNodeRecord) -> String {
        let mut identity = Sha256::new();
        identity.update(node.block_id.len().to_be_bytes());
        identity.update(node.block_id.as_bytes());
        identity.update(node.code_ref.as_bytes());
        format!(
            "frontstage:block-source-digest:v1:{}:{}:{:x}:{}",
            node.workspace_id,
            node.page_id,
            identity.finalize(),
            node.updated_at.unix_timestamp_nanos(),
        )
    }

    pub(super) async fn load<F>(
        &self,
        node: &domain::FrontstageBlockNodeRecord,
        source: F,
    ) -> Result<Option<String>>
    where
        F: Future<Output = Result<Option<String>>>,
    {
        match self.0.get_json(&Self::key(node)).await {
            Ok(Some(value)) => {
                if let Some(digest) = value.as_str().filter(|digest| valid_digest(digest)) {
                    return Ok(Some(digest.to_owned()));
                }
                tracing::warn!("invalid block source digest cache value");
            }
            Ok(None) => {}
            Err(error) => tracing::warn!(%error, "block source digest cache read failed"),
        }
        let digest = source.await?;
        if let Some(digest) = &digest {
            self.store(node, digest).await;
        }
        Ok(digest)
    }

    pub(super) async fn store(&self, node: &domain::FrontstageBlockNodeRecord, digest: &str) {
        if !valid_digest(digest) {
            return;
        }
        // A save advances node.updated_at atomically with source. Late reads fill
        // only their earlier revision key, even if cache invalidation is unavailable.
        if let Err(error) = self
            .0
            .set_json(&Self::key(node), json!(digest), Some(TTL))
            .await
        {
            tracing::warn!(%error, "block source digest cache write failed");
        }
    }
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn etag_matches(condition: &str, digest: &str) -> bool {
    let etag = format!("\"{digest}\"");
    condition.split(',').any(|candidate| {
        let candidate = candidate.trim();
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

#[cfg(test)]
#[path = "_tests/code_cache.rs"]
mod tests;
