use std::collections::BTreeMap;

use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    ports::{
        FrontendBlockCatalogRepository, FrontstageDependencyLockReconciliationRepository,
        ReconcileFrontstageBlockDependencyLockInput,
    },
};

use super::source_dependencies::dependency_lock_from_source;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FrontstageDependencyLockReconciliationReceipt {
    pub candidate_block_count: u32,
    pub updated_block_count: u32,
    pub unresolved_block_count: u32,
}

pub async fn reconcile_legacy_frontstage_block_dependency_locks<R>(
    repository: &R,
    node_id: &str,
    actor_user_id: Uuid,
) -> Result<FrontstageDependencyLockReconciliationReceipt>
where
    R: FrontendBlockCatalogRepository + FrontstageDependencyLockReconciliationRepository,
{
    let candidates = repository
        .list_legacy_frontstage_block_dependency_lock_candidates()
        .await?;
    if candidates.is_empty() {
        return Ok(FrontstageDependencyLockReconciliationReceipt::default());
    }
    let candidate_block_count = u32::try_from(candidates.len())
        .map_err(|_| anyhow::anyhow!("too many legacy frontstage dependency lock candidates"))?;
    let mut unresolved_block_count = 0;
    let mut updates = Vec::with_capacity(candidates.len());
    let mut modules_by_workspace = BTreeMap::<Uuid, Vec<domain::FrontendBlockCodeModule>>::new();

    for candidate in candidates {
        let modules = match modules_by_workspace.get(&candidate.workspace_id) {
            Some(modules) => modules.clone(),
            None => {
                let modules = repository
                    .list_workspace_frontend_blocks(node_id, candidate.workspace_id)
                    .await?
                    .into_iter()
                    .flat_map(|entry| entry.code_modules)
                    .collect::<Vec<_>>();
                modules_by_workspace.insert(candidate.workspace_id, modules.clone());
                modules
            }
        };
        let dependency_lock = match dependency_lock_from_source(
            candidate.workspace_id,
            &candidate.source_code,
            modules,
        ) {
            Ok(dependency_lock) => dependency_lock,
            Err(error) => {
                unresolved_block_count += 1;
                tracing::warn!(
                    workspace_id = %candidate.workspace_id,
                    page_id = %candidate.page_id,
                    block_id = %candidate.block_id,
                    error = %error,
                    "legacy frontstage block dependency lock could not be reconciled"
                );
                continue;
            }
        };
        updates.push(ReconcileFrontstageBlockDependencyLockInput {
            workspace_id: candidate.workspace_id,
            page_id: candidate.page_id,
            code_ref: candidate.code_ref,
            dependency_lock,
            audit_log: audit_log(
                Some(candidate.workspace_id),
                Some(actor_user_id),
                "frontstage_block",
                Some(candidate.page_id),
                "frontstage.block_node_dependency_lock_reconciled",
                serde_json::json!({ "block_id": candidate.block_id }),
            ),
        });
    }

    let updated_block_count = if updates.is_empty() {
        0
    } else {
        repository
            .reconcile_frontstage_block_dependency_locks(&updates)
            .await?
    };
    Ok(FrontstageDependencyLockReconciliationReceipt {
        candidate_block_count,
        updated_block_count,
        unresolved_block_count,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use tokio::sync::Mutex;

    use super::*;
    use crate::ports::{
        FrontendBlockCatalogRepository, LegacyFrontstageBlockDependencyLockCandidate,
        ReconcileFrontstageBlockDependencyLockInput, ReplaceInstallationFrontendBlocksInput,
    };

    #[derive(Clone)]
    struct MemoryRepository {
        candidates: Vec<LegacyFrontstageBlockDependencyLockCandidate>,
        catalog_entries: Vec<domain::FrontendBlockCatalogEntry>,
        updates: Arc<Mutex<Vec<ReconcileFrontstageBlockDependencyLockInput>>>,
    }

    #[async_trait]
    impl FrontendBlockCatalogRepository for MemoryRepository {
        async fn replace_installation_frontend_blocks(
            &self,
            _input: &ReplaceInstallationFrontendBlocksInput,
        ) -> Result<()> {
            Ok(())
        }

        async fn list_workspace_frontend_blocks(
            &self,
            _node_id: &str,
            _workspace_id: Uuid,
        ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
            Ok(self.catalog_entries.clone())
        }

        async fn list_system_frontend_blocks(
            &self,
            _node_id: &str,
        ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
            Ok(self.catalog_entries.clone())
        }
    }

    #[async_trait]
    impl FrontstageDependencyLockReconciliationRepository for MemoryRepository {
        async fn list_legacy_frontstage_block_dependency_lock_candidates(
            &self,
        ) -> Result<Vec<LegacyFrontstageBlockDependencyLockCandidate>> {
            Ok(self.candidates.clone())
        }

        async fn reconcile_frontstage_block_dependency_locks(
            &self,
            updates: &[ReconcileFrontstageBlockDependencyLockInput],
        ) -> Result<u32> {
            self.updates.lock().await.extend_from_slice(updates);
            Ok(updates.len() as u32)
        }
    }

    fn code_module(
        source: &str,
        binding: domain::FrontendModuleBinding,
        exports: &[&str],
    ) -> domain::FrontendBlockCodeModule {
        domain::FrontendBlockCodeModule {
            source: source.to_owned(),
            version: "1.0.0".to_owned(),
            exports: exports.iter().map(ToString::to_string).collect(),
            binding,
            assets: match binding {
                domain::FrontendModuleBinding::Host => vec![],
                domain::FrontendModuleBinding::Fetched => {
                    vec![domain::FrontendModuleAsset {
                        path: "module.js".to_owned(),
                        role: domain::FrontendModuleAssetRole::BrowserModule,
                        media_type: "text/javascript; charset=utf-8".to_owned(),
                        sha256: "a".repeat(64),
                    }]
                }
            },
            type_declarations: String::new(),
            components: vec![],
        }
    }

    fn catalog_entry() -> domain::FrontendBlockCatalogEntry {
        domain::FrontendBlockCatalogEntry {
            installation_id: Uuid::nil(),
            provider_code: "1flowbase".to_owned(),
            plugin_id: "1flowbase".to_owned(),
            plugin_version: "1.0.0".to_owned(),
            contribution_code: "frontstage.js-ui-block".to_owned(),
            title: "Code Block".to_owned(),
            runtime: "native_react".to_owned(),
            entry: "index.js".to_owned(),
            code_template: None,
            code_template_version: None,
            code_template_language: None,
            code_modules: vec![
                code_module("react", domain::FrontendModuleBinding::Host, &["default"]),
                code_module("antd", domain::FrontendModuleBinding::Host, &["Button"]),
                code_module(
                    "@ant-design/icons",
                    domain::FrontendModuleBinding::Fetched,
                    &["ReloadOutlined"],
                ),
            ],
            context_contract: domain::FrontendBlockContextContract {
                primitives: vec![],
                input_schema: serde_json::json!({}),
            },
            permissions: domain::FrontendBlockPermissions {
                network: "none".to_owned(),
                storage: "none".to_owned(),
                secrets: "none".to_owned(),
            },
            ui_capabilities: vec![],
        }
    }

    #[tokio::test]
    async fn rebuilds_a_legacy_lock_with_host_modules_and_the_newly_catalogued_icon() {
        let workspace_id = Uuid::now_v7();
        let repository = MemoryRepository {
            candidates: vec![LegacyFrontstageBlockDependencyLockCandidate {
                workspace_id,
                page_id: Uuid::now_v7(),
                block_id: "tree".to_owned(),
                code_ref: "tree-code".to_owned(),
                source_code: "import { Button } from 'antd';\nimport { ReloadOutlined } from '@ant-design/icons';\nexport default () => <Button icon={<ReloadOutlined />} />;".to_owned(),
            }],
            catalog_entries: vec![catalog_entry()],
            updates: Arc::new(Mutex::new(vec![])),
        };

        let receipt = reconcile_legacy_frontstage_block_dependency_locks(
            &repository,
            "test-node",
            Uuid::now_v7(),
        )
        .await
        .expect("a legacy source with registered imports must be reconciled");

        assert_eq!(
            receipt,
            FrontstageDependencyLockReconciliationReceipt {
                candidate_block_count: 1,
                updated_block_count: 1,
                unresolved_block_count: 0,
            }
        );
        let updates = repository.updates.lock().await;
        assert_eq!(updates.len(), 1);
        assert_eq!(
            updates[0]
                .dependency_lock
                .as_array()
                .expect("canonical dependency lock is an array")
                .iter()
                .map(|entry| entry["module_source"].as_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["@ant-design/icons", "antd", "react"]
        );
        assert_eq!(
            updates[0].audit_log.event_code,
            "frontstage.block_node_dependency_lock_reconciled"
        );
    }
}
