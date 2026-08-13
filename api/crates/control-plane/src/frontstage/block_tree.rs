use std::collections::BTreeMap;

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
        DeleteFrontstageBlockSubtreeInput, FrontstageBlockExecutableInput, FrontstageBlockPosition,
        FrontstageBlockSubtreeDeleteResult, FrontstageBlockTreeRepository,
        FrontstagePageRepository, MoveFrontstageBlockNodeInput, SaveFrontstageBlockNodeCodeInput,
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
    pub query: String,
    pub limit: u32,
}

pub struct CreateFrontstageBlockNodeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub presentation: domain::FrontstageBlockPresentation,
    pub position: FrontstageBlockPosition,
    pub executable: FrontstageBlockExecutableInput,
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
    pub executable: FrontstageBlockExecutableInput,
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
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        self.ensure_block_designer(
            command.actor_user_id,
            command.workspace_id,
            command.page_id,
            None,
        )
        .await?;
        self.repository
            .list_frontstage_block_roots(command.workspace_id, command.page_id, command.limit)
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
            None,
        )
        .await?;
        self.repository
            .search_frontstage_blocks(
                command.workspace_id,
                command.page_id,
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
    ) -> Result<domain::FrontstageBlockNodeRecord> {
        let actor = self
            .ensure_block_designer(
                command.actor_user_id,
                command.workspace_id,
                command.page_id,
                Some(command.tab_id),
            )
            .await?;
        let title = required_block_title(command.title)?;
        let description = optional_block_description(command.description);
        let block_id = Uuid::now_v7().to_string();
        let code_ref = format!("frontstage.block.{block_id}");
        let runtime_descriptor =
            canonical_runtime_descriptor(&block_id, &code_ref, command.runtime_descriptor)?;
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
                tab_id: command.tab_id,
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
                executable: validate_executable(command.executable)?,
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
                executable: validate_executable(command.executable)?,
                audit_log,
            })
            .await
            .map_err(map_block_repository_error)
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

pub(super) fn validate_executable(
    executable: FrontstageBlockExecutableInput,
) -> Result<FrontstageBlockExecutableInput> {
    if !is_dependency_lock(&executable.dependency_lock) {
        return Err(ControlPlaneError::InvalidInput("dependency_lock").into());
    }
    if !is_identity_object(&executable.tailwind_toolchain_lock) {
        return Err(ControlPlaneError::InvalidInput("tailwind_toolchain_lock").into());
    }
    if !is_identity_object(&executable.compiler_identity) {
        return Err(ControlPlaneError::InvalidInput("compiler_identity").into());
    }
    let digest = format!("{:x}", Sha256::digest(executable.generated_css.as_bytes()));
    if digest != executable.generated_css_sha256 {
        return Err(ControlPlaneError::InvalidInput("generated_css_sha256").into());
    }
    let imports_tailwind = executable.source_code.contains("import 'tailwindcss'")
        || executable.source_code.contains("import \"tailwindcss\"");
    if imports_tailwind != !executable.generated_css.is_empty() {
        return Err(ControlPlaneError::InvalidInput("tailwind_style_payload").into());
    }
    Ok(executable)
}

fn is_identity_object(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        !object.is_empty()
            && object
                .values()
                .all(|value| value.as_str().is_some_and(|value| !value.trim().is_empty()))
    })
}

fn is_dependency_lock(value: &Value) -> bool {
    value.as_array().is_some_and(|entries| {
        entries.iter().all(|entry| {
            entry.as_object().is_some_and(|entry| {
                ["module_source", "module_version", "binding"]
                    .iter()
                    .all(|field| {
                        entry
                            .get(*field)
                            .and_then(Value::as_str)
                            .is_some_and(|v| !v.is_empty())
                    })
                    && entry.get("assets").is_some_and(Value::is_array)
                    && entry
                        .get("exports")
                        .and_then(Value::as_array)
                        .is_some_and(|exports| {
                            !exports.is_empty()
                                && exports
                                    .iter()
                                    .all(|export| export.as_str().is_some_and(|v| !v.is_empty()))
                        })
            })
        })
    })
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
mod executable_state_tests {
    use super::*;

    fn executable() -> FrontstageBlockExecutableInput {
        FrontstageBlockExecutableInput {
            source_code: "import 'tailwindcss'; export default () => null;".to_owned(),
            dependency_lock: serde_json::json!([]),
            tailwind_toolchain_lock: serde_json::json!({
                "package": "tailwindcss",
                "version": "4.3.3",
                "theme_sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }),
            generated_css: "a{}".to_owned(),
            generated_css_sha256:
                "5f546eb4606b5c2b7d2a449a5cc2bbb477ed5a246c7051ce871b12f2dbfc8419".to_owned(),
            compiler_identity: serde_json::json!({ "name": "tailwindcss", "abi": "v1" }),
        }
    }

    #[test]
    fn ac_005_006_preserves_a_deterministic_locked_executable_payload() {
        let validated = validate_executable(executable()).expect("fixture must be valid");
        assert_eq!(validated.generated_css, "a{}");
        assert_eq!(validated.tailwind_toolchain_lock["version"], "4.3.3");
    }

    #[test]
    fn ac_008_011_rejects_digest_mismatch_before_the_repository_write() {
        let mut input = executable();
        input.generated_css_sha256 = "0".repeat(64);
        let error = validate_executable(input).expect_err("mismatch must fail closed");
        assert!(matches!(
            error.downcast_ref::<ControlPlaneError>(),
            Some(ControlPlaneError::InvalidInput("generated_css_sha256"))
        ));
    }

    #[test]
    fn ac_011_rejects_tailwind_import_without_a_style_payload() {
        let mut input = executable();
        input.generated_css.clear();
        input.generated_css_sha256 =
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned();
        assert!(validate_executable(input).is_err());
    }
}
