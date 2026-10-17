//! AC-005/006: real uploaded package, root activation and SDK process through the managed Host.
use super::managed_snapshot_tests::RuntimeFixture;
use crate::{
    notification_runtime::ManagedNotificationProvider, provider_runtime::ApiProviderRuntime,
};
use control_plane::{
    notifications::{
        CreateNotificationInstance, NotificationSenderPort, NotificationService,
        PatchNotificationInstance,
    },
    plugin_management::*,
    ports::AuthRepository,
};
use domain::NotificationInstanceStatus;
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

fn package() -> Vec<u8> {
    let fixture = crate::api_workspace_root()
        .unwrap()
        .join("plugins/fixtures/acme.notification");
    let executable = PathBuf::from(
        std::env::var_os("NOTIFICATION_WORKER_FIXTURE")
            .expect("prebuild the SDK notification_worker example"),
    );
    assert!(
        executable.is_absolute() && executable.is_file(),
        "a real SDK process fixture is required"
    );
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    archive
        .append_path_with_name(fixture.join("manifest.yaml"), "manifest.yaml")
        .unwrap();
    archive
        .append_path_with_name(executable, "bin/worker")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}

#[tokio::test]
async fn notification_plugin_uses_unified_instances_and_managed_sdk_dispatch() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = state
        .store
        .load_actor_context_for_user(actor_id)
        .await
        .unwrap();
    let runtime = RuntimeFixture::new(&state);
    let management = PluginManagementService::new(
        runtime.store.clone(),
        ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "notification-fixture.1flowbasepkg".into(),
            package_bytes: package(),
        })
        .await
        .unwrap();
    let installation_id = installed.installation.id;
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    // Explicit root enable approves supported capabilities through the activation owner.
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    // Startup reconciles enabled installations before it knows the complete interface catalogue.
    let restarted = RuntimeFixture::new(&state);
    restarted
        .composition
        .rebuild_installation(installation_id)
        .await
        .unwrap();
    let before = restarted
        .composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    let interfaces = [
        "notifications.instances.list",
        "console.identity.session.delete",
    ];
    let module =
        plugin_framework::extension_bus::compile_managed_interface_module(interfaces).unwrap();
    assert!(module.extension_points.iter().all(|declared| before
        .graph
        .points()
        .iter()
        .all(|point| point.descriptor().point_id != declared.point_id)));
    restarted
        .composition
        .attach_interface_module(module.clone())
        .unwrap();
    restarted
        .composition
        .refresh_boot_snapshots()
        .await
        .unwrap();
    let refreshed = restarted
        .composition
        .snapshot(actor.current_workspace_id)
        .await
        .unwrap();
    assert_eq!(module.extension_points.len(), interfaces.len() * 6);
    assert!(module.extension_points.iter().all(|declared| refreshed
        .graph
        .points()
        .iter()
        .any(|point| point.descriptor() == declared)));
    assert_eq!(before.bindings.len(), refreshed.bindings.len());
    for (id, binding) in &before.bindings {
        assert_eq!(refreshed.bindings[id].handle, binding.handle);
    }
    restarted.host.stop().await.unwrap();
    let service = NotificationService::new(
        runtime.store.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_provider(Arc::new(ManagedNotificationProvider::new(
        runtime.composition.clone(),
    )));
    let types = service.types(&actor).await.unwrap();
    let descriptor = types
        .iter()
        .find(|entry| entry.descriptor.type_code == "acme/notification-fixture/receipt")
        .unwrap();
    assert!(descriptor.available);
    assert_eq!(
        descriptor.descriptor.config_form.fields[0]
            .control
            .as_deref(),
        Some("password")
    );
    assert_eq!(descriptor.descriptor.input_form.fields[0].key, "text");
    let instance = service
        .create_instance(
            &actor,
            CreateNotificationInstance {
                type_code: descriptor.descriptor.type_code.clone(),
                installation_id: Some(installation_id),
                display_name: "Ops receipt".into(),
                config_json: json!({"token":"private-notification-token","channel":"ops"}),
                status: None,
            },
        )
        .await
        .unwrap();
    let id = instance.record.id;
    assert!(instance.record.config_json.get("token").is_none());
    assert_eq!(instance.record.configured_secret_fields, ["token"]);
    let output = service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"text":"hello"}),
        )
        .await
        .unwrap();
    assert_eq!(
        output.result,
        json!({"accepted":true,"delivery_id":"ops:hello"})
    );
    assert!(!serde_json::to_string(&output)
        .unwrap()
        .contains("private-notification-token"));
    // A provider can accept externally while the final durable log update fails. The caller
    // receives an error, but the invocation must remain pending rather than become failure.
    sqlx::query("CREATE FUNCTION reject_notification_log_finalize() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture finalize unavailable'; END $$").execute(state.store.pool()).await.unwrap();
    sqlx::query("CREATE TRIGGER notification_log_finalize_fixture BEFORE UPDATE ON notification_send_logs FOR EACH ROW EXECUTE FUNCTION reject_notification_log_finalize()").execute(state.store.pool()).await.unwrap();
    assert!(service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"text":"accepted-log-write-fails"})
        )
        .await
        .is_err());
    sqlx::query("DROP TRIGGER notification_log_finalize_fixture ON notification_send_logs")
        .execute(state.store.pool())
        .await
        .unwrap();
    let pending = service
        .logs(
            &actor,
            control_plane::ports::NotificationLogListInput {
                status: Some("pending".into()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(pending.total, 1);
    assert!(pending.items[0].failure_code.is_none());
    assert!(pending.items[0].completed_at.is_none());
    let changed = service
        .patch_instance(
            &actor,
            id,
            PatchNotificationInstance {
                config_json: Some(json!({"channel":"alerts"})),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(changed.record.configured_secret_fields, ["token"]);
    assert_eq!(
        service
            .send(
                &actor,
                domain::NotificationSource::Api,
                id,
                json!({"text":"saved"})
            )
            .await
            .unwrap()
            .result["delivery_id"],
        "alerts:saved"
    );
    for input in [
        json!({"text":42}),
        json!({"text":"fixture-invalid-output"}),
        json!({"text":"fixture-secret-output"}),
        json!({"text":"fixture-provider-error"}),
    ] {
        let error = service
            .send(&actor, domain::NotificationSource::Api, id, input)
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("private-notification-token"));
    }
    let mut foreign = actor.clone();
    foreign.current_workspace_id = Uuid::now_v7();
    assert!(service
        .send(
            &foreign,
            domain::NotificationSource::Api,
            id,
            json!({"text":"foreign"})
        )
        .await
        .is_err());
    service
        .patch_instance(
            &actor,
            id,
            PatchNotificationInstance {
                status: Some(NotificationInstanceStatus::Disabled),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"text":"disabled"})
        )
        .await
        .is_err());
    service
        .patch_instance(
            &actor,
            id,
            PatchNotificationInstance {
                status: Some(NotificationInstanceStatus::Enabled),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    let retained = service.get_instance(&actor, id).await.unwrap();
    assert!(!retained.available);
    assert_eq!(retained.record.config_json["channel"], "alerts");
    assert!(service
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"text":"unavailable"})
        )
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar(
        "select count(*) from notification_instances where id=$1 and workspace_id=$2",
    )
    .bind(id)
    .bind(actor.current_workspace_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert_eq!(count, 1);
    runtime.host.stop().await.unwrap();
    let logs = service.logs(&actor, Default::default()).await.unwrap();
    assert_eq!(
        logs.items.iter().filter(|l| l.status == "success").count(),
        2
    );
    assert_eq!(
        logs.items.iter().filter(|l| l.status == "unknown").count(),
        3
    );
    assert_eq!(
        logs.items
            .iter()
            .filter(|l| l.failure_code.as_deref() == Some("provider_output_invalid"))
            .count(),
        2
    );
    assert_eq!(
        logs.items
            .iter()
            .filter(|l| l.failure_code.as_deref() == Some("provider_result_unknown"))
            .count(),
        1
    );
    let safe_json = serde_json::to_string(&logs).unwrap();
    assert!(!safe_json.contains("private-notification-token"));
    assert!(logs
        .items
        .iter()
        .all(|log| log.title.is_none() && log.content.is_none()));
}

mod authorization;
