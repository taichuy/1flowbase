use super::*;
use crate::frontstage::*;
impl<R: PortableTemplateInstallRepository> PortableTemplateInstallService<R> {
    pub(super) async fn map_frontend_installations(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        let descriptors: Vec<_> = package
            .pages
            .iter()
            .flat_map(|p| &p.tabs)
            .flat_map(|t| &t.blocks)
            .map(|b| &b.runtime_descriptor)
            .filter(|d| {
                d.pointer("/catalog/installationId")
                    .and_then(serde_json::Value::as_str)
                    .is_some()
            })
            .collect();
        if descriptors.is_empty() {
            return Ok(());
        }
        let node_id = self
            .node_id
            .as_deref()
            .context("portable_template_frontend_catalog_node_missing")?;
        let catalog = self
            .repository
            .list_workspace_frontend_blocks(node_id, actor.current_workspace_id)
            .await?;
        for descriptor in descriptors {
            let field = |path| {
                descriptor
                    .pointer(path)
                    .and_then(serde_json::Value::as_str)
                    .context("portable_template_frontend_catalog_identity")
            };
            let source = field("/catalog/installationId")?;
            let provider = field("/catalog/providerCode")?;
            let plugin = field("/contribution/pluginId")?;
            let version = field("/contribution/pluginVersion")?;
            let contribution = field("/contribution/code")?;
            let entry = catalog
                .iter()
                .find(|e| {
                    e.provider_code == provider
                        && e.plugin_id == plugin
                        && e.plugin_version == version
                        && e.contribution_code == contribution
                })
                .context("portable_template_target_frontend_contribution_missing")?;
            result
                .id_map
                .insert(source.to_owned(), entry.installation_id.to_string());
        }
        Ok(())
    }
    pub(super) async fn create_pages(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        target: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        let owner = self.page_owner(actor);
        let mut pending: Vec<_> = package.pages.iter().collect();
        while !pending.is_empty() {
            let index = pending
                .iter()
                .position(|p| {
                    p.parent_id
                        .is_none_or(|id| result.id_map.contains_key(&id.to_string()))
                })
                .context("unresolved page parent/cycle")?;
            let page = pending.remove(index);
            let parent_id = page.parent_id.map(|id| result.mapped(id)).transpose()?;
            let target_id: Uuid = result
                .id_map
                .get(&page.id.to_string())
                .and_then(|id| id.parse().ok())
                .unwrap_or(page.id);
            if let Some(existing) = target.pages.iter().find(|item| item.id == target_id) {
                owner
                    .move_page(MoveFrontstagePageCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id: existing.id,
                        parent_id,
                        rank: Some(page.rank.clone()),
                    })
                    .await?;
                owner
                    .update_metadata(UpdateFrontstagePageMetadataCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id: existing.id,
                        title: Some(page.title.clone()),
                        icon: Some(page.icon.clone()),
                        tooltip: Some(page.tooltip.clone()),
                        is_hidden: Some(page.is_hidden),
                        placement: Some(page.placement),
                        content_presentation: (page.kind == domain::FrontstagePageKind::Page)
                            .then_some(page.content_presentation),
                        slug: Some(page.slug.clone()),
                    })
                    .await?;
                result.updated("page", page.id, existing.id);
                for tab in &page.tabs {
                    let mapped_id = result
                        .id_map
                        .get(&tab.id.to_string())
                        .and_then(|id| id.parse::<Uuid>().ok());
                    let existing_tab = existing
                        .tabs
                        .iter()
                        .find(|item| Some(item.id) == mapped_id || item.id == tab.id)
                        .or_else(|| {
                            tab.is_default
                                .then(|| existing.tabs.iter().find(|item| item.is_default))
                                .flatten()
                        });
                    if let Some(existing_tab) = existing_tab {
                        owner
                            .update_page_tab(UpdateFrontstagePageTabCommand {
                                actor_user_id: actor.user_id,
                                workspace_id: actor.current_workspace_id,
                                page_id: existing.id,
                                tab_id: existing_tab.id,
                                title: Some(tab.title.clone()),
                                rank: Some(tab.rank.clone()),
                            })
                            .await?;
                        result.updated("tab", tab.id, existing_tab.id);
                        result.id_map.insert(
                            tab.document_root_uid.clone(),
                            existing_tab.document_root_uid.clone(),
                        );
                    } else {
                        let created = owner
                            .create_page_tab(CreateFrontstagePageTabCommand {
                                actor_user_id: actor.user_id,
                                workspace_id: actor.current_workspace_id,
                                page_id: existing.id,
                                title: tab.title.clone(),
                                route_segment: tab.route_segment.clone(),
                                rank: Some(tab.rank.clone()),
                            })
                            .await?;
                        self.record_created(actor, result, "tab", tab.id, created.id)
                            .await?;
                        result
                            .id_map
                            .insert(tab.document_root_uid.clone(), created.document_root_uid);
                    }
                }
                continue;
            }
            let created = match page.kind {
                domain::FrontstagePageKind::Group => domain::frontstage::FrontstagePageCreation {
                    page: owner
                        .create_group(CreateFrontstageGroupCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            title: page.title.clone(),
                            icon: page.icon.clone(),
                            tooltip: page.tooltip.clone(),
                            parent_id,
                            rank: Some(page.rank.clone()),
                            placement: page.placement,
                            slug: page.slug.clone(),
                        })
                        .await?,
                    default_tab: None,
                },
                domain::FrontstagePageKind::Page => {
                    owner
                        .create_page(CreateFrontstagePageCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            title: page.title.clone(),
                            icon: page.icon.clone(),
                            tooltip: page.tooltip.clone(),
                            parent_id,
                            rank: Some(page.rank.clone()),
                            placement: page.placement,
                            slug: page.slug.clone(),
                        })
                        .await?
                }
            };
            let page_id = created.page.id;
            self.record_created(actor, result, "page", page.id, page_id)
                .await?;
            owner
                .update_metadata(UpdateFrontstagePageMetadataCommand {
                    actor_user_id: actor.user_id,
                    workspace_id: actor.current_workspace_id,
                    page_id,
                    title: None,
                    icon: None,
                    tooltip: None,
                    is_hidden: Some(page.is_hidden),
                    placement: None,
                    content_presentation: (page.kind == domain::FrontstagePageKind::Page)
                        .then_some(page.content_presentation),
                    slug: None,
                })
                .await?;
            for tab in &page.tabs {
                let created_tab = if tab.is_default {
                    let default = created.default_tab.as_ref().context("default tab absent")?;
                    owner
                        .update_page_tab(UpdateFrontstagePageTabCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            page_id,
                            tab_id: default.id,
                            title: Some(tab.title.clone()),
                            rank: Some(tab.rank.clone()),
                        })
                        .await?
                } else {
                    owner
                        .create_page_tab(CreateFrontstagePageTabCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            page_id,
                            title: tab.title.clone(),
                            route_segment: tab.route_segment.clone(),
                            rank: Some(tab.rank.clone()),
                        })
                        .await?
                };
                self.record_created(actor, result, "tab", tab.id, created_tab.id)
                    .await?;
                result
                    .id_map
                    .insert(tab.document_root_uid.clone(), created_tab.document_root_uid);
            }
        }
        // Create the block hierarchy in stable sibling order, then rewrite content in a second pass.
        for page in &package.pages {
            let page_id = result.mapped(page.id)?;
            for tab in &page.tabs {
                let tab_id = result.mapped(tab.id)?;
                let mut pending: Vec<_> = tab.blocks.iter().collect();
                pending.sort_by(|a, b| a.rank.cmp(&b.rank));
                let mut last_sibling: BTreeMap<Option<String>, String> = BTreeMap::new();
                while !pending.is_empty() {
                    let index = pending
                        .iter()
                        .position(|b| {
                            b.parent_block_id
                                .as_ref()
                                .is_none_or(|id| result.id_map.contains_key(id))
                        })
                        .context("unresolved block parent/cycle")?;
                    let block = pending.remove(index);
                    let target_block = target
                        .pages
                        .iter()
                        .find(|item| item.id == page_id)
                        .and_then(|item| item.tabs.iter().find(|item| item.id == tab_id))
                        .and_then(|item| {
                            item.blocks.iter().find(|item| {
                                item.block_id
                                    == result
                                        .id_map
                                        .get(&block.block_id)
                                        .cloned()
                                        .unwrap_or_else(|| block.block_id.clone())
                            })
                        });
                    if let Some(existing) = target_block {
                        result.updated("block", &block.block_id, &existing.block_id);
                        result
                            .id_map
                            .insert(block.code_ref.clone(), existing.code_ref.clone());
                        continue;
                    }
                    let parent_block_id = block
                        .parent_block_id
                        .as_ref()
                        .map(|id| {
                            result
                                .id_map
                                .get(id)
                                .cloned()
                                .context("block parent missing")
                        })
                        .transpose()?;
                    let created = owner
                        .create_block_node(CreateFrontstageBlockNodeCommand {
                            actor_user_id: actor.user_id,
                            workspace_id: actor.current_workspace_id,
                            page_id,
                            tab_id: Some(tab_id),
                            title: block.title.clone(),
                            description: block.description.clone(),
                            presentation: block.presentation,
                            position: FrontstageBlockPosition {
                                parent_block_id: parent_block_id.clone(),
                                before_block_id: None,
                                after_block_id: last_sibling.get(&parent_block_id).cloned(),
                            },
                            source_code: block.source_code.clone(),
                            input_mapping: block.input_mapping.clone(),
                            output_mapping: block.output_mapping.clone(),
                            runtime_descriptor: Some(
                                result.value(block.runtime_descriptor.clone()),
                            ),
                        })
                        .await?;
                    last_sibling.insert(parent_block_id, created.block_id.clone());
                    self.record_created(actor, result, "block", &block.block_id, &created.block_id)
                        .await?;
                    result
                        .id_map
                        .insert(block.code_ref.clone(), created.code_ref);
                }
            }
        }
        Ok(())
    }
    pub(super) async fn fill_pages(
        &self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        let owner = self.page_owner(actor);
        for page in &package.pages {
            let page_id = result.mapped(page.id)?;
            for tab in &page.tabs {
                let mut blocks: Vec<_> = tab.blocks.iter().collect();
                blocks.sort_by(|left, right| left.rank.cmp(&right.rank));
                let mut last_sibling: BTreeMap<Option<String>, String> = BTreeMap::new();
                for block in blocks {
                    let block_id = result
                        .id_map
                        .get(&block.block_id)
                        .context("block mapping missing")?
                        .clone();
                    let scope = || FrontstageBlockScopeCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id,
                        block_id: block_id.clone(),
                    };
                    let parent_block_id = block
                        .parent_block_id
                        .as_ref()
                        .map(|id| {
                            result
                                .id_map
                                .get(id)
                                .cloned()
                                .context("block parent mapping missing")
                        })
                        .transpose()?;
                    owner
                        .move_block_node(MoveFrontstageBlockNodeCommand {
                            scope: scope(),
                            position: FrontstageBlockPosition {
                                parent_block_id: parent_block_id.clone(),
                                before_block_id: None,
                                after_block_id: last_sibling.get(&parent_block_id).cloned(),
                            },
                        })
                        .await?;
                    last_sibling.insert(parent_block_id, block_id.clone());
                    owner
                        .update_block_node(UpdateFrontstageBlockNodeCommand {
                            scope: scope(),
                            title: block.title.clone(),
                            description: block.description.clone(),
                            presentation: Some(block.presentation),
                            input_mapping: Some(serde_json::from_value(
                                result.value(serde_json::to_value(&block.input_mapping)?),
                            )?),
                            output_mapping: Some(serde_json::from_value(
                                result.value(serde_json::to_value(&block.output_mapping)?),
                            )?),
                            runtime_descriptor: Some(
                                result.value(block.runtime_descriptor.clone()),
                            ),
                        })
                        .await?;
                    owner
                        .save_block_node_code(SaveFrontstageBlockNodeCodeCommand {
                            scope: scope(),
                            expected_source_revision: None,
                            source_code: rewrite_template_text(&block.source_code, &result.id_map),
                        })
                        .await?;
                }
                owner
                    .save_tab_document(SaveFrontstageTabDocumentCommand {
                        actor_user_id: actor.user_id,
                        workspace_id: actor.current_workspace_id,
                        page_id,
                        tab_id: result.mapped(tab.id)?,
                        document_payload: result.value(tab.document_payload.clone()),
                    })
                    .await?;
            }
        }
        Ok(())
    }
}
