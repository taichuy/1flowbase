use super::*;

impl PgControlPlaneStore {
    pub(super) async fn upsert_artifact_instance_impl(
        &self,
        input: &UpsertPluginArtifactInstanceInput,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        let row = sqlx::query(
            r#"
            insert into extension_artifact_instances (
                node_id,
                installation_id,
                local_version,
                local_checksum,
                local_path,
                package_path,
                manifest_fingerprint,
                artifact_status,
                runtime_status,
                availability_status,
                checked_at,
                last_error,
                is_current
            ) values (
                $1, $2, $3, $4, $5, $6, $7,
                $8, $9, $10, $11, $12, $13
            )
            on conflict (node_id, installation_id) do update
            set
                local_version = excluded.local_version,
                local_checksum = excluded.local_checksum,
                local_path = excluded.local_path,
                package_path = excluded.package_path,
                manifest_fingerprint = excluded.manifest_fingerprint,
                artifact_status = excluded.artifact_status,
                runtime_status = excluded.runtime_status,
                availability_status = excluded.availability_status,
                checked_at = excluded.checked_at,
                last_error = excluded.last_error,
                is_current = excluded.is_current
            returning
                node_id,
                installation_id,
                local_version,
                local_checksum,
                local_path,
                package_path,
                manifest_fingerprint,
                artifact_status,
                runtime_status,
                availability_status,
                checked_at,
                last_error,
                is_current
            "#,
        )
        .bind(&input.node_id)
        .bind(input.installation_id)
        .bind(input.local_version.as_deref())
        .bind(input.local_checksum.as_deref())
        .bind(input.local_path.as_deref())
        .bind(input.package_path.as_deref())
        .bind(input.manifest_fingerprint.as_deref())
        .bind(input.artifact_status.as_str())
        .bind(input.runtime_status.as_str())
        .bind(input.availability_status.as_str())
        .bind(input.checked_at)
        .bind(input.last_error.as_deref())
        .bind(input.is_current)
        .fetch_one(self.pool())
        .await?;

        map_artifact_instance(row)
    }

