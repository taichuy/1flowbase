use std::collections::BTreeSet;

use anyhow::{Result, anyhow};
use control_plane::{
    ports::{RoleConsolePolicyMigrationRehearsalInput, RoleConsolePolicyMigrationRepository},
    role::console_policy_migration::{
        ConsolePolicyMigrationActorProbeSet, ConsolePolicyMigrationActorRoleBinding,
        ConsolePolicyMigrationPreview, ConsolePolicyMigrationProbe,
        ConsolePolicyMigrationProbeKind, preview_console_policy_migration_actor_authorizations,
    },
};
use domain::ConsoleOperationId;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::{
    app_state::compile_console_boot_plan,
    config::ApiConfig,
    host_extension_loader::prepare_host_extensions_at_startup,
    host_extensions::{
        builtin::load_builtin_host_extension_manifests,
        console::{
            linked_host_console_route_sources, resolve_linked_host_extension_console_contribution,
        },
    },
};

use super::{
    CompiledCoreConsolePolicyMigration, compile_core_console_policy_migration_plan,
    crosswalk::LIVE_CORE_MIGRATION_SOURCE_CONTRACT, report::ConsolePolicyMigrationUnknownGrant,
};

pub(super) struct LiveConsolePolicyMigrationContext {
    pub(super) store: storage_durable::MainDurableStore,
    pub(super) migration: CompiledCoreConsolePolicyMigration,
}

pub(super) struct LiveConsolePolicyMigrationPreview {
    pub(super) role_projections: Vec<Value>,
    pub(super) actor_previews: Vec<Value>,
    pub(super) unknown_grants: Vec<ConsolePolicyMigrationUnknownGrant>,
    pub(super) authorization_deltas: Vec<Value>,
    pub(super) validation_errors: Vec<String>,
    pub(super) rehearsal: Option<RoleConsolePolicyMigrationRehearsalInput>,
}

pub(super) async fn load_live_context(
    config: &ApiConfig,
) -> Result<LiveConsolePolicyMigrationContext> {
    let durable = storage_durable::build_main_durable_postgres_with_max_connections(
        &config.database_url,
        config.database_pool_max_connections,
    )
    .await?;
    let store = durable.store;
    let builtin_host_extensions =
        load_builtin_host_extension_manifests(crate::api_workspace_root()?)?;
    let mut prepared_host_extensions = prepare_host_extensions_at_startup(
        &store,
        &config.api_node_id,
        &config.provider_install_root,
        &config.host_extension_dropin_root,
        config.allow_unverified_filesystem_dropins,
    )
    .await?;
    let mut console_host_extensions = builtin_host_extensions
        .iter()
        .map(|(_, contribution)| {
            resolve_linked_host_extension_console_contribution(
                contribution.clone(),
                linked_host_console_route_sources(),
            )
        })
        .collect::<Result<Vec<_>>>()?;
    console_host_extensions.extend(prepared_host_extensions.take_contributions());
    let boot = compile_console_boot_plan(console_host_extensions)?;
    let migration =
        compile_core_console_policy_migration_plan(boot.console_operation_registry.inventory())?;

    Ok(LiveConsolePolicyMigrationContext { store, migration })
}

