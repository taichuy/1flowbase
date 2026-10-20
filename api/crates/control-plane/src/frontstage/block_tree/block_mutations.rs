use super::*;

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository + FrontstageBlockTreeRepository,
{
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
}