    pub(super) async fn select_network_egress_current_impl(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginArtifactInstanceRecord>> {
        let mut transaction = self.pool().begin().await?;
        let target = sqlx::query(
            r#"
            select installation.category, installation.organization,
                   installation.artifact_id as provider_code
            from extension_installations installation
            join extension_artifact_instances artifact
              on artifact.installation_id = installation.id and artifact.node_id = $1
            where installation.id = $2
              and installation.contract_version = '1flowbase.network_egress_provider/v1'
              and installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
              and artifact.artifact_status = 'ready'
            for update of installation, artifact
            "#,
        )
        .bind(node_id)
        .bind(installation_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(target) = target else {
            transaction.rollback().await?;
            return Ok(None);
        };
        let category: String = target.get("category");
        let organization: String = target.get("organization");
        let provider_code: String = target.get("provider_code");
        let family_lock =
            format!("network-egress-current:{category}:{organization}:{provider_code}");
        sqlx::query("select pg_advisory_xact_lock(hashtext($1))")
            .bind(family_lock)
            .execute(&mut *transaction)
            .await?;
        sqlx::query(
            r#"
            update extension_artifact_instances artifact
            set is_current = false, checked_at = now()
            from extension_installations installation
            where installation.id = artifact.installation_id
              and artifact.node_id = $1
              and installation.category = $2
              and installation.organization = $3
              and installation.artifact_id = $4
              and installation.metadata_json ->> 'plugin_type' = 'network_egress_provider'
              and artifact.is_current
            "#,
        )
        .bind(node_id)
        .bind(&category)
        .bind(&organization)
        .bind(&provider_code)
        .execute(&mut *transaction)
        .await?;
        let artifact = sqlx::query(
            r#"
            update extension_artifact_instances
            set is_current = true, checked_at = now()
            where node_id = $1 and installation_id = $2
            returning node_id, installation_id, local_version, local_checksum, local_path,
                package_path, manifest_fingerprint, artifact_status, runtime_status,
                availability_status, checked_at, last_error, is_current
            "#,
        )
        .bind(node_id)
        .bind(installation_id)
        .fetch_one(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(Some(map_artifact_instance(artifact)?))
    }

    pub(super) async fn commit_plugin_family_uninstall_impl(
        &self,
        input: &CommitPluginFamilyUninstallInput,
    ) -> Result<()> {
        let mut transaction = self.pool().begin().await?;
        lock_managed_dependency_scope(&mut transaction).await?;
        let removed: Vec<_> = input
            .artifact_instances
            .iter()
            .map(|artifact| artifact.installation_id)
            .collect();
        let sorted: std::collections::BTreeSet<_> = removed.iter().copied().collect();
        for installation_id in sorted {
            super::lock_managed_desired_state_scopes(&mut transaction, installation_id).await?;
        }
        managed_dependency_facts(&mut transaction)
            .await?
            .guard_removal(&removed, None, None)?;
        for artifact in &input.artifact_instances {
            sqlx::query(
                r#"
                insert into extension_artifact_instances (
                    node_id,
                    installation_id,
                    local_version,
                    local_checksum,
                    local_path,
                    package_path,
                    manifest_fingerprint,
                    artifact_status,
                    runtime_status,
                    availability_status,
                    checked_at,
                    last_error,
                    is_current
                ) values (
                    $1, $2, $3, $4, $5, $6, $7,
                    $8, $9, $10, $11, $12, $13
                )
                on conflict (node_id, installation_id) do update
                set
                    local_version = excluded.local_version,
                    local_checksum = excluded.local_checksum,
                    local_path = excluded.local_path,
                    package_path = excluded.package_path,
                    manifest_fingerprint = excluded.manifest_fingerprint,
                    artifact_status = excluded.artifact_status,
                    runtime_status = excluded.runtime_status,
                    availability_status = excluded.availability_status,
                    checked_at = excluded.checked_at,
                    last_error = excluded.last_error,
                    is_current = excluded.is_current
                "#,
            )
            .bind(&artifact.node_id)
            .bind(artifact.installation_id)
            .bind(artifact.local_version.as_deref())
            .bind(artifact.local_checksum.as_deref())
            .bind(artifact.local_path.as_deref())
            .bind(artifact.package_path.as_deref())
            .bind(artifact.manifest_fingerprint.as_deref())
            .bind(artifact.artifact_status.as_str())
            .bind(artifact.runtime_status.as_str())
            .bind(artifact.availability_status.as_str())
            .bind(artifact.checked_at)
            .bind(artifact.last_error.as_deref())
            .bind(artifact.is_current)
            .execute(&mut *transaction)
            .await?;
        }
        // Family uninstall retains installation/audit history but revokes reusable projections.
        let installation_ids: Vec<_> = input
            .artifact_instances
            .iter()
            .map(|artifact| artifact.installation_id)
            .collect();
        sqlx::query("delete from ui_component_records where origin='plugin' and owner_installation_id=any($1)")
            .bind(&installation_ids).execute(&mut *transaction).await?;
        sqlx::query("delete from frontend_block_catalog where installation_id=any($1)")
            .bind(&installation_ids)
            .execute(&mut *transaction)
            .await?;
        for cleanup in &input.artifact_cleanups {
            sqlx::query(
                r#"
                insert into plugin_artifact_cleanup_jobs (
                    id,
                    node_id,
                    provider_code,
                    tombstone_path,
                    created_at
                ) values ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(cleanup.cleanup_id)
            .bind(&cleanup.node_id)
            .bind(&cleanup.provider_code)
            .bind(&cleanup.tombstone_path)
            .bind(cleanup.created_at)
            .execute(&mut *transaction)
            .await?;
        }
        let event = &input.audit_log;
        sqlx::query(
            r#"
            insert into audit_logs (
                id,
                workspace_id,
                scope_id,
                actor_user_id,
                target_type,
                target_id,
                event_code,
                payload,
                created_by,
                updated_by,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $4, $4, $9, $9)
            "#,
        )
        .bind(event.id)
        .bind(event.workspace_id)
        .bind(event.workspace_id.unwrap_or(domain::SYSTEM_SCOPE_ID))
        .bind(event.actor_user_id)
        .bind(&event.target_type)
        .bind(event.target_id)
        .bind(&event.event_code)
        .bind(&event.payload)
        .bind(event.created_at)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub(super) async fn list_plugin_artifact_cleanups_impl(
        &self,
        node_id: &str,
    ) -> Result<Vec<domain::PluginArtifactCleanupRecord>> {
        let rows = sqlx::query(
            r#"
            select
                id,
                node_id,
                provider_code,
                tombstone_path,
                created_at,
                last_error,
                last_attempt_at
            from plugin_artifact_cleanup_jobs
            where node_id = $1
            order by created_at asc, id asc
            "#,
        )
        .bind(node_id)
        .fetch_all(self.pool())
        .await?;
        Ok(rows.into_iter().map(map_artifact_cleanup).collect())
    }

    pub(super) async fn complete_plugin_artifact_cleanup_impl(
        &self,
        cleanup_id: Uuid,
    ) -> Result<()> {
        sqlx::query("delete from plugin_artifact_cleanup_jobs where id = $1")
            .bind(cleanup_id)
            .execute(self.pool())
            .await?;
        Ok(())
    }

    pub(super) async fn record_plugin_artifact_cleanup_failure_impl(
        &self,
        input: &RecordPluginArtifactCleanupFailureInput,
    ) -> Result<()> {
        sqlx::query(
            r#"
            update plugin_artifact_cleanup_jobs
            set last_error = $2, last_attempt_at = $3
            where id = $1
            "#,
        )
        .bind(input.cleanup_id)
        .bind(&input.last_error)
        .bind(input.attempted_at)
        .execute(self.pool())
        .await?;
        Ok(())
    }
}

/// The nil workspace serializes cross-workspace provider/consumer lifecycle changes with
/// system authority leases. Acquire it BEFORE other workspace locks and installation rows.
pub(crate) async fn lock_managed_dependency_scope(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<()> {
    crate::plugin_contribution_authority_repository::lock_managed_workspace(
        transaction,
        domain::SYSTEM_SCOPE_ID,
    )
    .await
}

#[derive(Clone, Default)]
pub(super) struct ManagedDependencyFacts {
    installations: std::collections::BTreeMap<Uuid, domain::PluginInstallationRecord>,
    // Only ready artifacts and actual authorization/selection scopes; never other-node fallback.
    contexts: std::collections::BTreeMap<Uuid, std::collections::BTreeSet<(String, Uuid)>>,
    selected_contexts: std::collections::BTreeMap<Uuid, std::collections::BTreeSet<(String, Uuid)>>,
}
pub(super) async fn managed_dependency_facts(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<ManagedDependencyFacts> {
    let rows = sqlx::query("select i.*,i.artifact_id as provider_code,i.artifact_version as plugin_version,i.receipt ->> 'legacy_manifest_compatibility' as legacy_manifest_compatibility from extension_installations i where i.plugin_id is not null and i.contract_version='1flowbase.extension-bus/v1'")
        .fetch_all(&mut **transaction).await?;
    let installations = rows
        .into_iter()
        .map(map_installation)
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .map(|i| (i.id, i))
        .collect();
    let rows = sqlx::query("select a.installation_id,a.node_id,s.workspace_id,(s.workspace_id='00000000-0000-0000-0000-000000000000'::uuid or exists(select 1 from plugin_assignments p where p.installation_id=i.id and p.workspace_id=s.workspace_id)) as selected from extension_artifact_instances a join extension_installations i on i.id=a.installation_id cross join lateral (select workspace_id from plugin_assignments where installation_id=i.id union select workspace_id from plugin_contribution_authorization_revisions where installation_id=i.id union select '00000000-0000-0000-0000-000000000000'::uuid where coalesce(i.metadata_json #>> '{managed_service,scope}' = 'system' or i.metadata_json -> 'binding_targets' = '[\"system\"]'::jsonb,false)) s where i.contract_version='1flowbase.extension-bus/v1' and a.artifact_status='ready' and a.local_version=i.artifact_version")
        .fetch_all(&mut **transaction).await?;
    let mut facts = ManagedDependencyFacts {
        installations,
        ..Default::default()
    };
    for row in rows {
        let context = (row.get("node_id"), row.get("workspace_id"));
        facts
            .contexts
            .entry(row.get("installation_id"))
            .or_default()
            .insert(context.clone());
        if row.get::<bool, _>("selected") {
            facts
                .selected_contexts
                .entry(row.get("installation_id"))
                .or_default()
                .insert(context);
        }
    }
    Ok(facts)
}
fn dependency_metadata<T: serde::de::DeserializeOwned>(
    i: &domain::PluginInstallationRecord,
    key: &str,
) -> Result<Vec<T>> {
    i.metadata_json
        .get(key)
        .map(|value| serde_json::from_value(value.clone()).map_err(Into::into))
        .unwrap_or_else(|| Ok(Vec::new()))
}
fn dependency_identity(i: &domain::PluginInstallationRecord) -> String {
    format!(
        "{}/{}@{}",
        i.organization, i.provider_code, i.plugin_version
    )
}
fn dependency_matches(
    i: &domain::PluginInstallationRecord,
    p: &extension_contracts::PluginDependency,
) -> bool {
    i.organization == p.publisher_namespace && i.provider_code == p.plugin_code
}
fn dependency_failure(
    code: &str,
    p: &extension_contracts::PluginDependency,
    resource: Option<&str>,
    dependents: Vec<String>,
) -> anyhow::Error {
    ControlPlaneError::PluginDependency {
        code: code.into(),
        provider_publisher_namespace: p.publisher_namespace.clone(),
        provider_plugin_code: p.plugin_code.clone(),
        resource_code: resource.map(str::to_owned),
        dependents,
    }
    .into()
}
fn installation_dependency(
    i: &domain::PluginInstallationRecord,
) -> extension_contracts::PluginDependency {
    extension_contracts::PluginDependency {
        publisher_namespace: i.organization.clone(),
        plugin_code: i.provider_code.clone(),
    }
}
impl ManagedDependencyFacts {
    fn references(
        &self,
        consumer: &domain::PluginInstallationRecord,
        provider: &domain::PluginInstallationRecord,
    ) -> Result<bool> {
        Ok(
            dependency_metadata::<extension_contracts::PluginDependency>(
                consumer,
                "plugin_dependencies",
            )?
            .iter()
            .any(|p| dependency_matches(provider, p))
                || dependency_metadata::<extension_contracts::PluginResourceImport>(
                    consumer,
                    "resource_imports",
                )?
                .iter()
                .any(|r| dependency_matches(provider, &r.provider)),
        )
    }
    fn provider<'a>(
        &'a self,
        consumer: &domain::PluginInstallationRecord,
        dependency: &extension_contracts::PluginDependency,
        node: &str,
        workspace: Uuid,
    ) -> Result<&'a domain::PluginInstallationRecord> {
        let mut providers = self.installations.values().filter(|p| {
            p.id != consumer.id
                && dependency_matches(p, dependency)
                && p.desired_state == domain::PluginDesiredState::ActiveRequested
                && self.selected_contexts.get(&p.id).is_some_and(|contexts| {
                    contexts.contains(&(node.to_owned(), domain::SYSTEM_SCOPE_ID))
                        || (workspace != domain::SYSTEM_SCOPE_ID
                            && contexts.contains(&(node.to_owned(), workspace)))
                })
        });
        let provider = providers.next().ok_or_else(|| {
            dependency_failure(
                "plugin_dependency_unavailable",
                dependency,
                None,
                vec![dependency_identity(consumer)],
            )
        })?;
        if providers.next().is_some() || dependency_matches(consumer, dependency) {
            return Err(dependency_failure(
                "plugin_dependency_ambiguous",
                dependency,
                None,
                vec![dependency_identity(consumer)],
            ));
        }
        Ok(provider)
    }
    // This validates the durable declaration contract, NOT grants or runtime permission scopes.
    // The resource owner resolves actual permissions under its authority lease.
    fn validate(
        &self,
        consumer: &domain::PluginInstallationRecord,
        context: Option<(&str, Uuid)>,
    ) -> Result<()> {
        let dependencies = dependency_metadata::<extension_contracts::PluginDependency>(
            consumer,
            "plugin_dependencies",
        )?;
        let imports = dependency_metadata::<extension_contracts::PluginResourceImport>(
            consumer,
            "resource_imports",
        )?;
        if dependencies.is_empty() && imports.is_empty() {
            return Ok(());
        }
        let contexts = match context {
            Some((node, workspace)) => {
                std::collections::BTreeSet::from([(node.to_owned(), workspace)])
            }
            None => self.contexts.get(&consumer.id).cloned().unwrap_or_default(),
        };
        if contexts.is_empty() {
            let dependency = dependencies
                .first()
                .cloned()
                .unwrap_or_else(|| imports[0].provider.clone());
            return Err(dependency_failure(
                "plugin_dependency_unavailable",
                &dependency,
                None,
                vec![dependency_identity(consumer)],
            ));
        }
        for (node, workspace) in contexts {
            for dependency in &dependencies {
                self.provider(consumer, dependency, &node, workspace)?;
            }
            for import in &imports {
                let denied = || {
                    dependency_failure(
                        "plugin_resource_import_denied",
                        &import.provider,
                        Some(&import.resource_code),
                        vec![dependency_identity(consumer)],
                    )
                };
                if !dependencies.contains(&import.provider) {
                    return Err(denied());
                }
                let provider = self.provider(consumer, &import.provider, &node, workspace)?;
                let exports = dependency_metadata::<extension_contracts::PluginResourceExport>(
                    provider,
                    "resource_exports",
                )?;
                let mut matches = exports
                    .iter()
                    .filter(|e| e.resource_code == import.resource_code);
                let export = matches.next().ok_or_else(denied)?;
                if matches.next().is_some() {
                    return Err(denied());
                }
                let models = dependency_metadata::<extension_contracts::PluginDataModelContribution>(
                    provider,
                    "data_models",
                )?;
                for collection in &import.collections {
                    let mut matches = export
                        .collections
                        .iter()
                        .filter(|c| c.collection_code == collection.collection_code);
                    let offered = matches.next().ok_or_else(denied)?;
                    if matches.next().is_some()
                        || !offered.access.allows(collection.access)
                        || !models
                            .iter()
                            .flat_map(|m| &m.owned_collections)
                            .any(|c| c.collection_code == collection.collection_code)
                    {
                        return Err(denied());
                    }
                }
                if import.credentials.is_some_and(|requested| {
                    !export
                        .credentials
                        .is_some_and(|offered| offered.allows(requested))
                }) {
                    return Err(denied());
                }
                for code in &import.record_resources {
                    let mut bindings = export
                        .record_resources
                        .iter()
                        .filter(|r| &r.resource_code == code);
                    let binding = bindings.next().ok_or_else(denied)?;
                    if bindings.next().is_some()
                        || binding.id_field.is_empty()
                        || !import
                            .collections
                            .iter()
                            .any(|c| c.collection_code == binding.collection_code)
                    {
                        return Err(denied());
                    }
                }
                if export.record_resources.iter().any(|r| {
                    import
                        .collections
                        .iter()
                        .any(|c| c.collection_code == r.collection_code)
                        && !import.record_resources.contains(&r.resource_code)
                }) {
                    return Err(denied());
                }
            }
        }
        Ok(())
    }
    fn guard_provider_activation(&self, provider: &domain::PluginInstallationRecord) -> Result<()> {
        let mut projected = self.clone();
        if let Some(candidate) = projected.installations.get_mut(&provider.id) {
            candidate.desired_state = domain::PluginDesiredState::ActiveRequested;
        }
        let mut blocked = Vec::new();
        for consumer in self.installations.values().filter(|i| {
            i.id != provider.id && i.desired_state == domain::PluginDesiredState::ActiveRequested
        }) {
            if self.references(consumer, provider)? && projected.validate(consumer, None).is_err() {
                blocked.push(dependency_identity(consumer));
            }
        }
        if !blocked.is_empty() {
            return Err(dependency_failure(
                "plugin_dependency_in_use",
                &installation_dependency(provider),
                None,
                blocked,
            ));
        }
        Ok(())
    }
    pub(super) fn guard_removal(
        &self,
        removed: &[Uuid],
        replacement: Option<Uuid>,
        context: Option<(&str, Uuid)>,
    ) -> Result<()> {
        for provider_id in removed {
            let Some(provider) = self.installations.get(provider_id) else {
                continue;
            };
            let mut projected = self.clone();
            for id in removed {
                projected.installations.remove(id);
            }
            if let Some(candidate) = replacement.and_then(|id| projected.installations.get_mut(&id))
            {
                candidate.desired_state = domain::PluginDesiredState::ActiveRequested;
            }
            let mut blocked = Vec::new();
            let mut resource = None;
            for consumer in self.installations.values().filter(|i| {
                !removed.contains(&i.id)
                    && Some(i.id) != replacement
                    && i.desired_state == domain::PluginDesiredState::ActiveRequested
            }) {
                if !self.references(consumer, provider)? {
                    continue;
                }
                // A workspace switch only changes that selection, not other workspaces.
                if let Some((node, workspace)) = context {
                    if !self
                        .contexts
                        .get(&consumer.id)
                        .is_some_and(|c| c.contains(&(node.to_owned(), workspace)))
                        && workspace != domain::SYSTEM_SCOPE_ID
                    {
                        continue;
                    }
                }
                let validation = projected.validate(
                    consumer,
                    context.filter(|(_, workspace)| *workspace != domain::SYSTEM_SCOPE_ID),
                );
                let binding_changed = if let Some(candidate) =
                    replacement.and_then(|id| projected.installations.get(&id))
                {
                    let old = dependency_metadata::<extension_contracts::PluginResourceExport>(
                        provider,
                        "resource_exports",
                    )?;
                    let new = dependency_metadata::<extension_contracts::PluginResourceExport>(
                        candidate,
                        "resource_exports",
                    )?;
                    dependency_metadata::<extension_contracts::PluginResourceImport>(
                        consumer,
                        "resource_imports",
                    )?
                    .iter()
                    .filter(|i| dependency_matches(provider, &i.provider))
                    .any(|import| {
                        import.record_resources.iter().any(|code| {
                            old.iter()
                                .find(|e| e.resource_code == import.resource_code)
                                .and_then(|e| {
                                    e.record_resources.iter().find(|r| &r.resource_code == code)
                                })
                                != new
                                    .iter()
                                    .find(|e| e.resource_code == import.resource_code)
                                    .and_then(|e| {
                                        e.record_resources.iter().find(|r| &r.resource_code == code)
                                    })
                        })
                    })
                } else {
                    false
                };
                if let Err(error) = &validation {
                    if let Some(ControlPlaneError::PluginDependency { resource_code, .. }) =
                        error.downcast_ref::<ControlPlaneError>()
                    {
                        resource = resource_code.clone();
                    }
                }
                if validation.is_err() || binding_changed {
                    blocked.push(dependency_identity(consumer));
                }
            }
            if !blocked.is_empty() {
                blocked.sort();
                blocked.dedup();
                return Err(dependency_failure(
                    "plugin_dependency_in_use",
                    &installation_dependency(provider),
                    resource.as_deref(),
                    blocked,
                ));
            }
        }
        Ok(())
    }
}
pub(super) async fn guard_managed_desired_state(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    installation_id: Uuid,
    desired_state: domain::PluginDesiredState,
) -> Result<()> {
    let facts = managed_dependency_facts(transaction).await?;
    if let Some(installation) = facts.installations.get(&installation_id) {
        if desired_state == domain::PluginDesiredState::ActiveRequested {
            facts.validate(installation, None)?;
            facts.guard_provider_activation(installation)?;
        } else if installation.desired_state == domain::PluginDesiredState::ActiveRequested {
            facts.guard_removal(&[installation_id], None, None)?;
        }
    }
    Ok(())
}
/// Caller holds nil + relevant workspace locks, before candidate publication or any effects.
pub(crate) async fn validate_managed_dependency_switch(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    current: Uuid,
    target: Uuid,
    node_id: &str,
    workspace_id: Uuid,
) -> Result<()> {
    let mut facts = managed_dependency_facts(transaction).await?;
    facts
        .contexts
        .entry(target)
        .or_default()
        .insert((node_id.to_owned(), workspace_id));
    facts
        .selected_contexts
        .entry(target)
        .or_default()
        .insert((node_id.to_owned(), workspace_id));
    if let Some(candidate) = facts.installations.get(&target) {
        facts.validate(candidate, Some((node_id, workspace_id)))?;
    }
    facts.guard_removal(&[current], Some(target), Some((node_id, workspace_id)))
}

#[cfg(test)]
#[path = "_tests/dependencies.rs"]
mod dependency_tests;

#[cfg(test)]
#[path = "_tests/dependency_transactions.rs"]
mod dependency_transaction_tests;
