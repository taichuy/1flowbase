//! Cross-template tool ownership, stable forks, and atomic binding redirection.
use super::*;
use control_plane::{
    mcp_bundle::{ExportMcpBundleCommand, ImportMcpBundleCommand},
    mcp_management::{
        CreateMcpToolBindingCommand, McpManagementService, UpdateMcpToolBindingCommand,
        UpdateMcpToolCommand,
    },
};
use serde_json::Value;
use storage_durable_postgres::PgControlPlaneStore;

const ORIGINAL: &str = "shared_template_tool";
const INSTANCE: &str = "template_b";

async fn source_fixture(store: &PgControlPlaneStore, actor: Uuid) -> domain::McpBundlePackage {
    let service = McpManagementService::new(store.clone());
    let mut package = mcp_template_fixture();
    package.instances[0].instance_id = "existing_a".into();
    let imported = service
        .import_bundle(ImportMcpBundleCommand {
            actor_user_id: actor,
            package,
            interface_catalog: mcp_template_catalog(),
            current_system_version: "1.0.0".into(),
        })
        .await
        .unwrap();
    assert_eq!(imported.effect_summary.failed, 0);
    assert_eq!(imported.effect_summary.conflicts, 0);
    // Export through the owner so equality includes its canonical schema/mapping projection.
    let mut source = service
        .export_bundle(ExportMcpBundleCommand {
            actor_user_id: actor,
            organization: "test".into(),
            bundle_id: "fork-fixture".into(),
            bundle_version: "1.0.0".into(),
            locale: "en_US".into(),
            current_system_version: "1.0.0".into(),
        })
        .await
        .unwrap();
    source
        .instances
        .retain(|instance| instance.instance_id == "existing_a");
    source.tools.retain(|tool| tool.tool_id == ORIGINAL);
    source.instances[0].instance_id = INSTANCE.into();
    source.instances[0].name = "Template B".into();
    source
}

fn add_group(package: &mut domain::McpBundlePackage, path: &str) {
    let mut group = package.instances[0].groups[0].clone();
    group.path = path.into();
    let mut binding = package.instances[0].bindings[0].clone();
    binding.group_path = path.into();
    package.instances[0].groups.push(group);
    package.instances[0].bindings.push(binding);
}

