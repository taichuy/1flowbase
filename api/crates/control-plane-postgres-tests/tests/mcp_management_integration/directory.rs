use super::*;
use control_plane::mcp_management::{CopyMcpInstanceCommand, ReplaceMcpInstanceDirectoryCommand};
use control_plane::ports::{ReplaceMcpInstanceDirectoryInput, UpsertMcpGroupInput};
use domain::mcp_management::{McpDirectoryBinding, McpDirectoryGroup, McpInstanceDirectory};
use uuid::Uuid;

async fn fixture() -> (
    PgControlPlaneStore,
    domain::ActorContext,
    McpInstanceDirectory,
) {
    let (store, workspace, user) = seed_store().await;
    seed_builtin_bundle_installation(&store, user.id, "1.0.2").await;
    store
        .seed_mcp_bundle_graph_once_atomically(&managed_frontstage_seed_input(
            user.id,
            workspace.id,
            managed_frontstage_package("1.0.2"),
        ))
        .await
        .unwrap();
    let actor = McpManagementRepository::load_actor_context_for_user(&store, user.id)
        .await
        .unwrap();
    let snapshot = store
        .get_mcp_instance_directory(workspace.id, "frontstage_browser")
        .await
        .unwrap();
    (store, actor, snapshot)
}

fn command(snapshot: &McpInstanceDirectory) -> ReplaceMcpInstanceDirectoryCommand {
    ReplaceMcpInstanceDirectoryCommand {
        instance_id: "frontstage_browser".into(),
        expected_snapshot: snapshot.clone(),
        groups: snapshot
            .groups
            .iter()
            .map(|group| McpDirectoryGroup {
                id: Some(group.id),
                path: group.path.clone(),
                display_name: group.display_name.clone(),
                description_short: group.description_short.clone(),
                enabled: group.enabled,
                sort_order: group.sort_order,
            })
            .collect(),
        bindings: snapshot
            .bindings
            .iter()
            .map(|binding| McpDirectoryBinding {
                id: Some(binding.id),
                tool_id: binding.tool_id.clone(),
                group_path: binding.group_path.clone(),
                display_alias: binding.display_alias.clone(),
                visible: binding.visible,
                sort_order: binding.sort_order,
            })
            .collect(),
    }
}

fn assert_contract(error: &anyhow::Error, expected: control_plane::errors::ControlPlaneError) {
    assert_eq!(
        error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(&expected),
        "{error:#}"
    );
}

async fn wait_for_lock(store: &PgControlPlaneStore, table: &str, mode: &str) {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_locks where relation=to_regclass($1) and mode=$2 and not granted)",
            ).bind(table).bind(mode).fetch_one(store.pool()).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("directory operation must reach the controlled table lock");
}

