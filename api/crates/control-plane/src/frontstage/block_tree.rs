use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use runtime_core::runtime_record_repository::{OrderedTreeCommandError, OrderedTreeQueryError};
use serde_json::{Map, Value};
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateFrontstageBlockNodeInput, DeleteFrontstageBlockLeafInput,
        DeleteFrontstageBlockSubtreeInput, FrontendBlockCatalogRepository,
        FrontstageBlockCodeInput, FrontstageBlockDescriptorUpdate, FrontstageBlockPosition,
        FrontstageBlockSourceInput, FrontstageBlockSubtreeDeleteResult,
        FrontstageBlockTreeRepository, FrontstagePageRepository, MoveFrontstageBlockNodeInput,
        SaveFrontstageBlockNodeCodeInput, UpdateFrontstageBlockDescriptorsInput,
        UpdateFrontstageBlockNodeInput,
    },
};

use super::{ensure_design_permission, FrontstagePageService};

const MAX_BLOCK_TITLE_LEN: usize = 200;
const DEFAULT_RENDERER_VERSION: &str = "v1";

pub struct FrontstageBlockScopeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub block_id: String,
}

pub struct ListFrontstageBlocksCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub limit: u32,
}

pub struct ListFrontstageBlockChildrenCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub limit: u32,
}

pub struct ListFrontstageBlockDescendantsCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub max_depth: u32,
    pub limit: u32,
}

pub struct SearchFrontstageBlocksCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub query: String,
    pub limit: u32,
}

pub struct CreateFrontstageBlockNodeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub presentation: domain::FrontstageBlockPresentation,
    pub position: FrontstageBlockPosition,
    pub source_code: String,
    pub input_mapping: BTreeMap<String, String>,
    pub output_mapping: BTreeMap<String, String>,
    pub runtime_descriptor: Option<Value>,
}

pub struct UpdateFrontstageBlockNodeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub title: Option<String>,
    pub description: Option<String>,
    pub presentation: Option<domain::FrontstageBlockPresentation>,
    pub input_mapping: Option<BTreeMap<String, String>>,
    pub output_mapping: Option<BTreeMap<String, String>>,
    pub runtime_descriptor: Option<Value>,
}

pub struct UpdateFrontstageBlockDescriptorsCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub updates: Vec<(String, Value)>,
}

pub struct MoveFrontstageBlockNodeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub position: FrontstageBlockPosition,
}

pub struct DeleteFrontstageBlockSubtreeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub expected_affected_count: u64,
}

pub struct SaveFrontstageBlockNodeCodeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub expected_source_revision: Option<String>,
    pub source_code: String,
}