pub(super) async fn preview_live_migration(
    store: &storage_durable::MainDurableStore,
    migration: &CompiledCoreConsolePolicyMigration,
    run_id: Uuid,
) -> Result<LiveConsolePolicyMigrationPreview> {
    let inventories = store
        .list_role_console_policy_migration_grants(migration.source())
        .await?;
    let known_grants = migration
        .legacy_mappings()
        .iter()
        .map(|mapping| mapping.legacy_grant.as_str())
        .collect::<BTreeSet<_>>();
    let mut unknown_grants = Vec::new();
    let mut previews = Vec::<ConsolePolicyMigrationPreview>::new();
    let mut role_projections = Vec::new();
    let mut authorization_deltas = Vec::new();

    for inventory in &inventories {
        let unknown_for_role = inventory
            .source_grants
            .iter()
            .filter(|grant| !known_grants.contains(grant.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for grant in &unknown_for_role {
            unknown_grants.push(ConsolePolicyMigrationUnknownGrant {
                role_id: inventory.role_id.to_string(),
                workspace_id: inventory.workspace_id.to_string(),
                role_code: inventory.role_code.clone(),
                grant: grant.clone(),
            });
        }
        if !unknown_for_role.is_empty() {
            role_projections.push(serde_json::json!({
                "role_id": inventory.role_id,
                "workspace_id": inventory.workspace_id,
                "role_code": inventory.role_code,
                "source_grants": inventory.source_grants,
                "unknown_grants": unknown_for_role,
            }));
            continue;
        }
        let preview = migration
            .plan()
            .project_legacy_role(inventory.role_id, &inventory.source_grants)
            .map_err(|error| {
                anyhow!(
                    "cannot project legacy console grants for role {}: {error}",
                    inventory.role_code
                )
            })?;
        if !preview.authorization_delta.added.is_empty()
            || !preview.authorization_delta.removed.is_empty()
            || !preview.effective_delta.is_empty()
        {
            authorization_deltas.push(serde_json::json!({
                "role_id": inventory.role_id,
                "workspace_id": inventory.workspace_id,
                "role_code": inventory.role_code,
                "authorization_delta": preview.authorization_delta,
                "effective_delta": preview.effective_delta,
            }));
        }
        role_projections.push(serde_json::json!({
            "role_id": inventory.role_id,
            "workspace_id": inventory.workspace_id,
            "role_code": inventory.role_code,
            "source_grants": inventory.source_grants,
            "preview": preview,
        }));
        previews.push(preview);
    }

    let mut validation_errors = Vec::new();
    if !unknown_grants.is_empty() {
        validation_errors.push("unknown legacy grants stop migration before rehearsal".to_string());
    }
    if !authorization_deltas.is_empty() {
        validation_errors.push("authorization delta stops migration before rehearsal".to_string());
    }
    if previews.is_empty() && inventories.is_empty() {
        validation_errors
            .push("no workspace role grants are available for migration rehearsal".to_string());
    }
    if !validation_errors.is_empty() {
        return Ok(LiveConsolePolicyMigrationPreview {
            role_projections,
            actor_previews: Vec::new(),
            unknown_grants,
            authorization_deltas,
            validation_errors,
            rehearsal: None,
        });
    }

    let role_ids = previews
        .iter()
        .map(|preview| preview.policy.role_id())
        .collect::<Vec<_>>();
    let actor_bindings = live_actor_role_bindings(store, &role_ids).await?;
    let probes = default_five_probes()?;
    let actor_probe_sets = actor_bindings
        .into_iter()
        .map(|binding| ConsolePolicyMigrationActorProbeSet {
            binding,
            probes: probes.clone(),
        })
        .collect::<Vec<_>>();
    let actor_previews = preview_console_policy_migration_actor_authorizations(
        migration.plan(),
        &actor_probe_sets,
        &previews,
    )
    .map_err(|error| anyhow!("cannot build actor five-probe migration matrix: {error}"))?;
    if actor_previews
        .iter()
        .any(|preview| !preview.effective_delta.is_empty())
    {
        validation_errors
            .push("actor multi-role five-probe delta stops migration before rehearsal".to_string());
    }
    let actor_preview_values = actor_previews
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let rehearsal =
        validation_errors
            .is_empty()
            .then(|| RoleConsolePolicyMigrationRehearsalInput {
                run_id,
                source_contract: LIVE_CORE_MIGRATION_SOURCE_CONTRACT.to_string(),
                source: migration.source().clone(),
                plan: migration.plan().clone(),
                previews,
                actor_previews,
            });

    Ok(LiveConsolePolicyMigrationPreview {
        role_projections,
        actor_previews: actor_preview_values,
        unknown_grants,
        authorization_deltas,
        validation_errors,
        rehearsal,
    })
}

async fn live_actor_role_bindings(
    store: &storage_durable::MainDurableStore,
    role_ids: &[Uuid],
) -> Result<Vec<ConsolePolicyMigrationActorRoleBinding>> {
    if role_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        r#"
        select binding.user_id as actor_user_id,
               array_agg(binding.role_id order by binding.role_id) as role_ids
        from user_role_bindings binding
        where binding.role_id = any($1::uuid[])
        group by binding.user_id
        order by binding.user_id
        "#,
    )
    .bind(role_ids)
    .fetch_all(store.pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| ConsolePolicyMigrationActorRoleBinding {
            actor_user_id: row.get("actor_user_id"),
            role_ids: row.get("role_ids"),
        })
        .collect())
}

fn default_five_probes() -> Result<Vec<ConsolePolicyMigrationProbe>> {
    let operation = |value: &str| {
        ConsoleOperationId::try_from(value)
            .map_err(|_| anyhow!("invalid audited five-probe operation {value}"))
    };
    Ok(vec![
        ConsolePolicyMigrationProbe {
            operation_id: operation("settings_feature.access.system.applications")?,
            kind: ConsolePolicyMigrationProbeKind::Simple,
        },
        ConsolePolicyMigrationProbe {
            operation_id: operation("applications.create")?,
            kind: ConsolePolicyMigrationProbeKind::Create,
        },
        ConsolePolicyMigrationProbe {
            operation_id: operation("applications.view")?,
            kind: ConsolePolicyMigrationProbeKind::OwnRow,
        },
        ConsolePolicyMigrationProbe {
            operation_id: operation("applications.view")?,
            kind: ConsolePolicyMigrationProbeKind::SameScopeOther,
        },
        ConsolePolicyMigrationProbe {
            operation_id: operation("applications.view")?,
            kind: ConsolePolicyMigrationProbeKind::CrossScope,
        },
    ])
}