#[tokio::test]
async fn directory_get_keeps_one_snapshot_across_concurrent_group_and_binding_writes() {
    let (store, actor, initial) = fixture().await;
    let mut writer = store.pool().begin().await.unwrap();
    sqlx::query("lock table mcp_tool_bindings in access exclusive mode")
        .execute(&mut *writer)
        .await
        .unwrap();
    let reader_store = store.clone();
    let workspace_id = actor.current_workspace_id;
    let reader = tokio::spawn(async move {
        reader_store
            .get_mcp_instance_directory(workspace_id, "frontstage_browser")
            .await
    });
    // The instance and group reads have completed before the binding read is unblocked.
    wait_for_lock(&store, "mcp_tool_bindings", "AccessShareLock").await;
    sqlx::query(
        "update mcp_groups set display_name='Concurrent group' where instance_record_id=$1",
    )
    .bind(initial.instance_record_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query("update mcp_tool_bindings set display_alias='Concurrent binding' where instance_record_id=$1")
        .bind(initial.instance_record_id).execute(&mut *writer).await.unwrap();
    writer.commit().await.unwrap();
    assert_eq!(reader.await.unwrap().unwrap(), initial);
    let current = store
        .get_mcp_instance_directory(workspace_id, "frontstage_browser")
        .await
        .unwrap();
    assert_ne!(current, initial);
    assert!(current
        .groups
        .iter()
        .all(|group| group.display_name == "Concurrent group"));
    assert!(current
        .bindings
        .iter()
        .all(|binding| binding.display_alias.as_deref() == Some("Concurrent binding")));
}

#[tokio::test]
async fn directory_replace_waits_for_legacy_writer_then_rejects_its_stale_snapshot() {
    let (store, actor, initial) = fixture().await;
    let mut legacy_writer = store.pool().begin().await.unwrap();
    sqlx::query("update mcp_groups set display_name='Legacy writer' where instance_record_id=$1")
        .bind(initial.instance_record_id)
        .execute(&mut *legacy_writer)
        .await
        .unwrap();
    let replacing_store = store.clone();
    let replacing_actor = actor.clone();
    let replacement = command(&initial);
    let replacing = tokio::spawn(async move {
        McpManagementService::new(replacing_store)
            .replace_instance_directory_for_actor(&replacing_actor, replacement)
            .await
    });
    wait_for_lock(&store, "mcp_groups", "ShareRowExclusiveLock").await;
    sqlx::query("update mcp_tool_bindings set visible=false where instance_record_id=$1")
        .bind(initial.instance_record_id)
        .execute(&mut *legacy_writer)
        .await
        .unwrap();
    legacy_writer.commit().await.unwrap();
    assert_contract(
        &replacing.await.unwrap().unwrap_err(),
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_snapshot"),
    );
    let saved = store
        .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
        .await
        .unwrap();
    assert_eq!(saved.groups[0].display_name, "Legacy writer");
    assert!(saved.bindings.iter().all(|binding| !binding.visible));
}

#[tokio::test]
async fn directory_replace_creates_moves_reorders_deletes_and_preserves_non_target_data() {
    let (store, actor, initial) = fixture().await;
    let service = McpManagementService::new(store.clone());
    service
        .copy_instance_for_actor(
            &actor,
            CopyMcpInstanceCommand {
                actor_user_id: actor.user_id,
                source_instance_id: "frontstage_browser".into(),
                instance_id: "other".into(),
                name: "Other".into(),
            },
        )
        .await
        .unwrap();
    let other = store
        .get_mcp_instance_directory(actor.current_workspace_id, "other")
        .await
        .unwrap();
    let tools = store
        .list_mcp_tools(actor.current_workspace_id)
        .await
        .unwrap();
    let policy = store
        .get_mcp_instance_discovery_policy(initial.instance_record_id)
        .await
        .unwrap();
    let instance = store
        .get_mcp_instance(actor.current_workspace_id, "frontstage_browser")
        .await
        .unwrap();
    store
        .upsert_mcp_client_credential(&UpsertMcpClientCredentialInput {
            id: Uuid::now_v7(),
            user_id: actor.user_id,
            workspace_id: actor.current_workspace_id,
            instance_record_id: initial.instance_record_id,
            api_key: "preserved-key".into(),
            master_key: "test-key".into(),
        })
        .await
        .unwrap();
    let original_groups = store
        .list_mcp_groups(&[initial.instance_record_id])
        .await
        .unwrap();
    let original_bindings = store
        .list_mcp_tool_bindings(&[initial.instance_record_id])
        .await
        .unwrap();
    let mut replacement = command(&initial);
    replacement.groups[0].path = "/ops".into();
    replacement.groups[0].display_name = "Operations".into();
    replacement.groups[0].enabled = false;
    replacement.groups[0].sort_order = 8;
    replacement.groups.push(McpDirectoryGroup {
        id: None,
        path: "/ops/sub".into(),
        display_name: "Sub".into(),
        description_short: Some("New".into()),
        enabled: true,
        sort_order: -2,
    });
    replacement.bindings.truncate(1);
    replacement.bindings[0].group_path = "/ops/sub".into();
    replacement.bindings[0].display_alias = Some("Moved".into());
    replacement.bindings[0].visible = false;
    replacement.bindings[0].sort_order = -7;
    replacement.bindings.push(McpDirectoryBinding {
        id: None,
        tool_id: initial.bindings[1].tool_id.clone(),
        group_path: "/".into(),
        display_alias: None,
        visible: true,
        sort_order: 11,
    });
    let saved = service
        .replace_instance_directory_for_actor(&actor, replacement)
        .await
        .unwrap();
    assert_eq!(saved.groups.len(), 2);
    assert_eq!(saved.bindings.len(), 2);
    assert!(saved
        .groups
        .iter()
        .any(|group| group.id == initial.groups[0].id
            && group.path == "/ops"
            && !group.enabled
            && group.sort_order == 8));
    assert!(saved
        .bindings
        .iter()
        .any(|binding| binding.id == initial.bindings[0].id
            && binding.group_path == "/ops/sub"
            && !binding.visible
            && binding.sort_order == -7));
    assert!(!saved
        .bindings
        .iter()
        .any(|binding| binding.id == initial.bindings[1].id));
    let groups = store
        .list_mcp_groups(&[initial.instance_record_id])
        .await
        .unwrap();
    let bindings = store
        .list_mcp_tool_bindings(&[initial.instance_record_id])
        .await
        .unwrap();
    let retained = groups
        .iter()
        .find(|group| group.id == initial.groups[0].id)
        .unwrap();
    assert_eq!(
        (retained.created_at, retained.created_by),
        (original_groups[0].created_at, original_groups[0].created_by)
    );
    let retained = bindings
        .iter()
        .find(|binding| binding.id == initial.bindings[0].id)
        .unwrap();
    let original = original_bindings
        .iter()
        .find(|binding| binding.id == retained.id)
        .unwrap();
    assert_eq!(
        (retained.created_at, retained.created_by),
        (original.created_at, original.created_by)
    );
    assert_eq!(
        store
            .get_mcp_instance_directory(actor.current_workspace_id, "other")
            .await
            .unwrap(),
        other
    );
    assert_eq!(
        store
            .list_mcp_tools(actor.current_workspace_id)
            .await
            .unwrap(),
        tools
    );
    assert_eq!(
        store
            .get_mcp_instance_discovery_policy(initial.instance_record_id)
            .await
            .unwrap(),
        policy
    );
    assert_eq!(
        store
            .get_mcp_instance(actor.current_workspace_id, "frontstage_browser")
            .await
            .unwrap(),
        instance
    );
    assert_eq!(
        store
            .get_mcp_client_credential(
                actor.user_id,
                actor.current_workspace_id,
                initial.instance_record_id,
                "test-key"
            )
            .await
            .unwrap()
            .as_deref(),
        Some("preserved-key")
    );
    let mut empty = command(&saved);
    empty.groups.clear();
    empty.bindings.clear();
    let deleted = service
        .replace_instance_directory_for_actor(&actor, empty)
        .await
        .unwrap();
    assert!(deleted.groups.is_empty() && deleted.bindings.is_empty());
}

#[tokio::test]
async fn directory_snapshot_ignores_array_order_and_detects_legacy_writer_changes() {
    let (store, actor, initial) = fixture().await;
    let service = McpManagementService::new(store.clone());
    let mut unchanged = command(&initial);
    unchanged.expected_snapshot.groups.reverse();
    unchanged.expected_snapshot.bindings.reverse();
    let saved = service
        .replace_instance_directory_for_actor(&actor, unchanged)
        .await
        .unwrap();
    assert_eq!(saved, initial);
    store
        .upsert_mcp_group(&UpsertMcpGroupInput {
            id: Uuid::now_v7(),
            actor_user_id: actor.user_id,
            instance_record_id: initial.instance_record_id,
            path: "/legacy".into(),
            display_name: "Legacy".into(),
            description_short: None,
            enabled: true,
            sort_order: 2,
        })
        .await
        .unwrap();
    let before = store
        .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
        .await
        .unwrap();
    let error = service
        .replace_instance_directory_for_actor(&actor, command(&initial))
        .await
        .unwrap_err();
    assert_contract(
        &error,
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_snapshot"),
    );
    assert_eq!(
        store
            .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
            .await
            .unwrap(),
        before
    );
    let mut stale = command(&before);
    stale.expected_snapshot.bindings[0].visible = !stale.expected_snapshot.bindings[0].visible;
    assert!(service
        .replace_instance_directory_for_actor(&actor, stale)
        .await
        .is_err());
}

#[tokio::test]
async fn directory_concurrent_replacements_commit_exactly_once() {
    let (store, actor, initial) = fixture().await;
    let left = McpManagementService::new(store.clone());
    let right = McpManagementService::new(store.clone());
    let mut a = command(&initial);
    a.groups[0].display_name = "First".into();
    let mut b = command(&initial);
    b.groups[0].display_name = "Second".into();
    let (a, b) = tokio::join!(
        left.replace_instance_directory_for_actor(&actor, a),
        right.replace_instance_directory_for_actor(&actor, b)
    );
    assert_ne!(
        a.is_ok(),
        b.is_ok(),
        "exactly one replacement must commit: {a:?} {b:?}"
    );
    let (saved, error) = match (a, b) {
        (Ok(saved), Err(error)) | (Err(error), Ok(saved)) => (saved, error),
        _ => unreachable!(),
    };
    assert_contract(
        &error,
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_snapshot"),
    );
    assert_eq!(
        store
            .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
            .await
            .unwrap(),
        saved
    );
}

#[tokio::test]
async fn directory_database_failure_after_deletes_rolls_back_every_write() {
    let (store, actor, initial) = fixture().await;
    let groups = store
        .list_mcp_groups(&[initial.instance_record_id])
        .await
        .unwrap();
    let bindings = store
        .list_mcp_tool_bindings(&[initial.instance_record_id])
        .await
        .unwrap();
    // Controlled database failures happen after deletion and group insertion.
    sqlx::query("create function reject_directory_binding() returns trigger language plpgsql as $$ begin if new.display_alias='reject_unique' then raise exception 'controlled unique failure' using errcode='23505'; elsif new.display_alias='reject_serialization' then raise exception 'controlled serialization failure' using errcode='40001'; elsif new.display_alias='reject_deadlock' then raise exception 'controlled deadlock failure' using errcode='40P01'; end if; return new; end $$")
        .execute(store.pool()).await.unwrap();
    sqlx::query("create trigger reject_directory_binding before insert on mcp_tool_bindings for each row execute function reject_directory_binding()")
        .execute(store.pool()).await.unwrap();
    for rejected_alias in ["reject_unique", "reject_serialization", "reject_deadlock"] {
        let mut replacement = command(&initial);
        replacement.groups[0].display_name = "Must roll back".into();
        replacement.bindings[0].display_alias = Some(rejected_alias.into());
        let error = McpManagementService::new(store.clone())
            .replace_instance_directory_for_actor(&actor, replacement)
            .await
            .unwrap_err();
        assert_contract(
            &error,
            control_plane::errors::ControlPlaneError::Conflict("mcp_directory_concurrent_write"),
        );
        assert_eq!(
            store
                .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
                .await
                .unwrap(),
            initial
        );
        assert_eq!(
            store
                .list_mcp_groups(&[initial.instance_record_id])
                .await
                .unwrap(),
            groups
        );
        assert_eq!(
            store
                .list_mcp_tool_bindings(&[initial.instance_record_id])
                .await
                .unwrap(),
            bindings
        );
    }
}

#[tokio::test]
async fn directory_rejects_cross_instance_ids_recreation_and_foreign_workspace_tools() {
    let (store, actor, initial) = fixture().await;
    let service = McpManagementService::new(store.clone());
    service
        .copy_instance_for_actor(
            &actor,
            CopyMcpInstanceCommand {
                actor_user_id: actor.user_id,
                source_instance_id: "frontstage_browser".into(),
                instance_id: "other".into(),
                name: "Other".into(),
            },
        )
        .await
        .unwrap();
    let other = store
        .get_mcp_instance_directory(actor.current_workspace_id, "other")
        .await
        .unwrap();
    let mut bad = command(&initial);
    bad.groups[0].id = Some(other.groups[0].id);
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::InvalidInput("group_id"),
    );
    let mut bad = command(&initial);
    bad.bindings[0].id = Some(other.bindings[0].id);
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::InvalidInput("binding_id"),
    );
    let mut bad = command(&initial);
    bad.groups[0].id = None;
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_group_identity"),
    );
    let mut bad = command(&initial);
    bad.bindings[0].id = None;
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_binding_identity"),
    );
    let mut bad = command(&initial);
    bad.expected_snapshot.instance_record_id = other.instance_record_id;
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_snapshot"),
    );
    let tenant = store.upsert_root_tenant().await.unwrap();
    let foreign_workspace = store
        .upsert_workspace(tenant.id, "Foreign MCP Workspace")
        .await
        .unwrap();
    let mut tool = managed_frontstage_seed_input(
        actor.user_id,
        foreign_workspace.id,
        managed_frontstage_package("1.0.2"),
    )
    .tools
    .remove(0);
    tool.tool_id = "foreign_only".into();
    store.create_mcp_tool(&tool).await.unwrap();
    let mut bad = command(&initial);
    bad.bindings[0].tool_id = "foreign_only".into();
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::NotFound("mcp_tool"),
    );
    assert_contract(
        &store
            .get_mcp_instance_directory(foreign_workspace.id, "frontstage_browser")
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::NotFound("mcp_instance"),
    );
    let bad = command(&initial);
    assert_contract(
        &store
            .replace_mcp_instance_directory_atomically(&ReplaceMcpInstanceDirectoryInput {
                actor_user_id: actor.user_id,
                workspace_id: foreign_workspace.id,
                instance_id: bad.instance_id,
                expected_snapshot: bad.expected_snapshot,
                groups: bad.groups,
                bindings: bad.bindings,
            })
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::NotFound("mcp_instance"),
    );
    assert_eq!(
        store
            .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
            .await
            .unwrap(),
        initial
    );
    assert_eq!(
        store
            .get_mcp_instance_directory(actor.current_workspace_id, "other")
            .await
            .unwrap(),
        other
    );
}