pub struct FrontstageBlockOpenTarget {
    pub slug: String,
    pub page_id: Uuid,
    pub block_id: String,
}

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository + FrontstageBlockTreeRepository,
{
    pub async fn list_block_roots(
        &self,
        command: ListFrontstageBlocksCommand,
    ) -> Result<Vec<domain::FrontstageBlockNodeRecord>> {
        self.ensure_block_designer(
            command.actor_user_id,
            command.workspace_id,
            command.page_id,
            Some(command.tab_id),
        )
        .await?;
        self.repository
            .list_frontstage_block_roots(
                command.workspace_id,
                command.page_id,
                command.tab_id,
                command.limit,
            )
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn search_blocks(
        &self,
        command: SearchFrontstageBlocksCommand,
    ) -> Result<Vec<domain::FrontstageBlockSearchResult>> {
        self.ensure_block_designer(
            command.actor_user_id,
            command.workspace_id,
            command.page_id,
            Some(command.tab_id),
        )
        .await?;
        self.repository
            .search_frontstage_blocks(
                command.workspace_id,
                command.page_id,
                command.tab_id,
                &command.query,
                command.limit,
            )
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn get_block_node(
        &self,
        command: FrontstageBlockScopeCommand,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        let (_, node) = self.load_visible_block(&command).await?;
        Ok(node)
    }

    pub async fn get_block_runtime_assembly(
        &self,
        command: FrontstageBlockScopeCommand,
    ) -> Result<Vec<domain::frontstage::FrontstageBlockRuntimeLayer>> {
        let actor = self
            .load_actor_context(command.actor_user_id, command.workspace_id)
            .await?;
        let layers = self
            .repository
            .get_frontstage_block_runtime_assembly(
                command.workspace_id,
                command.page_id,
                &command.block_id,
            )
            .await?;
        let target = layers
            .last()
            .ok_or(ControlPlaneError::NotFound("block_node_not_found"))?;
        if target.node.block_id != command.block_id
            || layers
                .first()
                .is_some_and(|layer| layer.node.parent_block_id.is_some())
            || layers.iter().any(|layer| {
                layer.node.workspace_id != command.workspace_id
                    || layer.node.page_id != command.page_id
                    || layer.node.tab_id != target.node.tab_id
            })
            || layers.windows(2).any(|pair| {
                pair[1].node.parent_block_id.as_deref() != Some(pair[0].node.block_id.as_str())
            })
        {
            return Err(ControlPlaneError::NotFound("block_node_not_found").into());
        }
        if let Err(error) = self
            .ensure_page_tab_visible(
                &actor,
                command.actor_user_id,
                command.workspace_id,
                command.page_id,
                target.node.tab_id,
            )
            .await
        {
            if matches!(
                error.downcast_ref::<ControlPlaneError>(),
                Some(ControlPlaneError::NotFound(_))
            ) {
                return Err(ControlPlaneError::NotFound("block_node_not_found").into());
            }
            return Err(error);
        }
        Ok(layers)
    }

    pub async fn open_block(
        &self,
        command: FrontstageBlockScopeCommand,
    ) -> Result<FrontstageBlockOpenTarget> {
        let (_, node) = self.load_visible_block(&command).await?;
        let pages = self
            .repository
            .list_frontstage_pages(command.workspace_id)
            .await?;
        let pages_by_id = pages
            .iter()
            .map(|page| (page.id, page))
            .collect::<std::collections::HashMap<_, _>>();
        let mut root = pages_by_id
            .get(&command.page_id)
            .copied()
            .ok_or(ControlPlaneError::NotFound("block_node_not_found"))?;
        while let Some(parent_id) = root.parent_id {
            root = pages_by_id
                .get(&parent_id)
                .copied()
                .ok_or(ControlPlaneError::NotFound("block_node_not_found"))?;
        }
        let slug = root
            .slug
            .clone()
            .ok_or(ControlPlaneError::NotFound("block_node_not_found"))?;

        Ok(FrontstageBlockOpenTarget {
            slug,
            page_id: command.page_id,
            block_id: node.block_id,
        })
    }

    pub async fn list_block_children(
        &self,
        command: ListFrontstageBlockChildrenCommand,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        self.ensure_block_designer(
            command.scope.actor_user_id,
            command.scope.workspace_id,
            command.scope.page_id,
            None,
        )
        .await?;
        self.repository
            .list_frontstage_block_children(
                command.scope.workspace_id,
                command.scope.page_id,
                &command.scope.block_id,
                command.limit,
            )
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn list_block_ancestors(
        &self,
        command: FrontstageBlockScopeCommand,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        self.load_visible_block(&command).await?;
        self.repository
            .list_frontstage_block_ancestors(
                command.workspace_id,
                command.page_id,
                &command.block_id,
            )
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn list_block_descendants(
        &self,
        command: ListFrontstageBlockDescendantsCommand,
    ) -> Result<Vec<domain::FrontstageBlockDescendantProjection>> {
        self.ensure_block_designer(
            command.scope.actor_user_id,
            command.scope.workspace_id,
            command.scope.page_id,
            None,
        )
        .await?;
        self.repository
            .list_frontstage_block_descendants(
                command.scope.workspace_id,
                command.scope.page_id,
                &command.scope.block_id,
                command.max_depth,
                command.limit,
            )
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn get_block_delete_impact(
        &self,
        command: FrontstageBlockScopeCommand,
    ) -> Result<domain::FrontstageBlockSubtreeImpact> {
        self.ensure_block_designer(
            command.actor_user_id,
            command.workspace_id,
            command.page_id,
            None,
        )
        .await?;
        self.repository
            .get_frontstage_block_subtree_impact(
                command.workspace_id,
                command.page_id,
                &command.block_id,
            )
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn create_block_node(
        &self,
        command: CreateFrontstageBlockNodeCommand,
    ) -> Result<domain::FrontstageBlockNodeRecord>
    where
        R: FrontendBlockCatalogRepository,
    {
        let actor = self
            .ensure_block_designer(
                command.actor_user_id,
                command.workspace_id,
                command.page_id,
                None,
            )
            .await?;
        let mut parent_runtime_descriptor = None;
        let tab_id = if let Some(parent_block_id) = command.position.parent_block_id.as_deref() {
            let parent = self
                .load_block_node(&FrontstageBlockScopeCommand {
                    actor_user_id: command.actor_user_id,
                    workspace_id: command.workspace_id,
                    page_id: command.page_id,
                    block_id: parent_block_id.to_owned(),
                })
                .await?;
            parent_runtime_descriptor = Some(parent.runtime_descriptor.clone());
            if command
                .tab_id
                .is_some_and(|requested_tab_id| requested_tab_id != parent.tab_id)
            {
                return Err(ControlPlaneError::InvalidInput(
                    "frontstage_block_parent_tab_mismatch",
                )
                .into());
            }
            parent.tab_id
        } else {
            let tab_id = command.tab_id.ok_or(ControlPlaneError::InvalidInput(
                "frontstage_block_tab_id_required",
            ))?;
            let tab_exists = self
                .repository
                .list_frontstage_page_tabs(command.workspace_id, command.page_id)
                .await?
                .iter()
                .any(|tab| tab.id == tab_id);
            if !tab_exists {
                return Err(ControlPlaneError::NotFound("frontstage_page_tab").into());
            }
            tab_id
        };
        let title = required_block_title(command.title)?;
        let description = optional_block_description(command.description);
        let block_id = Uuid::now_v7().to_string();
        let code_ref = format!("frontstage.block.{block_id}");
        let runtime_descriptor = canonical_runtime_descriptor(
            &block_id,
            &code_ref,
            inherit_runtime_identity(
                command.runtime_descriptor,
                parent_runtime_descriptor.as_ref(),
            )?,
        )?;
        let (runtime_descriptor, dependency_lock) = self
            .resolve_runtime_descriptor_and_lock(command.workspace_id, runtime_descriptor)
            .await?;
        let audit_log = block_audit(
            &actor,
            command.page_id,
            &block_id,
            "frontstage.block_node_created",
        );
        self.repository
            .create_frontstage_block_node(&CreateFrontstageBlockNodeInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                tab_id,
                block_id,
                position: command.position,
                presentation: command.presentation,
                title: Some(title),
                description,
                code_ref,
                schema_version: 1,
                input_mapping: command.input_mapping,
                output_mapping: command.output_mapping,
                runtime_descriptor,
                code: validate_code(FrontstageBlockCodeInput {
                    source_code: command.source_code,
                    dependency_lock,
                })?,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn update_block_node(
        &self,
        command: UpdateFrontstageBlockNodeCommand,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        let actor = self
            .ensure_block_designer(
                command.scope.actor_user_id,
                command.scope.workspace_id,
                command.scope.page_id,
                None,
            )
            .await?;
        let existing = self.load_block_node(&command.scope).await?;
        let title = command.title.map(required_block_title).transpose()?;
        let description = command
            .description
            .map(|value| optional_block_description(Some(value)));
        let runtime_descriptor = command
            .runtime_descriptor
            .map(|descriptor| {
                canonical_runtime_descriptor(
                    &existing.block_id,
                    &existing.code_ref,
                    Some(descriptor),
                )
            })
            .transpose()?;
        let audit_log = block_audit(
            &actor,
            command.scope.page_id,
            &command.scope.block_id,
            "frontstage.block_node_updated",
        );
        self.repository
            .update_frontstage_block_node(&UpdateFrontstageBlockNodeInput {
                workspace_id: command.scope.workspace_id,
                actor_user_id: command.scope.actor_user_id,
                page_id: command.scope.page_id,
                block_id: command.scope.block_id,
                presentation: command.presentation,
                title: title.map(Some),
                description,
                input_mapping: command.input_mapping,
                output_mapping: command.output_mapping,
                runtime_descriptor,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn update_block_descriptors(
        &self,
        command: UpdateFrontstageBlockDescriptorsCommand,
    ) -> Result<Vec<domain::FrontstageBlockNodeRecord>> {
        let actor = self
            .ensure_block_designer(
                command.actor_user_id,
                command.workspace_id,
                command.page_id,
                Some(command.tab_id),
            )
            .await?;
        if command.updates.is_empty() {
            return Err(ControlPlaneError::InvalidInput("frontstage_block_descriptors").into());
        }
        let mut seen = HashSet::with_capacity(command.updates.len());
        let mut updates = Vec::with_capacity(command.updates.len());
        for (block_id, descriptor) in command.updates {
            if !seen.insert(block_id.clone()) {
                return Err(ControlPlaneError::InvalidInput(
                    "frontstage_block_descriptor_duplicate",
                )
                .into());
            }
            let scope = FrontstageBlockScopeCommand {
                actor_user_id: command.actor_user_id,
                workspace_id: command.workspace_id,
                page_id: command.page_id,
                block_id: block_id.clone(),
            };
            let existing = self.load_block_node(&scope).await?;
            if existing.tab_id != command.tab_id {
                return Err(ControlPlaneError::NotFound("block_node_not_found").into());
            }
            updates.push(FrontstageBlockDescriptorUpdate {
                block_id,
                runtime_descriptor: canonical_runtime_descriptor(
                    &existing.block_id,
                    &existing.code_ref,
                    Some(descriptor),
                )?,
            });
        }
        let audit_log = audit_log(
            Some(actor.current_workspace_id),
            Some(actor.user_id),
            "frontstage_page_tab",
            Some(command.tab_id),
            "frontstage.block_descriptors_updated",
            serde_json::json!({
                "page_id": command.page_id,
                "block_ids": updates.iter().map(|item| &item.block_id).collect::<Vec<_>>()
            }),
        );
        self.repository
            .update_frontstage_block_descriptors(&UpdateFrontstageBlockDescriptorsInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                tab_id: command.tab_id,
                updates,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn move_block_node(
        &self,
        command: MoveFrontstageBlockNodeCommand,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        let actor = self
            .ensure_block_designer(
                command.scope.actor_user_id,
                command.scope.workspace_id,
                command.scope.page_id,
                None,
            )
            .await?;
        let audit_log = block_audit(
            &actor,
            command.scope.page_id,
            &command.scope.block_id,
            "frontstage.block_node_moved",
        );
        self.repository
            .move_frontstage_block_node(&MoveFrontstageBlockNodeInput {
                workspace_id: command.scope.workspace_id,
                actor_user_id: command.scope.actor_user_id,
                page_id: command.scope.page_id,
                block_id: command.scope.block_id,
                position: command.position,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn delete_block_leaf(&self, command: FrontstageBlockScopeCommand) -> Result<()> {
        let actor = self
            .ensure_block_designer(
                command.actor_user_id,
                command.workspace_id,
                command.page_id,
                None,
            )
            .await?;
        self.load_block_node(&command).await?;
        let audit_log = block_audit(
            &actor,
            command.page_id,
            &command.block_id,
            "frontstage.block_node_deleted",
        );
        let deleted = self
            .repository
            .delete_frontstage_block_leaf(&DeleteFrontstageBlockLeafInput {
                workspace_id: command.workspace_id,
                page_id: command.page_id,
                block_id: command.block_id,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)?;
        if deleted {
            Ok(())
        } else {
            Err(ControlPlaneError::NotFound("block_node_not_found").into())
        }
    }

    pub async fn delete_block_subtree(
        &self,
        command: DeleteFrontstageBlockSubtreeCommand,
    ) -> Result<FrontstageBlockSubtreeDeleteResult> {
        let actor = self
            .ensure_block_designer(
                command.scope.actor_user_id,
                command.scope.workspace_id,
                command.scope.page_id,
                None,
            )
            .await?;
        let audit_log = block_audit(
            &actor,
            command.scope.page_id,
            &command.scope.block_id,
            "frontstage.block_subtree_deleted",
        );
        self.repository
            .delete_frontstage_block_subtree(&DeleteFrontstageBlockSubtreeInput {
                workspace_id: command.scope.workspace_id,
                page_id: command.scope.page_id,
                block_id: command.scope.block_id,
                expected_affected_count: command.expected_affected_count,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    pub async fn get_block_node_code(
        &self,
        command: FrontstageBlockScopeCommand,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        let (_, node) = self.load_visible_block(&command).await?;
        self.repository
            .get_frontstage_block_code(command.workspace_id, command.page_id, &node.code_ref)
            .await?
            .ok_or(ControlPlaneError::NotFound("block_node_not_found").into())
    }

    pub async fn save_block_node_code(
        &self,
        command: SaveFrontstageBlockNodeCodeCommand,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        let actor = self
            .ensure_block_designer(
                command.scope.actor_user_id,
                command.scope.workspace_id,
                command.scope.page_id,
                None,
            )
            .await?;
        self.load_block_node(&command.scope).await?;
        let audit_log = block_audit(
            &actor,
            command.scope.page_id,
            &command.scope.block_id,
            "frontstage.block_node_code_saved",
        );
        self.repository
            .save_frontstage_block_node_code(&SaveFrontstageBlockNodeCodeInput {
                workspace_id: command.scope.workspace_id,
                actor_user_id: command.scope.actor_user_id,
                page_id: command.scope.page_id,
                block_id: command.scope.block_id,
                expected_source_revision: validate_source_revision(
                    command.expected_source_revision,
                )?,
                source: FrontstageBlockSourceInput {
                    source_code: command.source_code,
                },
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    async fn resolve_runtime_descriptor_and_lock(
        &self,
        workspace_id: Uuid,
        mut runtime_descriptor: Value,
    ) -> Result<(Value, Value)>
    where
        R: FrontendBlockCatalogRepository,
    {
        if runtime_descriptor
            .pointer("/runtime/kind")
            .and_then(Value::as_str)
            != Some("native_react")
        {
            return Ok((runtime_descriptor, Value::Array(Vec::new())));
        }

        let requested_identity = FrontstageCatalogIdentity::from_descriptor(&runtime_descriptor)?;
        let node_id = self
            .node_id
            .as_deref()
            .ok_or(ControlPlaneError::UpstreamUnavailable(
                "frontstage_catalog_node",
            ))?;
        let entries = self
            .repository
            .list_workspace_frontend_blocks(node_id, workspace_id)
            .await?;
        let entry = match requested_identity {
            Some(identity) => entries.into_iter().find(|entry| identity.matches(entry)),
            None => {
                let mut defaults = entries.into_iter().filter(|entry| {
                    entry.provider_code == "1flowbase"
                        && entry.contribution_code == "frontstage.js-ui-block"
                });
                let entry = defaults.next();
                if defaults.next().is_some() {
                    return Err(ControlPlaneError::InvalidInput(
                        "frontstage_block_catalog_identity",
                    )
                    .into());
                }
                entry
            }
        }
        .ok_or(ControlPlaneError::NotFound(
            "frontstage_block_catalog_entry",
        ))?;
        apply_catalog_identity(&mut runtime_descriptor, &entry)?;
        let dependency_lock = canonical_dependency_lock(workspace_id, entry.code_modules)?;
        Ok((runtime_descriptor, dependency_lock))
    }

    async fn ensure_block_designer(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Option<Uuid>,
    ) -> Result<domain::ActorContext> {
        let actor = self.load_actor_context(actor_user_id, workspace_id).await?;
        ensure_design_permission(&actor)?;
        self.ensure_existing_page(workspace_id, page_id).await?;
        if let Some(tab_id) = tab_id {
            let tab_exists = self
                .repository
                .list_frontstage_page_tabs(workspace_id, page_id)
                .await?
                .iter()
                .any(|tab| tab.id == tab_id);
            if !tab_exists {
                return Err(ControlPlaneError::NotFound("frontstage_page_tab").into());
            }
        }
        Ok(actor)
    }

    async fn load_block_node(
        &self,
        command: &FrontstageBlockScopeCommand,
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        self.repository
            .get_frontstage_block_node(command.workspace_id, command.page_id, &command.block_id)
            .await
            .map_err(map_block_repository_error)?
            .ok_or(ControlPlaneError::NotFound("block_node_not_found").into())
    }

    async fn load_visible_block(
        &self,
        command: &FrontstageBlockScopeCommand,
    ) -> Result<(domain::ActorContext, domain::FrontstageBlockNodeRecord)> {
        let actor = self
            .load_actor_context(command.actor_user_id, command.workspace_id)
            .await?;
        let node = self.load_block_node(command).await?;
        if let Err(error) = self
            .ensure_page_tab_visible(
                &actor,
                command.actor_user_id,
                command.workspace_id,
                command.page_id,
                node.tab_id,
            )
            .await
        {
            if matches!(
                error.downcast_ref::<ControlPlaneError>(),
                Some(ControlPlaneError::NotFound(_))
            ) {
                return Err(ControlPlaneError::NotFound("block_node_not_found").into());
            }
            return Err(error);
        }
        Ok((actor, node))
    }
}

pub(super) fn validate_code(code: FrontstageBlockCodeInput) -> Result<FrontstageBlockCodeInput> {
    if !is_dependency_lock(&code.dependency_lock) {
        return Err(ControlPlaneError::InvalidInput("dependency_lock").into());
    }
    Ok(code)
}

pub(super) fn validate_source_revision(value: Option<String>) -> Result<Option<String>> {
    match value {
        Some(value)
            if value.len() == 64
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) =>
        {
            Ok(Some(value))
        }
        Some(_) => Err(ControlPlaneError::InvalidInput("expected_source_revision").into()),
        None => Ok(None),
    }
}

fn is_dependency_lock(value: &Value) -> bool {
    let Some(entries) = value.as_array() else {
        return false;
    };
    let mut module_sources = HashSet::new();
    entries.iter().all(|entry| {
        let Some(entry) = entry.as_object() else {
            return false;
        };
        let Some(module_source) = non_empty_string(entry.get("module_source")) else {
            return false;
        };
        let Some(_module_version) = non_empty_string(entry.get("module_version")) else {
            return false;
        };
        let Some(binding) = entry.get("binding").and_then(Value::as_str) else {
            return false;
        };
        let host_binding = matches!(module_source, "react" | "react/jsx-runtime" | "antd");
        if !module_sources.insert(module_source)
            || !matches!(binding, "host" | "fetched")
            || (binding == "host") != host_binding
        {
            return false;
        }

        let Some(exports) = entry.get("exports").and_then(Value::as_array) else {
            return false;
        };
        let mut export_names = HashSet::new();
        if exports.is_empty()
            || !exports.iter().all(|value| {
                non_empty_string(Some(value)).is_some_and(|name| export_names.insert(name))
            })
        {
            return false;
        }

        let Some(assets) = entry.get("assets").and_then(Value::as_array) else {
            return false;
        };
        let mut asset_identities = HashSet::new();
        let mut browser_modules = 0;
        if !assets.iter().all(|asset| {
            let Some(asset) = asset.as_object() else {
                return false;
            };
            let Some(role) = asset.get("role").and_then(Value::as_str) else {
                return false;
            };
            let Some(media_type) = non_empty_string(asset.get("media_type")) else {
                return false;
            };
            let Some(sha256) = asset.get("sha256").and_then(Value::as_str) else {
                return false;
            };
            let Some(_url) = non_empty_string(asset.get("url")) else {
                return false;
            };
            if !matches!(role, "browser_module" | "shadow_style" | "support")
                || media_type.trim().is_empty()
                || !is_sha256(sha256)
                || asset
                    .get("integrity")
                    .is_some_and(|value| value.as_str() != Some("verified_sha256"))
                || !asset_identities.insert((role, sha256))
            {
                return false;
            }
            if role == "browser_module" {
                browser_modules += 1;
            }
            true
        }) {
            return false;
        }
        if binding == "host" {
            assets.is_empty()
        } else {
            browser_modules == 1 && !assets.is_empty()
        }
    })
}

#[derive(Debug)]
struct FrontstageCatalogIdentity {
    installation_id: Uuid,
    provider_code: String,
    plugin_id: String,
    plugin_version: String,
    contribution_code: String,
}

impl FrontstageCatalogIdentity {
    fn from_descriptor(descriptor: &Value) -> Result<Option<Self>> {
        let fields = [
            descriptor.pointer("/catalog/installationId"),
            descriptor.pointer("/catalog/providerCode"),
            descriptor.pointer("/contribution/pluginId"),
            descriptor.pointer("/contribution/pluginVersion"),
            descriptor.pointer("/contribution/code"),
        ];
        if fields
            .iter()
            .all(|value| non_empty_string(*value).is_none())
        {
            return Ok(None);
        }
        let required = |value: Option<&Value>| {
            non_empty_string(value)
                .ok_or(ControlPlaneError::InvalidInput(
                    "frontstage_block_catalog_identity",
                ))
                .map(str::to_owned)
        };
        let installation_id = Uuid::parse_str(&required(fields[0])?)
            .map_err(|_| ControlPlaneError::InvalidInput("frontstage_block_catalog_identity"))?;
        Ok(Some(Self {
            installation_id,
            provider_code: required(fields[1])?,
            plugin_id: required(fields[2])?,
            plugin_version: required(fields[3])?,
            contribution_code: required(fields[4])?,
        }))
    }

    fn matches(&self, entry: &domain::FrontendBlockCatalogEntry) -> bool {
        self.installation_id == entry.installation_id
            && self.provider_code == entry.provider_code
            && self.plugin_id == entry.plugin_id
            && self.plugin_version == entry.plugin_version
            && self.contribution_code == entry.contribution_code
    }
}

fn canonical_dependency_lock(
    workspace_id: Uuid,
    modules: Vec<domain::FrontendBlockCodeModule>,
) -> Result<Value> {
    let entries = modules
        .into_iter()
        .filter(|module| module.source != "tailwindcss")
        .map(|module| {
            let binding = match module.binding {
                domain::FrontendModuleBinding::Host => "host",
                domain::FrontendModuleBinding::Fetched => "fetched",
            };
            let assets = module
                .assets
                .into_iter()
                .map(|asset| {
                    let sha256 = asset.sha256;
                    let role = match asset.role {
                        domain::FrontendModuleAssetRole::BrowserModule => "browser_module",
                        domain::FrontendModuleAssetRole::ShadowStyle => "shadow_style",
                        domain::FrontendModuleAssetRole::Support => "support",
                    };
                    serde_json::json!({
                        "role": role,
                        "media_type": asset.media_type,
                        "sha256": sha256.clone(),
                        "url": format!(
                            "/api/console/frontstage/{workspace_id}/component-module-assets/{}",
                            sha256
                        ),
                        "integrity": "verified_sha256"
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "module_source": module.source,
                "module_version": module.version,
                "binding": binding,
                "assets": assets,
                "exports": module.exports
            })
        })
        .collect::<Vec<_>>();
    let value = Value::Array(entries);
    validate_code(FrontstageBlockCodeInput {
        source_code: String::new(),
        dependency_lock: value,
    })
    .map(|code| code.dependency_lock)
}

fn apply_catalog_identity(
    descriptor: &mut Value,
    entry: &domain::FrontendBlockCatalogEntry,
) -> Result<()> {
    let object = descriptor
        .as_object_mut()
        .ok_or(ControlPlaneError::InvalidInput(
            "frontstage_block_runtime_descriptor",
        ))?;
    object.insert(
        "catalog".to_owned(),
        serde_json::json!({
            "providerCode": entry.provider_code,
            "installationId": entry.installation_id.to_string()
        }),
    );
    object.insert(
        "contribution".to_owned(),
        serde_json::json!({
            "pluginId": entry.plugin_id,
            "pluginVersion": entry.plugin_version,
            "code": entry.contribution_code
        }),
    );
    Ok(())
}

fn inherit_runtime_identity(
    descriptor: Option<Value>,
    parent: Option<&Value>,
) -> Result<Option<Value>> {
    let Some(parent) = parent.and_then(Value::as_object) else {
        return Ok(descriptor);
    };
    let mut descriptor = match descriptor {
        None => Map::new(),
        Some(Value::Object(value)) => value,
        Some(_) => {
            return Err(
                ControlPlaneError::InvalidInput("frontstage_block_runtime_descriptor").into(),
            )
        }
    };
    for field in ["catalog", "contribution", "runtime"] {
        let missing = descriptor
            .get(field)
            .is_none_or(|value| value.as_object().is_some_and(|object| object.is_empty()));
        if missing {
            if let Some(value) = parent.get(field) {
                descriptor.insert(field.to_owned(), value.clone());
            }
        }
    }
    Ok(Some(Value::Object(descriptor)))
}

fn non_empty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn required_block_title(title: String) -> Result<String> {
    let title = title.trim().to_owned();
    if title.is_empty() || title.len() > MAX_BLOCK_TITLE_LEN {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_title").into());
    }
    Ok(title)
}

fn optional_block_description(description: Option<String>) -> Option<String> {
    description.filter(|value| !value.trim().is_empty())
}

fn canonical_runtime_descriptor(
    block_id: &str,
    code_ref: &str,
    descriptor: Option<Value>,
) -> Result<Value> {
    let mut descriptor = match descriptor {
        None => Map::new(),
        Some(Value::Object(descriptor)) => descriptor,
        Some(_) => {
            return Err(
                ControlPlaneError::InvalidInput("frontstage_block_runtime_descriptor").into(),
            )
        }
    };
    descriptor.insert("id".to_owned(), Value::String(block_id.to_owned()));
    descriptor.insert("codeRef".to_owned(), Value::String(code_ref.to_owned()));
    descriptor
        .entry("rendererVersion".to_owned())
        .or_insert_with(|| Value::String(DEFAULT_RENDERER_VERSION.to_owned()));
    descriptor
        .entry("catalog".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    descriptor
        .entry("contribution".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    descriptor
        .entry("props".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    descriptor
        .entry("ports".to_owned())
        .or_insert_with(|| serde_json::json!({ "inputs": [], "outputs": [] }));
    descriptor
        .entry("x-layout".to_owned())
        .or_insert_with(|| serde_json::json!({ "order": 0 }));
    descriptor
        .entry("x-presentation".to_owned())
        .or_insert_with(|| serde_json::json!({ "heightMode": "auto", "height": null }));
    descriptor.entry("runtime".to_owned()).or_insert_with(|| {
        serde_json::json!({
            "kind": "native_react",
            "entry": "index.js",
            "hint": "native_react"
        })
    });
    for field in [
        "catalog",
        "contribution",
        "props",
        "ports",
        "x-layout",
        "x-presentation",
        "runtime",
    ] {
        if !descriptor.get(field).is_some_and(Value::is_object) {
            return Err(
                ControlPlaneError::InvalidInput("frontstage_block_runtime_descriptor").into(),
            );
        }
    }
    let ports = descriptor["ports"].as_object().expect("validated object");
    if !ports.get("inputs").is_some_and(Value::is_array)
        || !ports.get("outputs").is_some_and(Value::is_array)
    {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_runtime_descriptor").into());
    }
    Ok(Value::Object(descriptor))
}

fn block_audit(
    actor: &domain::ActorContext,
    page_id: Uuid,
    block_id: &str,
    event_code: &'static str,
) -> domain::AuditLogRecord {
    audit_log(
        Some(actor.current_workspace_id),
        Some(actor.user_id),
        "frontstage_block",
        Some(page_id),
        event_code,
        serde_json::json!({ "block_id": block_id }),
    )
}

fn map_block_repository_error(error: anyhow::Error) -> anyhow::Error {
    if let Some(tree_error) = error.downcast_ref::<OrderedTreeCommandError>() {
        let mapped = match tree_error {
            OrderedTreeCommandError::NodeNotFound => {
                ControlPlaneError::NotFound("block_node_not_found").into()
            }
            OrderedTreeCommandError::ParentNotFound => {
                ControlPlaneError::NotFound("block_parent_not_found").into()
            }
            OrderedTreeCommandError::AnchorNotFound => {
                ControlPlaneError::NotFound("block_anchor_not_found").into()
            }
            OrderedTreeCommandError::ConflictingAnchors => {
                ControlPlaneError::InvalidInput("block_conflicting_anchors").into()
            }
            OrderedTreeCommandError::Cycle => {
                ControlPlaneError::Conflict("block_tree_cycle").into()
            }
            OrderedTreeCommandError::TreeNodeHasChildren => {
                ControlPlaneError::Conflict("block_node_has_children").into()
            }
            OrderedTreeCommandError::ExpectedAffectedCountMismatch { .. } => {
                ControlPlaneError::Conflict("block_subtree_changed").into()
            }
            OrderedTreeCommandError::PositionConflict => {
                ControlPlaneError::Conflict("block_position_conflict").into()
            }
            OrderedTreeCommandError::AnchorSiblingGroupConflict => {
                ControlPlaneError::Conflict("block_anchor_sibling_group_conflict").into()
            }
            OrderedTreeCommandError::WrongTemplate
            | OrderedTreeCommandError::FieldNotWritable(_) => return error,
        };
        return mapped;
    }
    if let Some(tree_error) = error.downcast_ref::<OrderedTreeQueryError>() {
        let mapped = match tree_error {
            OrderedTreeQueryError::NodeNotFound | OrderedTreeQueryError::ParentNotFound => {
                ControlPlaneError::NotFound("block_node_not_found").into()
            }
            OrderedTreeQueryError::InvalidResultLimit { .. } => {
                ControlPlaneError::InvalidInput("block_result_limit").into()
            }
            OrderedTreeQueryError::InvalidMaxDepth { .. } => {
                ControlPlaneError::InvalidInput("block_max_depth").into()
            }
            OrderedTreeQueryError::EmptySearchPrefix => {
                ControlPlaneError::InvalidInput("block_search_query").into()
            }
            OrderedTreeQueryError::WrongTemplate
            | OrderedTreeQueryError::AncestorDepthLimitExceeded { .. }
            | OrderedTreeQueryError::NoSearchableFields => return error,
        };
        return mapped;
    }
    error
}

#[cfg(test)]
mod code_input_tests {
    use super::*;

    fn code() -> FrontstageBlockCodeInput {
        FrontstageBlockCodeInput {
            source_code: "import 'tailwindcss'; export default () => null;".to_owned(),
            dependency_lock: serde_json::json!([]),
        }
    }

    #[test]
    fn accepts_source_and_canonical_dependency_lock() {
        let validated = validate_code(code()).expect("fixture must be valid");
        assert!(validated.source_code.contains("tailwindcss"));
        assert_eq!(validated.dependency_lock, serde_json::json!([]));
    }

    #[test]
    fn rejects_invalid_dependency_declarations() {
        let mut input = code();
        input.dependency_lock = serde_json::json!({});
        let error = validate_code(input).expect_err("invalid lock must fail");
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::InvalidInput("dependency_lock"))
        ));
    }

    #[test]
    fn rejects_fetched_assets_without_runtime_contract_fields() {
        let mut input = code();
        input.dependency_lock = serde_json::json!([{
            "module_source": "@1flowbase/native-components",
            "module_version": "1.0.0",
            "binding": "fetched",
            "exports": ["Surface"],
            "assets": [{
                "role": "browser_module",
                "sha256": "a".repeat(64),
                "url": "/asset"
            }]
        }]);
        let error = validate_code(input).expect_err("media_type is mandatory");
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::InvalidInput("dependency_lock"))
        ));
    }

    #[test]
    fn accepts_complete_fetched_asset_contract() {
        let mut input = code();
        input.dependency_lock = serde_json::json!([{
            "module_source": "@1flowbase/native-components",
            "module_version": "1.0.0",
            "binding": "fetched",
            "exports": ["Surface"],
            "assets": [{
                "role": "browser_module",
                "media_type": "text/javascript; charset=utf-8",
                "sha256": "a".repeat(64),
                "url": "/asset",
                "integrity": "verified_sha256"
            }]
        }]);
        validate_code(input).expect("complete lock must be accepted");
    }

    #[test]
    fn validates_optimistic_source_revisions() {
        assert_eq!(
            validate_source_revision(Some("a".repeat(64))).unwrap(),
            Some("a".repeat(64))
        );
        assert!(validate_source_revision(Some("latest".to_owned())).is_err());
    }
}
