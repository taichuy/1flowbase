use std::collections::{BTreeMap, HashSet};

use anyhow::Result;
use runtime_core::runtime_record_repository::{OrderedTreeCommandError, OrderedTreeQueryError};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
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
const MAX_SOURCE_FRAGMENT_LINES: u32 = 1_000;
const MAX_SOURCE_FRAGMENT_CHARS: u32 = 50_000;
const MAX_SOURCE_EDITS: usize = 100;

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
    pub title: Option<String>,
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

pub struct GetFrontstageBlockCodeFragmentCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub start_line: u32,
    pub start_column: u32,
    pub line_count: u32,
    pub max_chars: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstageSourceEdit {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub replacement: String,
}

pub struct PatchFrontstageBlockNodeCodeCommand {
    pub scope: FrontstageBlockScopeCommand,
    pub expected_source_revision: String,
    pub edits: Vec<FrontstageSourceEdit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrontstageBlockCodeFragment {
    pub block_id: String,
    pub page_id: Uuid,
    pub source_revision: String,
    pub source_fragment: String,
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
    pub total_lines: u32,
    pub total_chars: u64,
    pub next_line: Option<u32>,
    pub next_column: Option<u32>,
    pub truncated_by_max_chars: bool,
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
        let description = optional_block_description(command.description);
        let block_uuid = Uuid::now_v7();
        let block_id = block_uuid.to_string();
        let title = command
            .title
            .map(required_block_title)
            .transpose()?
            .unwrap_or_else(|| default_block_title(block_uuid));
        let code_ref = format!("frontstage.block.{block_id}");
        let runtime_descriptor = canonical_runtime_descriptor(
            &block_id,
            &code_ref,
            inherit_runtime_identity(
                command.runtime_descriptor,
                parent_runtime_descriptor.as_ref(),
            )?,
        )?;
        let runtime_descriptor = self
            .resolve_runtime_descriptor(command.workspace_id, runtime_descriptor)
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

    pub async fn get_block_code_fragment(
        &self,
        command: GetFrontstageBlockCodeFragmentCommand,
    ) -> Result<FrontstageBlockCodeFragment> {
        let block_id = command.scope.block_id.clone();
        let code = self.get_block_node_code(command.scope).await?;
        source_fragment(
            block_id,
            code,
            command.start_line,
            command.start_column,
            command.line_count,
            command.max_chars,
        )
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

    pub async fn patch_block_node_code(
        &self,
        command: PatchFrontstageBlockNodeCodeCommand,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        let expected_source_revision =
            validate_source_revision(Some(command.expected_source_revision))?
                .ok_or(ControlPlaneError::InvalidInput("expected_source_revision"))?;
        let actor = self
            .ensure_block_designer(
                command.scope.actor_user_id,
                command.scope.workspace_id,
                command.scope.page_id,
                None,
            )
            .await?;
        let node = self.load_block_node(&command.scope).await?;
        let current = self
            .repository
            .get_frontstage_block_code(
                command.scope.workspace_id,
                command.scope.page_id,
                &node.code_ref,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("block_node_not_found"))?;
        if current.source_sha256.as_deref() != Some(expected_source_revision.as_str()) {
            return Err(ControlPlaneError::Conflict("frontstage_block_source_revision").into());
        }
        let source_code = apply_source_edits(&current.source_code, command.edits)?;
        let audit_log = block_audit(
            &actor,
            command.scope.page_id,
            &command.scope.block_id,
            "frontstage.block_node_code_patched",
        );
        self.repository
            .save_frontstage_block_node_code(&SaveFrontstageBlockNodeCodeInput {
                workspace_id: command.scope.workspace_id,
                actor_user_id: command.scope.actor_user_id,
                page_id: command.scope.page_id,
                block_id: command.scope.block_id,
                expected_source_revision: Some(expected_source_revision),
                source: FrontstageBlockSourceInput { source_code },
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
    }

    async fn resolve_runtime_descriptor(
        &self,
        workspace_id: Uuid,
        mut runtime_descriptor: Value,
    ) -> Result<Value>
    where
        R: FrontendBlockCatalogRepository,
    {
        if runtime_descriptor
            .pointer("/runtime/kind")
            .and_then(Value::as_str)
            != Some("native_react")
        {
            return Ok(runtime_descriptor);
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
        Ok(runtime_descriptor)
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
    Ok(code)
}

fn source_fragment(
    block_id: String,
    code: domain::frontstage::FrontstageBlockCodeRecord,
    start_line: u32,
    start_column: u32,
    line_count: u32,
    max_chars: u32,
) -> Result<FrontstageBlockCodeFragment> {
    if line_count == 0 || line_count > MAX_SOURCE_FRAGMENT_LINES {
        return Err(ControlPlaneError::InvalidInput("line_count").into());
    }
    if max_chars == 0 || max_chars > MAX_SOURCE_FRAGMENT_CHARS {
        return Err(ControlPlaneError::InvalidInput("max_chars").into());
    }
    let source_revision = code
        .source_sha256
        .clone()
        .ok_or(ControlPlaneError::Conflict(
            "frontstage_block_source_revision",
        ))?;
    let line_starts = source_line_starts(&code.source_code);
    let start_offset =
        source_position_offset(&code.source_code, &line_starts, start_line, start_column)?;
    let requested_line_end = usize::try_from(start_line - 1)
        .map_err(|_| ControlPlaneError::InvalidInput("start_line"))?
        .saturating_add(
            usize::try_from(line_count)
                .map_err(|_| ControlPlaneError::InvalidInput("line_count"))?,
        )
        .min(line_starts.len());
    let requested_end_offset = line_starts
        .get(requested_line_end)
        .copied()
        .unwrap_or(code.source_code.len());
    let mut source_fragment = String::new();
    let mut end_offset = start_offset;
    let mut end_line = start_line;
    let mut end_column = start_column;
    for character in code.source_code[start_offset..requested_end_offset]
        .chars()
        .take(max_chars as usize)
    {
        source_fragment.push(character);
        end_offset += character.len_utf8();
        if character == '\n' {
            end_line += 1;
            end_column = 1;
        } else {
            end_column += 1;
        }
    }
    let truncated_by_max_chars = end_offset < requested_end_offset;
    let has_more = end_offset < code.source_code.len();
    Ok(FrontstageBlockCodeFragment {
        block_id,
        page_id: code.page_id,
        source_revision,
        source_fragment,
        start_line,
        start_column,
        end_line,
        end_column,
        total_lines: u32::try_from(line_starts.len())
            .map_err(|_| ControlPlaneError::InvalidInput("source_code"))?,
        total_chars: u64::try_from(code.source_code.chars().count())
            .map_err(|_| ControlPlaneError::InvalidInput("source_code"))?,
        next_line: has_more.then_some(end_line),
        next_column: has_more.then_some(end_column),
        truncated_by_max_chars,
    })
}

fn apply_source_edits(source: &str, edits: Vec<FrontstageSourceEdit>) -> Result<String> {
    if edits.is_empty() || edits.len() > MAX_SOURCE_EDITS {
        return Err(ControlPlaneError::InvalidInput("edits").into());
    }
    let line_starts = source_line_starts(source);
    let mut ranges = edits
        .into_iter()
        .map(|edit| {
            let start =
                source_position_offset(source, &line_starts, edit.start_line, edit.start_column)?;
            let end = source_position_offset(source, &line_starts, edit.end_line, edit.end_column)?;
            if start > end {
                return Err(ControlPlaneError::InvalidInput("source_edit_range").into());
            }
            Ok((start, end, edit.replacement))
        })
        .collect::<Result<Vec<_>>>()?;
    ranges.sort_by_key(|(start, end, _)| (*start, *end));
    for pair in ranges.windows(2) {
        let previous = &pair[0];
        let current = &pair[1];
        if current.0 < previous.1 || current.0 == previous.0 {
            return Err(ControlPlaneError::InvalidInput("source_edit_overlap").into());
        }
    }
    let mut patched = source.to_owned();
    for (start, end, replacement) in ranges.into_iter().rev() {
        patched.replace_range(start..end, &replacement);
    }
    Ok(patched)
}

fn source_line_starts(source: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(
            source
                .match_indices('\n')
                .map(|(offset, _)| offset.saturating_add(1)),
        )
        .collect()
}

fn source_position_offset(
    source: &str,
    line_starts: &[usize],
    line: u32,
    column: u32,
) -> Result<usize> {
    if line == 0 || column == 0 {
        return Err(ControlPlaneError::InvalidInput("source_position").into());
    }
    let line_index = usize::try_from(line - 1)
        .map_err(|_| ControlPlaneError::InvalidInput("source_position"))?;
    let line_start = *line_starts
        .get(line_index)
        .ok_or(ControlPlaneError::InvalidInput("source_position"))?;
    let line_end = line_starts
        .get(line_index.saturating_add(1))
        .map(|offset| offset.saturating_sub(1))
        .unwrap_or(source.len());
    let line_source = &source[line_start..line_end];
    let character_index = usize::try_from(column - 1)
        .map_err(|_| ControlPlaneError::InvalidInput("source_position"))?;
    if character_index == line_source.chars().count() {
        return Ok(line_end);
    }
    line_source
        .char_indices()
        .nth(character_index)
        .map(|(offset, _)| line_start + offset)
        .ok_or(ControlPlaneError::InvalidInput("source_position").into())
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

fn required_block_title(title: String) -> Result<String> {
    let title = title.trim().to_owned();
    if title.is_empty() || title.len() > MAX_BLOCK_TITLE_LEN {
        return Err(ControlPlaneError::InvalidInput("frontstage_block_title").into());
    }
    Ok(title)
}

pub(crate) fn default_block_title(block_id: Uuid) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let digest = Sha256::digest(block_id.as_bytes());
    let bits = u64::from_be_bytes([
        0, 0, 0, digest[0], digest[1], digest[2], digest[3], digest[4],
    ]);
    let mut title = String::with_capacity(8);
    for shift in (0..8).rev() {
        title.push(ALPHABET[((bits >> (shift * 5)) & 31) as usize] as char);
    }
    title
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
        }
    }

    #[test]
    fn ac_001_accepts_source_without_backend_dependency_resolution() {
        let validated = validate_code(code()).expect("fixture must be valid");
        assert!(validated.source_code.contains("tailwindcss"));
        validate_code(FrontstageBlockCodeInput {
            source_code: "import { Missing } from '@not-installed/module'; export default Missing;"
                .to_owned(),
        })
        .expect("backend must persist source without compiling frontend imports");
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