#[tokio::test]
async fn directory_rejects_invalid_paths_parents_duplicate_identities_and_tool_retargeting() {
    let (store, actor, initial) = fixture().await;
    let service = McpManagementService::new(store.clone());
    let mut bad = command(&initial);
    bad.groups[0].path = "/invalid//path".into();
    assert!(service
        .replace_instance_directory_for_actor(&actor, bad)
        .await
        .is_err());
    let mut bad = command(&initial);
    bad.groups[0].path = "/missing/child".into();
    assert!(service
        .replace_instance_directory_for_actor(&actor, bad)
        .await
        .is_err());
    let mut bad = command(&initial);
    bad.groups.push(bad.groups[0].clone());
    assert!(service
        .replace_instance_directory_for_actor(&actor, bad)
        .await
        .is_err());
    let mut bad = command(&initial);
    bad.bindings.push(bad.bindings[0].clone());
    assert!(service
        .replace_instance_directory_for_actor(&actor, bad)
        .await
        .is_err());
    let mut bad = command(&initial);
    bad.bindings[0].group_path = "/missing".into();
    assert!(service
        .replace_instance_directory_for_actor(&actor, bad)
        .await
        .is_err());
    let mut bad = command(&initial);
    bad.bindings[0].tool_id = bad.bindings[1].tool_id.clone();
    bad.bindings[0].group_path = "/".into();
    assert_contract(
        &service
            .replace_instance_directory_for_actor(&actor, bad)
            .await
            .unwrap_err(),
        control_plane::errors::ControlPlaneError::Conflict("mcp_directory_binding_identity"),
    );
    assert_eq!(
        store
            .get_mcp_instance_directory(actor.current_workspace_id, "frontstage_browser")
            .await
            .unwrap(),
        initial
    );
}
