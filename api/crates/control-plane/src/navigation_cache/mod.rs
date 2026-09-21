//! Ephemeral navigation inputs, never cached authorization decisions.
use std::sync::Arc;

use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::ports::{CacheStore, FrontstagePageRepository};

const TTL: time::Duration = time::Duration::minutes(1);

#[derive(Clone, Copy)]
pub enum NavigationCacheDomain {
    FrontstagePages,
    ConsoleRoutes,
}

impl NavigationCacheDomain {
    fn prefix(self, workspace_id: Uuid) -> String {
        let domain = match self {
            Self::FrontstagePages => "frontstage-pages",
            Self::ConsoleRoutes => "console-routes",
        };
        format!("navigation:{domain}:v1:{workspace_id}")
    }
}

/// Optional acceleration: cache failures fall back to the durable source.
#[derive(Clone)]
pub struct NavigationCache(pub Arc<dyn CacheStore>);

impl NavigationCache {
    async fn generation_key(
        &self,
        domain: NavigationCacheDomain,
        workspace_id: Uuid,
    ) -> Result<String> {
        let prefix = domain.prefix(workspace_id);
        let key = format!("{prefix}:generation");
        let generation = match self.0.get_json(&key).await? {
            Some(value) => value,
            None => {
                self.0
                    .set_if_absent_json(&key, serde_json::json!(Uuid::now_v7()), None)
                    .await?;
                self.0
                    .get_json(&key)
                    .await?
                    .ok_or_else(|| anyhow::anyhow!("navigation cache generation unavailable"))?
            }
        };
        let generation = generation
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("invalid navigation cache generation"))?;
        Ok(format!("{prefix}:{generation}"))
    }

    async fn load<T, F>(
        &self,
        domain: NavigationCacheDomain,
        workspace_id: Uuid,
        variant: &str,
        source: F,
    ) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: std::future::Future<Output = Result<T>>,
    {
        let key = match self.generation_key(domain, workspace_id).await {
            Ok(key) => Some(format!("{key}:{variant}")),
            Err(error) => {
                tracing::warn!(%workspace_id, %error, "navigation cache unavailable");
                None
            }
        };
        if let Some(key) = &key {
            match self.0.get_json(key).await {
                Ok(Some(value)) => match serde_json::from_value(value) {
                    Ok(value) => return Ok(value),
                    Err(error) => {
                        tracing::warn!(%workspace_id, %error, "invalid navigation cache value")
                    }
                },
                Ok(None) => {}
                Err(error) => tracing::warn!(%workspace_id, %error, "navigation cache read failed"),
            }
        }
        let value = source.await?;
        if let Some(key) = key {
            // A concurrent mutation rotates the generation. An older in-flight read can only
            // populate its old generation, which is unreachable and expires after TTL.
            let write = async {
                self.0
                    .set_json(&key, serde_json::to_value(&value)?, Some(TTL))
                    .await
            }
            .await;
            if let Err(error) = write {
                tracing::warn!(%workspace_id, %error, "navigation cache write failed");
            }
        }
        Ok(value)
    }

    pub async fn invalidate(&self, domain: NavigationCacheDomain, workspace_id: Uuid) {
        let key = format!("{}:generation", domain.prefix(workspace_id));
        if let Err(error) = self
            .0
            .set_json(&key, serde_json::json!(Uuid::now_v7()), None)
            .await
        {
            tracing::warn!(%workspace_id, %error, "navigation cache invalidation failed; TTL bounds stale data");
        }
    }

    pub async fn frontstage_pages<R: FrontstagePageRepository>(
        &self,
        repository: &R,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontstagePageRecord>> {
        self.load(
            NavigationCacheDomain::FrontstagePages,
            workspace_id,
            "pages",
            repository.list_frontstage_pages(workspace_id),
        )
        .await
    }

    pub async fn console_navigation<T, F>(
        &self,
        actor: &domain::ActorContext,
        registry_context: serde_json::Value,
        source: F,
    ) -> Result<T>
    where
        T: Serialize + DeserializeOwned,
        F: std::future::Future<Output = Result<T>>,
    {
        // Permissions are a HashSet; canonical ordering lets equivalent principals reuse a key.
        let mut permissions = actor.permissions.iter().collect::<Vec<_>>();
        permissions.sort();
        let identity = serde_json::to_vec(&(
            actor.user_id,
            actor.tenant_id,
            actor.current_workspace_id,
            &actor.effective_display_role,
            actor.is_root,
            permissions,
            registry_context,
        ))?;
        let variant = format!("routes:{:x}", Sha256::digest(identity));
        self.load(
            NavigationCacheDomain::ConsoleRoutes,
            actor.current_workspace_id,
            &variant,
            source,
        )
        .await
    }
}

#[cfg(test)]
mod _tests;
