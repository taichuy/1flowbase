use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn native_kind(kind: &str) -> bool {
    matches!(
        kind,
        "page"
            | "tab"
            | "tab_document"
            | "block"
            | "application"
            | "data_model"
            | "model_field"
            | "page_visibility"
    )
}

pub fn template_baseline_scope(
    workspace_id: Uuid,
    package: &PortableTemplatePackage,
) -> TemplateBaselineScope {
    TemplateBaselineScope {
        workspace_id,
        template_id: package
            .release
            .as_ref()
            .map(|release| release.template_id.clone())
            .unwrap_or_else(|| "@legacy/portable".into()),
    }
}

/// Reconstruct references to generated roots, code artifacts and flow IDs from the
/// authenticated snapshot; this maps identity only and does not adopt any baseline.
pub fn map_existing_template_identities(
    package: &PortableTemplatePackage,
    target: &PortableTemplatePackage,
    identities: &mut BTreeMap<String, String>,
) {
    let mapped = |ids: &BTreeMap<String, String>, id: Uuid| {
        ids.get(&id.to_string())
            .and_then(|v| v.parse().ok())
            .unwrap_or(id)
    };
    for model in &package.data_models {
        let existing = target.data_models.iter().find(|item| {
            if model.builtin {
                item.builtin && item.code == model.code
            } else {
                item.id == mapped(identities, model.id)
            }
        });
        if let Some(existing) = existing {
            identities.insert(model.id.to_string(), existing.id.to_string());
            for field in &model.fields {
                if let Some(current) = existing
                    .fields
                    .iter()
                    .find(|item| item.id == mapped(identities, field.id) || item.code == field.code)
                {
                    identities.insert(field.id.to_string(), current.id.to_string());
                }
            }
        }
    }
    for page in &package.pages {
        if let Some(existing) = target
            .pages
            .iter()
            .find(|item| item.id == mapped(identities, page.id))
        {
            identities.insert(page.id.to_string(), existing.id.to_string());
            for tab in &page.tabs {
                let current = existing
                    .tabs
                    .iter()
                    .find(|item| item.id == mapped(identities, tab.id))
                    .or_else(|| {
                        (tab.is_default && !identities.contains_key(&tab.id.to_string()))
                            .then(|| existing.tabs.iter().find(|t| t.is_default))
                            .flatten()
                    });
                if let Some(current) = current {
                    identities.insert(tab.id.to_string(), current.id.to_string());
                    identities.insert(
                        tab.document_root_uid.clone(),
                        current.document_root_uid.clone(),
                    );
                    for block in &tab.blocks {
                        let id = identities.get(&block.block_id).unwrap_or(&block.block_id);
                        if let Some(current_block) =
                            current.blocks.iter().find(|b| &b.block_id == id)
                        {
                            identities
                                .insert(block.block_id.clone(), current_block.block_id.clone());
                            identities
                                .insert(block.code_ref.clone(), current_block.code_ref.clone());
                            let source = &block.runtime_descriptor;
                            let current = &current_block.runtime_descriptor;
                            if [
                                "/contribution/pluginId",
                                "/contribution/pluginVersion",
                                "/contribution/code",
                            ]
                            .iter()
                            .all(|path| source.pointer(path) == current.pointer(path))
                            {
                                if let (Some(source_id), Some(target_id)) = (
                                    source
                                        .pointer("/catalog/installationId")
                                        .and_then(serde_json::Value::as_str),
                                    current
                                        .pointer("/catalog/installationId")
                                        .and_then(serde_json::Value::as_str),
                                ) {
                                    identities.insert(source_id.into(), target_id.into());
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for app in &package.applications {
        if let Some(existing) = target
            .applications
            .iter()
            .find(|item| item.id == mapped(identities, app.id))
        {
            identities.insert(app.id.to_string(), existing.id.to_string());
            if let Some(target_flow) = existing
                .flow_document
                .pointer("/meta/flowId")
                .and_then(serde_json::Value::as_str)
            {
                for document in std::iter::once(&app.flow_document)
                    .chain(app.published.iter().map(|p| &p.flow_document))
                {
                    if let Some(source) = document
                        .pointer("/meta/flowId")
                        .and_then(serde_json::Value::as_str)
                    {
                        identities.insert(source.into(), target_flow.into());
                    }
                }
            }
        }
    }
}

pub fn plan_native_template(
    package: &PortableTemplatePackage,
    target: &PortableTemplatePackage,
    identities: &BTreeMap<String, String>,
    baselines: &[TemplateResourceBaseline],
    historical_identities: &BTreeMap<String, String>,
) -> Result<Vec<PlannedTemplateResource>> {
    let desired: Vec<_> = project_template_resources(package, identities)?
        .into_iter()
        .filter(|r| native_kind(&r.key.kind))
        .collect();
    let tracked: BTreeSet<_> = desired
        .iter()
        .filter(|r| historical_identities.contains_key(&r.key.source_id))
        .map(|r| r.key.clone())
        .collect();
    let current = project_template_resources(target, &BTreeMap::new())?;
    // Preview simulates only proven owner receipts; install finalizes the same receipts
    // durably before planning. Mere equality with desired content never repairs history.
    let effective: Vec<_> =
        baselines
            .iter()
            .cloned()
            .map(|mut baseline| {
                if baseline.pending.as_ref().is_some_and(|intent| {
                    baseline.committed_operation_id == Some(intent.operation_id)
                }) && baseline.committed_fingerprint.is_some()
                {
                    baseline.applied_fingerprint = baseline.committed_fingerprint.take();
                    baseline.committed_operation_id = None;
                    baseline.pending = None;
                    baseline.generation += 1;
                }
                baseline
            })
            .collect();
    let mut plan = plan_template_resources(desired, &current, &effective, &tracked)?;
    // Missing preserved parents are not recreated. Existing locally modified parents
    // remain valid containers and do not block independently editable children.
    let mut unavailable: BTreeSet<String> = plan
        .iter()
        .filter(|p| {
            p.current_fingerprint.is_none() && p.decision != TemplateMergeDecision::Initialize
        })
        .map(|p| p.desired.key.source_id.clone())
        .collect();
    loop {
        let mut changed = false;
        for item in &mut plan {
            if item.decision.reason().is_some() {
                continue;
            }
            let source = &item.desired.key.source_id;
            let missing_parent = match item.desired.key.kind.as_str() {
                "page" => package
                    .pages
                    .iter()
                    .find(|p| p.id.to_string() == *source)
                    .and_then(|p| p.parent_id)
                    .is_some_and(|id| unavailable.contains(&id.to_string())),
                "tab" | "tab_document" => package.pages.iter().any(|p| {
                    unavailable.contains(&p.id.to_string())
                        && p.tabs.iter().any(|t| t.id.to_string() == *source)
                }),
                "block" => package.pages.iter().any(|p| {
                    p.tabs.iter().any(|t| {
                        t.blocks.iter().any(|b| {
                            b.block_id == *source
                                && (unavailable.contains(&p.id.to_string())
                                    || unavailable.contains(&t.id.to_string())
                                    || b.parent_block_id
                                        .as_ref()
                                        .is_some_and(|id| unavailable.contains(id)))
                        })
                    })
                }),
                "model_field" => package.data_models.iter().any(|m| {
                    unavailable.contains(&m.id.to_string())
                        && m.fields.iter().any(|f| f.id.to_string() == *source)
                }),
                "page_visibility" => serde_json::from_str::<Vec<String>>(source)
                    .ok()
                    .is_some_and(|parts| parts.iter().take(2).any(|id| unavailable.contains(id))),
                _ => false,
            };
            if missing_parent {
                item.decision = TemplateMergeDecision::SkipUserDeleted;
                changed |= unavailable.insert(source.clone());
            }
        }
        if !changed {
            break;
        }
    }
    Ok(plan)
}

impl<R: PortableTemplateInstallRepository> PortableTemplateInstallService<R> {
    pub(super) fn can_apply(&self, kind: &str, source: impl ToString) -> bool {
        self.allowed.as_ref().is_some_and(|allowed| {
            allowed.contains(&TemplateResourceKey {
                kind: kind.into(),
                source_id: source.to_string(),
            })
        })
    }
    pub(super) async fn safe_install(
        &self,
        actor_user_id: Uuid,
        package: PortableTemplatePackage,
    ) -> Result<PortableTemplateInstallResult> {
        let actor =
            ApplicationRepository::load_actor_context_for_user(&self.repository, actor_user_id)
                .await?;
        let preview = PortableTemplateService::new(self.repository.clone())
            .preview(actor_user_id, &package)
            .await?;
        anyhow::ensure!(
            preview.valid,
            "portable template preflight: {}",
            preview.failures.join("; ")
        );
        self.preflight_visibility(&actor, &package).await?;
        let mut transaction = self
            .repository
            .begin_portable_template_transaction()
            .await?;
        let mut owner = Self::new(transaction.repository.clone());
        owner.node_id = self.node_id.clone();
        let mut result = PortableTemplateInstallResult::default();
        let outcome = owner
            .install_in_transaction(&actor, &package, &mut result)
            .await;
        match outcome {
            Ok(()) => {
                transaction.guard.commit().await?;
                result.complete = true;
            }
            Err(error) => {
                transaction.guard.rollback().await?;
                result.created.clear();
                result.updated.clear();
                result.id_map.clear();
                result.failures.push(format!(
                    "{error:#}; native template resources and baselines rolled back"
                ));
            }
        }
        Ok(result)
    }
    async fn install_in_transaction(
        &mut self,
        actor: &domain::ActorContext,
        package: &PortableTemplatePackage,
        result: &mut PortableTemplateInstallResult,
    ) -> Result<()> {
        let target = self
            .repository
            .portable_template_snapshot(actor.user_id, actor.current_workspace_id)
            .await?;
        let historical = self
            .repository
            .load_portable_template_identity_map(actor.current_workspace_id)
            .await?;
        result.id_map = historical.clone();
        map_existing_template_identities(package, &target, &mut result.id_map);
        self.map_frontend_installations(actor, package, result)
            .await?;
        let scope = template_baseline_scope(actor.current_workspace_id, package);
        let mut baselines = self.repository.load_template_baselines(&scope).await?;
        for baseline in &baselines {
            if let Some(intent) = &baseline.pending {
                if baseline.committed_operation_id == Some(intent.operation_id) {
                    anyhow::ensure!(
                        self.repository
                            .finalize_template_write(&scope, &baseline.key, intent.operation_id)
                            .await?,
                        "portable_template_receipt_recovery_failed"
                    );
                }
            }
        }
        baselines = self.repository.load_template_baselines(&scope).await?;
        for baseline in &baselines {
            if native_kind(&baseline.key.kind) && baseline.key.kind != "page_visibility" {
                result
                    .id_map
                    .insert(baseline.key.source_id.clone(), baseline.target_id.clone());
            }
        }
        map_existing_template_identities(package, &target, &mut result.id_map);
        let plan = plan_native_template(package, &target, &result.id_map, &baselines, &historical)?;
        self.available = plan
            .iter()
            .filter(|p| {
                p.current_fingerprint.is_some() || p.decision == TemplateMergeDecision::Initialize
            })
            .map(|p| p.desired.key.clone())
            .collect();
        self.allowed = Some(
            plan.iter()
                .filter(|p| {
                    matches!(
                        p.decision,
                        TemplateMergeDecision::Initialize | TemplateMergeDecision::Update
                    )
                })
                .map(|p| p.desired.key.clone())
                .collect(),
        );
        let preview =
            preview_portable_template_with_merge_plan(package, &target, &result.id_map, &plan);
        anyhow::ensure!(
            preview.valid,
            "portable template preflight: {}",
            preview.failures.join("; ")
        );
        self.preflight_visibility(actor, package).await?;
        for item in &plan {
            if let Some(reason) = item.decision.reason() {
                result.skipped.push(PortableTemplateSkippedResource {
                    kind: item.desired.key.kind.clone(),
                    source_id: item.desired.key.source_id.clone(),
                    target_id: Some(item.desired.target_id.clone()),
                    reason: reason.into(),
                });
            }
        }
        self.install_models(actor, package, &target, result)
            .await
            .context("data models")?;
        self.create_applications(actor, package, &target, result)
            .await
            .context("applications")?;
        self.create_pages(actor, package, &target, result)
            .await
            .context("pages")?;
        self.fill_model_fields(actor, package, result)
            .await
            .context("model fields")?;
        self.fill_applications(actor, package, result)
            .await
            .context("application contents")?;
        self.fill_pages(actor, package, result)
            .await
            .context("page contents")?;
        self.install_visibility(actor, package, result)
            .await
            .context("visibility")?;
        let final_target = self
            .repository
            .portable_template_snapshot(actor.user_id, actor.current_workspace_id)
            .await?;
        let current = project_template_resources(&final_target, &BTreeMap::new())?;
        let desired = project_template_resources(package, &result.id_map)?;
        result.created.clear();
        result.updated.clear();
        for item in plan
            .iter()
            .filter(|p| self.can_apply(&p.desired.key.kind, &p.desired.key.source_id))
        {
            let desired = desired
                .iter()
                .find(|r| r.key == item.desired.key)
                .context("portable_template_final_projection_missing")?;
            let actual = current
                .iter()
                .find(|r| r.key.kind == desired.key.kind && r.target_id == desired.target_id)
                .context("portable_template_applied_resource_missing")?;
            let intent = TemplateWriteIntent {
                operation_id: Uuid::now_v7(),
                target_id: desired.target_id.clone(),
                expected_fingerprint: item.current_fingerprint.clone(),
                desired_fingerprint: desired.fingerprint.clone(),
            };
            anyhow::ensure!(
                self.repository
                    .prepare_template_write(&scope, &desired.key, item.baseline_generation, &intent)
                    .await?,
                "portable_template_baseline_concurrent_change"
            );
            anyhow::ensure!(
                self.repository
                    .acknowledge_portable_template_write(
                        &scope,
                        &desired.key,
                        &intent,
                        &actual.fingerprint
                    )
                    .await?,
                "portable_template_owner_receipt_failed"
            );
            anyhow::ensure!(
                self.repository
                    .finalize_template_write(&scope, &desired.key, intent.operation_id)
                    .await?,
                "portable_template_baseline_finalize_failed"
            );
            let resource = PortableTemplateCreatedResource {
                kind: desired.key.kind.clone(),
                source_id: desired.key.source_id.clone(),
                target_id: desired.target_id.clone(),
            };
            if item.decision == TemplateMergeDecision::Initialize {
                result.created.push(resource);
            } else {
                result.updated.push(resource);
            }
        }
        Ok(())
    }
}