async fn install(
    store: &PgControlPlaneStore,
    scope: &TemplateBaselineScope,
    actor: Uuid,
    package: &domain::McpBundlePackage,
) -> mcp_merge::McpMergeOutcome {
    let result = mcp_merge::install(
        store,
        scope,
        actor,
        package,
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(result.failures.is_empty(), "{:?}", result.failures);
    result
}

fn created_fork(result: &mcp_merge::McpMergeOutcome) -> String {
    let fork = result
        .created
        .iter()
        .find(|item| item.kind == "mcp_tool" && item.source_id == ORIGINAL)
        .expect("conflicting unowned tool must create a template-owned copy");
    assert_ne!(fork.target_id, ORIGINAL);
    fork.target_id.clone()
}

async fn tool_row(store: &PgControlPlaneStore, workspace: Uuid, tool: &str) -> Value {
    sqlx::query_scalar("select to_jsonb(t) from mcp_tools t where workspace_id=$1 and tool_id=$2")
        .bind(workspace)
        .bind(tool)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn binding_rows(
    store: &PgControlPlaneStore,
    workspace: Uuid,
    instance: &str,
) -> Vec<(Uuid, String, String, Option<String>)> {
    sqlx::query_as("select b.id,b.group_path,t.tool_id,b.display_alias from mcp_tool_bindings b join mcp_instances i on i.id=b.instance_record_id join mcp_tools t on t.id=b.tool_record_id where i.workspace_id=$1 and i.instance_id=$2 order by b.group_path,t.tool_id")
        .bind(workspace).bind(instance).fetch_all(store.pool()).await.unwrap()
}

async fn existing_instance_snapshot(store: &PgControlPlaneStore, workspace: Uuid) -> Value {
    sqlx::query_scalar("select jsonb_build_object('instance',to_jsonb(i),'groups',(select coalesce(jsonb_agg(to_jsonb(g) order by id),'[]') from mcp_groups g where instance_record_id=i.id),'bindings',(select coalesce(jsonb_agg(to_jsonb(b) order by id),'[]') from mcp_tool_bindings b where instance_record_id=i.id)) from mcp_instances i where workspace_id=$1 and instance_id='existing_a'")
        .bind(workspace).fetch_one(store.pool()).await.unwrap()
}

async fn database_snapshot(store: &PgControlPlaneStore, workspace: Uuid) -> Value {
    sqlx::query_scalar("select jsonb_build_object('tools',(select coalesce(jsonb_agg(to_jsonb(t) order by id),'[]') from mcp_tools t where workspace_id=$1),'bindings',(select coalesce(jsonb_agg(to_jsonb(b) order by b.id),'[]') from mcp_tool_bindings b where scope_id=$1),'baselines',(select coalesce(jsonb_agg(to_jsonb(r) order by id),'[]') from application_template_resource_baselines r where workspace_id=$1))")
        .bind(workspace).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn conflicting_shared_tool_forks_once_updates_same_copy_and_preserves_edited_or_deleted_copy()
{
    let (store, workspace, actor) = support::seed_store().await;
    let mut source = source_fixture(&store, actor.id).await;
    add_group(&mut source, "/second");
    source.tools[0].short_description = "Template B behavior".into();
    let scope = TemplateBaselineScope {
        workspace_id: workspace.id,
        template_id: "fork-isolation".into(),
    };
    let original = tool_row(&store, workspace.id, ORIGINAL).await;
    let other = existing_instance_snapshot(&store, workspace.id).await;
    let preview = mcp_merge::preview(
        &store,
        &scope,
        actor.id,
        &source,
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(preview.conflicts.is_empty(), "{:?}", preview.conflicts);
    let first = install(&store, &scope, actor.id, &source).await;
    let fork = created_fork(&first);
    let bindings = binding_rows(&store, workspace.id, INSTANCE).await;
    assert_eq!(bindings.len(), 2);
    assert!(bindings.iter().all(|binding| binding.2 == fork));
    let baselines = store.load_template_baselines(&scope).await.unwrap();
    let baseline = baselines
        .iter()
        .find(|b| b.key.kind == "mcp_tool" && b.key.source_id == ORIGINAL)
        .unwrap();
    assert_eq!(baseline.target_id, fork);
    assert!(baseline.applied_fingerprint.is_some());
    assert!(baseline.pending.is_none());
    let first_copy = tool_row(&store, workspace.id, &fork).await;
    let repeated = install(&store, &scope, actor.id, &source).await;
    assert!(repeated.created.is_empty());
    assert!(repeated.updated.is_empty());
    assert_eq!(tool_row(&store, workspace.id, &fork).await, first_copy);
    assert_eq!(binding_rows(&store, workspace.id, INSTANCE).await, bindings);

    source.manifest.bundle_version = "2.0.0".into();
    source.tools[0].short_description = "Template B upgraded".into();
    let upgraded = install(&store, &scope, actor.id, &source).await;
    assert!(!upgraded.created.iter().any(|item| item.kind == "mcp_tool"));
    assert!(upgraded.updated.iter().any(|item| item.kind == "mcp_tool"
        && item.source_id == ORIGINAL
        && item.target_id == fork));
    let updated_copy = tool_row(&store, workspace.id, &fork).await;
    assert_eq!(updated_copy["id"], first_copy["id"]);
    assert_eq!(updated_copy["short_description"], "Template B upgraded");

    let service = McpManagementService::new(store.clone());
    let current = service.get_tool(actor.id, &fork).await.unwrap();
    service
        .update_tool(UpdateMcpToolCommand {
            actor_user_id: actor.id,
            tool_id: fork.clone(),
            des_id: Some(current.des_id),
            name: current.name,
            short_description: "User edited copy".into(),
            full_description: current.full_description,
            interface_entry: mcp_template_catalog().remove(0),
            input_mapping: current.input_mapping,
            output_mapping: current.output_mapping,
            max_inline_chars: current.max_inline_chars,
            response_fields: current.response_fields,
            status: current.status,
        })
        .await
        .unwrap();
    source.tools[0].short_description = "Template B newer default".into();
    let edited_copy = tool_row(&store, workspace.id, &fork).await;
    let edited = install(&store, &scope, actor.id, &source).await;
    assert!(edited.created.iter().all(|item| item.kind != "mcp_tool"));
    assert!(edited
        .skipped
        .iter()
        .any(|item| item.kind == "mcp_tool" && item.reason == "user_modified"));
    assert_eq!(tool_row(&store, workspace.id, &fork).await, edited_copy);
    service.delete_tool(actor.id, &fork).await.unwrap();
    let deleted = install(&store, &scope, actor.id, &source).await;
    assert!(deleted.created.iter().all(|item| item.kind != "mcp_tool"));
    assert!(deleted
        .skipped
        .iter()
        .any(|item| item.kind == "mcp_tool" && item.reason == "user_deleted"));
    let tool_ids: Vec<String> =
        sqlx::query_scalar("select tool_id from mcp_tools where workspace_id=$1 order by tool_id")
            .bind(workspace.id)
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(tool_ids, vec![ORIGINAL.to_owned()]);
    assert_eq!(tool_row(&store, workspace.id, ORIGINAL).await, original);
    assert_eq!(
        existing_instance_snapshot(&store, workspace.id).await,
        other
    );
    assert_eq!(
        store
            .load_template_baselines(&scope)
            .await
            .unwrap()
            .iter()
            .find(|b| b.key.kind == "mcp_tool" && b.key.source_id == ORIGINAL)
            .unwrap()
            .target_id,
        fork
    );
}

#[tokio::test]
async fn equal_shared_tool_later_forks_and_redirects_only_unchanged_owned_binding_with_same_record_id(
) {
    let (store, workspace, actor) = support::seed_store().await;
    let mut source = source_fixture(&store, actor.id).await;
    for path in ["/modified", "/deleted", "/target-only"] {
        add_group(&mut source, path);
    }
    source.instances[0]
        .bindings
        .retain(|binding| binding.group_path != "/target-only");
    let scope = TemplateBaselineScope {
        workspace_id: workspace.id,
        template_id: "fork-rebind".into(),
    };
    let original = tool_row(&store, workspace.id, ORIGINAL).await;
    let other = existing_instance_snapshot(&store, workspace.id).await;
    let first = install(&store, &scope, actor.id, &source).await;
    assert!(first.created.iter().all(|item| item.kind != "mcp_tool"));
    let before = binding_rows(&store, workspace.id, INSTANCE).await;
    assert_eq!(before.len(), 3);
    assert!(before.iter().all(|binding| binding.2 == ORIGINAL));
    let tracked = store.load_template_baselines(&scope).await.unwrap();
    assert_eq!(
        tracked
            .iter()
            .filter(|b| b.key.kind == "mcp_binding")
            .count(),
        3
    );
    let owned_id = before.iter().find(|b| b.1 == "/group").unwrap().0;
    let modified_id = before.iter().find(|b| b.1 == "/modified").unwrap().0;
    let deleted_id = before.iter().find(|b| b.1 == "/deleted").unwrap().0;
    let service = McpManagementService::new(store.clone());
    service
        .update_tool_binding(UpdateMcpToolBindingCommand {
            actor_user_id: actor.id,
            binding_id: modified_id,
            group_path: "/modified".into(),
            display_alias: Some("User alias".into()),
            visible: true,
            sort_order: 0,
        })
        .await
        .unwrap();
    service
        .delete_tool_binding(actor.id, deleted_id)
        .await
        .unwrap();
    let target_only = service
        .create_tool_binding(CreateMcpToolBindingCommand {
            actor_user_id: actor.id,
            instance_id: INSTANCE.into(),
            group_path: "/target-only".into(),
            tool_id: ORIGINAL.into(),
            display_alias: Some("Local binding".into()),
            visible: true,
            sort_order: 0,
        })
        .await
        .unwrap();
    source.tools[0].short_description = "New template behavior".into();
    source.manifest.bundle_version = "2.0.0".into();
    let result = install(&store, &scope, actor.id, &source).await;
    let fork = created_fork(&result);
    let after = binding_rows(&store, workspace.id, INSTANCE).await;
    assert_eq!(
        after.len(),
        3,
        "No stale old binding or recreated deleted binding"
    );
    assert!(after.contains(&(owned_id, "/group".into(), fork.clone(), None)));
    assert!(after.contains(&(
        modified_id,
        "/modified".into(),
        ORIGINAL.into(),
        Some("User alias".into())
    )));
    assert!(after.contains(&(
        target_only.id,
        "/target-only".into(),
        ORIGINAL.into(),
        Some("Local binding".into())
    )));
    assert!(result
        .skipped
        .iter()
        .any(|item| item.kind == "mcp_binding" && item.reason == "user_modified"));
    assert!(result
        .skipped
        .iter()
        .any(|item| item.kind == "mcp_binding" && item.reason == "user_deleted"));
    let baseline = store.load_template_baselines(&scope).await.unwrap();
    let original_binding_key = serde_json::to_string(&[INSTANCE, "/group", ORIGINAL]).unwrap();
    let redirected = baseline
        .iter()
        .find(|b| b.key.kind == "mcp_binding" && b.key.source_id == original_binding_key)
        .unwrap();
    assert_eq!(
        redirected.target_id,
        serde_json::to_string(&[INSTANCE, "/group", fork.as_str()]).unwrap()
    );
    assert!(redirected.pending.is_none());
    let repeated = install(&store, &scope, actor.id, &source).await;
    assert!(repeated.created.is_empty());
    assert_eq!(binding_rows(&store, workspace.id, INSTANCE).await, after);
    assert_eq!(tool_row(&store, workspace.id, ORIGINAL).await, original);
    assert_eq!(
        existing_instance_snapshot(&store, workspace.id).await,
        other
    );
}

#[tokio::test]
async fn fork_rebind_and_baseline_receipt_failures_rollback_the_batch_then_retry_one_stable_copy() {
    let (store, workspace, actor) = support::seed_store().await;
    let mut source = source_fixture(&store, actor.id).await;
    let scope = TemplateBaselineScope {
        workspace_id: workspace.id,
        template_id: "fork-rollback".into(),
    };
    install(&store, &scope, actor.id, &source).await;
    source.tools[0].short_description = "Fork after shared reuse".into();
    let before = database_snapshot(&store, workspace.id).await;
    let preview = mcp_merge::preview(
        &store,
        &scope,
        actor.id,
        &source,
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(preview.conflicts.is_empty(), "{:?}", preview.conflicts);
    let planned_fork = preview
        .effects
        .iter()
        .find(|item| item.kind == "mcp_tool" && item.source_id == ORIGINAL)
        .and_then(|item| item.target_id.clone())
        .expect("preview identifies the planned tool copy");
    assert_ne!(planned_fork, ORIGINAL);
    assert_eq!(
        database_snapshot(&store, workspace.id).await,
        before,
        "Preview is read only"
    );
    // The owner must UPDATE the record when redirecting; a DELETE/INSERT would fail
    // the retained UUID assertion after retry, even if it escaped this trigger.
    sqlx::query("create function reject_fork_rebind() returns trigger language plpgsql as $$ begin if new.tool_record_id is distinct from old.tool_record_id then raise exception 'fixture rebind failure'; end if; return new; end $$").execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_fork_rebind before update on mcp_tool_bindings for each row execute function reject_fork_rebind()").execute(store.pool()).await.unwrap();
    let failed = mcp_merge::install(
        &store,
        &scope,
        actor.id,
        &source,
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(!failed.failures.is_empty());
    assert!(failed.created.is_empty() && failed.updated.is_empty());
    assert_eq!(database_snapshot(&store, workspace.id).await, before);
    sqlx::query("drop trigger reject_fork_rebind on mcp_tool_bindings")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("create function reject_fork_receipt() returns trigger language plpgsql as $$ begin if new.committed_operation_id is not null then raise exception 'fixture baseline receipt failure'; end if; return new; end $$").execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_fork_receipt before update on application_template_resource_baselines for each row execute function reject_fork_receipt()").execute(store.pool()).await.unwrap();
    let failed = mcp_merge::install(
        &store,
        &scope,
        actor.id,
        &source,
        &mcp_template_catalog(),
        "1.0.0",
    )
    .await
    .unwrap();
    assert!(!failed.failures.is_empty());
    assert!(failed.created.is_empty() && failed.updated.is_empty());
    assert_eq!(database_snapshot(&store, workspace.id).await, before);
    sqlx::query("drop trigger reject_fork_receipt on application_template_resource_baselines")
        .execute(store.pool())
        .await
        .unwrap();
    let prior_binding = binding_rows(&store, workspace.id, INSTANCE).await.remove(0);
    let retry = install(&store, &scope, actor.id, &source).await;
    let fork = created_fork(&retry);
    assert_eq!(
        fork, planned_fork,
        "Failed attempts do not consume a new copy identity"
    );
    let bindings = binding_rows(&store, workspace.id, INSTANCE).await;
    assert_eq!(
        bindings,
        vec![(
            prior_binding.0,
            prior_binding.1,
            fork.clone(),
            prior_binding.3
        )]
    );
    let snapshot = database_snapshot(&store, workspace.id).await;
    let repeated = install(&store, &scope, actor.id, &source).await;
    assert!(repeated.created.is_empty() && repeated.updated.is_empty());
    assert_eq!(database_snapshot(&store, workspace.id).await, snapshot);
    let tools: i64 = sqlx::query_scalar("select count(*) from mcp_tools where workspace_id=$1")
        .bind(workspace.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(tools, 2);
}
