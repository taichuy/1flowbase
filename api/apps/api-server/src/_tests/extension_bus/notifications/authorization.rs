//! Real canonical HTTP/service + encrypted PostgreSQL + installed SDK multiplex worker.
use super::*;
use async_trait::async_trait;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::notifications::NotificationProviderPort;
use control_plane::ports::NotificationRepository;
use extension_contracts::{
    ContributionId, NotificationAuthorizationOperation as Operation, NotificationTypeDescriptor,
};
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
use tower::ServiceExt;

fn notification_i18n() -> Value {
    json!({
        "zh_Hans": {
            "display_name":"回执测试", "description":"确认测试回执",
            "config_form":{"title":"渠道配置", "fields":{
                "token":{"label":"令牌", "description":"渠道令牌", "placeholder":"输入令牌"},
                "channel":{"label":"渠道"}
            }},
            "input_form":{"title":"消息", "fields":{"text":{"label":"正文"}}},
            "authorization":{"actions":{"qr":{"label":"扫码授权"}}}
        },
        "en_US": {
            "display_name":"Receipt fixture", "description":"Confirms a fixture receipt",
            "config_form":{"fields":{"token":{"label":"Token"},"channel":{"label":"Channel"}}},
            "input_form":{"fields":{"text":{"label":"Message"}}},
            "authorization":{"actions":{"qr":{"label":"Scan QR code"}}}
        }
    })
}

fn authorizable_package() -> Vec<u8> {
    let source = crate::api_workspace_root()
        .unwrap()
        .join("plugins/fixtures/acme.notification/manifest.yaml");
    let mut manifest: Value = serde_json::from_slice(&std::fs::read(source).unwrap()).unwrap();
    manifest["runtime"]["protocol"] = json!("stdio_json_multiplex_v1");
    manifest["managed"]["execution_bindings"][0]["runtime"]["protocol"] =
        json!("stdio_json_multiplex_v1");
    manifest["notification_types"][0]["authorization"] = json!({
        "actions":[{"code":"qr","label":"Scan QR code","kind":"qr_code"}],
        "managed_secret_keys":["qr_credential","qr_code_url","bot_token","bound_user_id","context_token","cursor"]
    });
    manifest["notification_types"][0]["i18n"] = notification_i18n();
    plugin_framework::parse_plugin_manifest(&manifest.to_string()).unwrap();
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let executable = PathBuf::from(
        std::env::var_os("NOTIFICATION_AUTHORIZATION_WORKER_FIXTURE")
            .expect("prebuild notification_authorization_worker shared SDK example"),
    );
    assert!(
        executable.is_absolute() && executable.is_file(),
        "real worker fixture required"
    );
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", bytes.as_slice())
        .unwrap();
    archive
        .append_path_with_name(executable, "bin/worker")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}

/// Delays only return delivery of a real worker poll result. It never simulates repository state.
struct GatedProvider {
    inner: ManagedNotificationProvider,
    pause: AtomicBool,
    entered: Notify,
    release: Notify,
}
#[async_trait]
impl NotificationProviderPort for GatedProvider {
    async fn available(
        &self,
        actor: &domain::ActorContext,
        installation: Uuid,
        contribution: &ContributionId,
    ) -> anyhow::Result<bool> {
        self.inner
            .available(actor, installation, contribution)
            .await
    }
    async fn send(
        &self,
        actor: &domain::ActorContext,
        installation: Uuid,
        descriptor: &NotificationTypeDescriptor,
        config: Value,
        input: Value,
    ) -> anyhow::Result<Value> {
        let poll = input["operation"]["type"] == "authorize"
            && input["operation"]["operation"]["type"] == "poll";
        let result = self
            .inner
            .send(actor, installation, descriptor, config, input)
            .await?;
        if poll && self.pause.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Ok(result)
    }
}

async fn api(
    app: &Router,
    cookie: &str,
    csrf: &str,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", cookie)
        .header("content-type", "application/json");
    if !csrf.is_empty() {
        request = request.header("x-csrf-token", csrf);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| json!({"message":String::from_utf8_lossy(&bytes)})),
    )
}

#[tokio::test]
async fn notification_authorization_http_encrypts_fences_late_polls_and_preserves_delivery_outcomes(
) {
    let (mut state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor_id: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE account='root'")
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
    .with_node_id(&state.api_node_id);
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "notification-auth-fixture.1flowbasepkg".into(),
            package_bytes: authorizable_package(),
        })
        .await
        .unwrap();
    let installation_id = installed.installation.id;
    assert_eq!(
        installed.installation.category,
        domain::ExtensionCategory::RuntimeExtensions
    );
    let inventory = management
        .list_local_inventory(
            actor_id,
            Some(ExtensionCatalogCategory::RuntimeExtensions),
            None,
            100,
        )
        .await
        .unwrap();
    assert!(inventory
        .entries
        .iter()
        .any(|entry| entry.installation.id == installation_id
            && entry.category == "runtime-extensions"));
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    // Manifest declaration without contribution authority remains insufficient.
    assert!(management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id
        })
        .await
        .is_err());
    PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .grant(
        &actor,
        installation_id,
        GrantContributionPermission {
            contribution_id: "acme.notification-fixture.send".into(),
            permission: "notification.send".into(),
            resource_scope: domain::ContributionResourceScope::Workspace,
            permission_contract_id: "notification".into(),
            permission_contract_version: "1".into(),
        },
    )
    .await
    .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id,
        })
        .await
        .unwrap();
    let provider = Arc::new(GatedProvider {
        inner: ManagedNotificationProvider::new(runtime.composition.clone()),
        pause: AtomicBool::new(false),
        entered: Notify::new(),
        release: Notify::new(),
    });
    let service = Arc::new(
        NotificationService::new(
            runtime.store.clone(),
            state.provider_secret_master_key.clone(),
        )
        .with_provider(provider.clone()),
    );
    Arc::get_mut(&mut state)
        .expect("unshared API state")
        .notification_service = service.clone();
    let app =
        crate::app_with_state_and_config(state.clone(), &crate::_tests::support::test_config());
    let (cookie, csrf) =
        crate::_tests::support::login_and_capture_cookie(&app, "root", "change-me").await;
    // Read through the authenticated Types API after upload, assignment, grant and enable.
    // Both dictionaries remain raw package metadata; no locale is selected by the Host.
    let (status, catalog) = api(
        &app,
        &cookie,
        &csrf,
        "GET",
        "/api/console/notifications/types",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let types = catalog["data"]
        .as_array()
        .expect("notification type catalog");
    let fixture = types
        .iter()
        .find(|entry| entry["type_code"] == "acme/notification-fixture/receipt")
        .expect("installed notification type");
    assert_eq!(fixture["installation_id"], installation_id.to_string());
    assert_eq!(fixture["available"], true);
    assert_eq!(fixture["i18n"], notification_i18n());
    assert_eq!(fixture["display_name"], "Receipt fixture");
    assert_eq!(fixture["config_form"]["fields"][0]["key"], "token");
    assert_eq!(fixture["config_form"]["fields"][0]["control"], "password");
    assert_eq!(fixture["input_form"]["fields"][0]["label"], "Text");
    assert_eq!(fixture["authorization"]["actions"][0]["code"], "qr");
    assert_eq!(
        fixture["authorization"]["actions"][0]["label"],
        "Scan QR code"
    );
    assert_eq!(
        fixture["input_schema"]["properties"]["text"]["type"],
        "string"
    );
    let system = types
        .iter()
        .find(|entry| entry["type_code"] == "system")
        .unwrap();
    assert!(
        system.get("i18n").is_none(),
        "legacy descriptor omits empty dictionaries"
    );
    let member_id = crate::_tests::support::create_member(
        &app,
        &cookie,
        &csrf,
        "auth-denied-member",
        "temp-pass",
    )
    .await;
    let member_role: String = sqlx::query_scalar(
        "SELECT code FROM roles WHERE workspace_id=$1 AND is_default_member_role=true",
    )
    .bind(actor.current_workspace_id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    control_plane::ports::RoleRepository::replace_role_console_policy(
        &state.store,
        &control_plane::ports::ReplaceRoleConsolePolicyInput {
            actor_user_id: actor_id,
            workspace_id: actor.current_workspace_id,
            role_code: member_role,
            groups: vec![domain::RoleConsoleGroupPolicy::custom(
                domain::ConsolePolicyGroup::settings_feature("system.notifications").unwrap(),
                vec![
                    domain::ConsoleOperationPolicy::simple(
                        domain::ConsoleOperationId::try_from("notifications.instances.get")
                            .unwrap(),
                        true,
                    ),
                    domain::ConsoleOperationPolicy::simple(
                        domain::ConsoleOperationId::try_from("notifications.instances.update")
                            .unwrap(),
                        true,
                    ),
                ],
            )],
        },
    )
    .await
    .unwrap();
    let (member_cookie, member_csrf) =
        crate::_tests::support::login_and_capture_cookie(&app, "auth-denied-member", "temp-pass")
            .await;
    let input = json!({"type_code":"acme/notification-fixture/receipt","installation_id":installation_id,"display_name":"QR managed","config_json":{"token":"private-config-token","channel":"ops"},"status":"disabled"});
    let (status, created) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        "/api/console/notifications/instances",
        input.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    assert_eq!(
        created["data"]["authorization"],
        json!({"status":"unauthorized"})
    );
    let id: Uuid = serde_json::from_value(created["data"]["id"].clone()).unwrap();
    let path = format!("/api/console/notifications/instances/{id}");
    let auth_path = format!("{path}/authorization");
    let send_path = format!("{path}/send");
    let (status, can_read) = api(
        &app,
        &member_cookie,
        &member_csrf,
        "GET",
        &path,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{can_read}");
    let (status, can_edit) = api(
        &app,
        &member_cookie,
        &member_csrf,
        "PATCH",
        &path,
        json!({"display_name":"Config editor without authorization permission"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{can_edit}");
    for (test_cookie, test_csrf, expected) in [
        (&member_cookie, &member_csrf, StatusCode::FORBIDDEN),
        (&cookie, &String::new(), StatusCode::UNAUTHORIZED),
        (&cookie, &"wrong-csrf".to_owned(), StatusCode::FORBIDDEN),
    ] {
        let (status, error) = api(
            &app,
            test_cookie,
            test_csrf,
            "POST",
            &auth_path,
            json!({"type":"begin","action":"qr"}),
        )
        .await;
        assert_eq!(status, expected, "{error}");
    }
    let (status, invalid) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        &auth_path,
        json!({"type":"poll","managed_secrets":{"bot_token":"injected"}}),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{invalid}");
    let (status, invalid) = api(&app, &cookie, &csrf, "POST", &auth_path,
        json!({"type":"maintain","delivery_window":{"status":"active","last_received_at_ms":1,"expires_at_ms":u64::MAX}})).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{invalid}");
    let mut foreign = actor.clone();
    foreign.current_workspace_id = Uuid::now_v7();
    assert!(service
        .authorize_instance(&foreign, id, Operation::Poll)
        .await
        .is_err());
    let (status, begin) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        &auth_path,
        json!({"type":"begin","action":"qr"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{begin}");
    assert_eq!(begin["data"]["status"], "pending");
    assert_eq!(begin["data"]["user_action"]["kind"], "qr_code");
    assert!(begin["data"].get("managed_secret_patch").is_none());
    assert!(!begin.to_string().contains("fixture-qr-credential"));
    let cipher: Value = sqlx::query_scalar(
        "SELECT encrypted_managed_secret_json FROM notification_instances WHERE id=$1",
    )
    .bind(id)
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    assert!(!cipher.to_string().contains("fixture-qr-credential"));
    let pending = service.get_instance(&actor, id).await.unwrap();
    assert_eq!(
        pending.record.status,
        NotificationInstanceStatus::Disabled,
        "authorization never enables administrator-disabled instance"
    );
    let (status, polled) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        &auth_path,
        json!({"type":"poll"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{polled}");
    assert_eq!(polled["data"], json!({"status":"authorized"}));
    assert_eq!(
        service
            .get_instance(&actor, id)
            .await
            .unwrap()
            .record
            .status,
        NotificationInstanceStatus::Disabled
    );
    // Account authorization does not imply a required inbound context. First send succeeds.
    let state_before_send = state
        .store
        .resolve_notification_authorization(
            actor.current_workspace_id,
            id,
            &state.provider_secret_master_key,
        )
        .await
        .unwrap();
    assert!(!state_before_send
        .managed_secrets
        .contains_key("context_token"));
    assert_eq!(
        state_before_send.managed_secrets["bound_user_id"],
        "fixture-scanned-peer"
    );
    api(
        &app,
        &cookie,
        &csrf,
        "PATCH",
        &path,
        json!({"status":"enabled"}),
    )
    .await;
    let (status, sent) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        &send_path,
        json!({"input":{"text":"hello"}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sent}");
    assert_eq!(
        sent["data"]["result"],
        json!({"accepted":true,"delivery_id":"ops:fixture"})
    );
    let persisted = state
        .store
        .resolve_notification_authorization(
            actor.current_workspace_id,
            id,
            &state.provider_secret_master_key,
        )
        .await
        .unwrap();
    assert_eq!(
        persisted.managed_secrets["context_token"],
        "fixture-context"
    );
    assert_eq!(persisted.managed_secrets["cursor"], "next");
    for (text, status, code) in [
        (
            "fixture-recipient-not-ready",
            "failure",
            "recipient_not_ready",
        ),
        (
            "fixture-subscription-required",
            "failure",
            "subscription_required",
        ),
        (
            "fixture-subscription-expired",
            "failure",
            "subscription_expired",
        ),
        (
            "fixture-subscription-unavailable",
            "failure",
            "subscription_unavailable",
        ),
        ("fixture-rejected", "failure", "provider_rejected"),
        ("fixture-unknown", "unknown", "provider_result_unknown"),
        (
            "fixture-undeclared-patch",
            "unknown",
            "provider_result_unknown",
        ),
        (
            "fixture-secret-output",
            "unknown",
            "provider_result_unknown",
        ),
        ("fixture-invalid", "failure", "authorization_invalid"),
        (
            "after-invalid-local-rejection",
            "failure",
            "authorization_invalid",
        ),
    ] {
        let (http, error) = api(
            &app,
            &cookie,
            &csrf,
            "POST",
            &send_path,
            json!({"input":{"text":text}}),
        )
        .await;
        assert!(!http.is_success(), "{error}");
        let logs = service
            .logs(
                &actor,
                control_plane::ports::NotificationLogListInput {
                    instance_id: Some(id),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(logs.items[0].status, status);
        assert_eq!(logs.items[0].failure_code.as_deref(), Some(code));
        assert!(logs.items[0].completed_at.is_some());
        if code.starts_with("subscription_") {
            assert_eq!(error["code"], code);
            assert!(error["message"].as_str().unwrap().contains("retry"));
            let persisted = state
                .store
                .resolve_notification_authorization(
                    actor.current_workspace_id,
                    id,
                    &state.provider_secret_master_key,
                )
                .await
                .unwrap();
            assert_eq!(persisted.managed_secrets["bot_token"], "fixture-bot-token");
            assert!(!persisted.managed_secrets.contains_key("delivery_window"));
            let (http, detail) = api(&app, &cookie, &csrf, "GET", &path, Value::Null).await;
            assert_eq!(http, StatusCode::OK);
            assert_eq!(detail["data"]["authorization"]["status"], "authorized");
            let expected = match code {
                "subscription_required" => "waiting_for_message",
                "subscription_expired" => "expired",
                _ => "unknown",
            };
            assert_eq!(
                detail["data"]["authorization"]["delivery_window"]["status"],
                expected
            );
            let listed = service.list_instances(&actor).await.unwrap();
            let listed = listed
                .iter()
                .find(|instance| instance.record.id == id)
                .unwrap();
            assert_eq!(
                serde_json::to_value(&listed.authorization).unwrap()["delivery_window"]["status"],
                expected
            );
            let polled = service
                .authorize_instance(&actor, id, Operation::Poll)
                .await
                .unwrap();
            assert_eq!(
                serde_json::to_value(polled).unwrap()["delivery_window"]["status"],
                expected
            );
            if code == "subscription_expired" {
                // Expiry projection does not mutate the persisted provider evidence.
                assert_eq!(
                    persisted
                        .authorization
                        .unwrap()
                        .delivery_window
                        .unwrap()
                        .status,
                    extension_contracts::NotificationDeliveryWindowStatus::Active
                );
            }
            let (_, maintained) = api(
                &app,
                &cookie,
                &csrf,
                "POST",
                &auth_path,
                json!({"type":"maintain"}),
            )
            .await;
            assert_eq!(maintained["data"]["delivery_window"]["status"], expected);
            assert!(!detail.to_string().contains("fixture-bot-token"));
            if code == "subscription_required" {
                let cancelled = service
                    .authorize_instance(&actor, id, Operation::Cancel)
                    .await
                    .unwrap();
                assert!(cancelled.delivery_window.is_none());
                let persisted = state
                    .store
                    .resolve_notification_authorization(
                        actor.current_workspace_id,
                        id,
                        &state.provider_secret_master_key,
                    )
                    .await
                    .unwrap();
                assert!(persisted.managed_secrets.is_empty());
                assert!(persisted.authorization.unwrap().delivery_window.is_none());
                service
                    .authorize_instance(
                        &actor,
                        id,
                        Operation::Begin {
                            action: "qr".into(),
                        },
                    )
                    .await
                    .unwrap();
                service
                    .authorize_instance(&actor, id, Operation::Poll)
                    .await
                    .unwrap();
            }
            if code == "subscription_unavailable" {
                let pending = service
                    .authorize_instance(
                        &actor,
                        id,
                        Operation::Begin {
                            action: "qr".into(),
                        },
                    )
                    .await
                    .unwrap();
                assert!(pending.delivery_window.is_none());
                let restored = service
                    .authorize_instance(&actor, id, Operation::Poll)
                    .await
                    .unwrap();
                assert!(restored.delivery_window.is_none());
            }
        }
        if text == "fixture-recipient-not-ready" {
            let failed_state = state
                .store
                .resolve_notification_authorization(
                    actor.current_workspace_id,
                    id,
                    &state.provider_secret_master_key,
                )
                .await
                .unwrap();
            assert_eq!(failed_state.managed_secrets["cursor"], "failure-next");
            assert_eq!(
                service
                    .get_instance(&actor, id)
                    .await
                    .unwrap()
                    .authorization
                    .unwrap()
                    .status,
                extension_contracts::NotificationAuthorizationStatus::Authorized
            );
        }
    }
    // An old provider result cannot revive a cancelled or re-begun challenge.
    for rebegin in [false, true] {
        service
            .authorize_instance(
                &actor,
                id,
                Operation::Begin {
                    action: "qr".into(),
                },
            )
            .await
            .unwrap();
        provider.pause.store(true, Ordering::SeqCst);
        let poll_service = service.clone();
        let poll_actor = actor.clone();
        let poll = tokio::spawn(async move {
            poll_service
                .authorize_instance(&poll_actor, id, Operation::Poll)
                .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(10),
            provider.entered.notified(),
        )
        .await
        .expect("poll fixture reached return gate");
        let expected = if rebegin { "pending" } else { "unauthorized" };
        service
            .authorize_instance(
                &actor,
                id,
                if rebegin {
                    Operation::Begin {
                        action: "qr".into(),
                    }
                } else {
                    Operation::Cancel
                },
            )
            .await
            .unwrap();
        provider.release.notify_one();
        let error = poll.await.unwrap().unwrap_err();
        assert_eq!(
            error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
            Some(&control_plane::errors::ControlPlaneError::Conflict(
                "notification_authorization_stale"
            ))
        );
        let public = serde_json::to_value(service.get_instance(&actor, id).await.unwrap()).unwrap();
        assert_eq!(public["authorization"]["status"], expected);
        assert!(public["authorization"].get("delivery_window").is_none());
    }
    service
        .authorize_instance(&actor, id, Operation::Cancel)
        .await
        .unwrap();
    let cleared = state
        .store
        .resolve_notification_authorization(
            actor.current_workspace_id,
            id,
            &state.provider_secret_master_key,
        )
        .await
        .unwrap();
    assert!(cleared.managed_secrets.is_empty());
    let (status, error) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        &send_path,
        json!({"input":{"text":"unauthorized"}}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{error}");
    assert_eq!(
        service
            .logs(&actor, Default::default())
            .await
            .unwrap()
            .items[0]
            .failure_code
            .as_deref(),
        Some("authorization_required")
    );
    // A supplier returning an internal challenge in the QR URL cannot leak it to the client.
    let mut unsafe_input = input;
    unsafe_input["config_json"]["channel"] = json!("unsafe-qr");
    let (status, unsafe_instance) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        "/api/console/notifications/instances",
        unsafe_input,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{unsafe_instance}");
    let unsafe_id = unsafe_instance["data"]["id"].as_str().unwrap();
    let (status, error) = api(
        &app,
        &cookie,
        &csrf,
        "POST",
        &format!("/api/console/notifications/instances/{unsafe_id}/authorization"),
        json!({"type":"begin","action":"qr"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY, "{error}");
    assert!(!error.to_string().contains("fixture-qr-credential"));
    let (status, error) = api(
        &app,
        &cookie,
        &csrf,
        "PATCH",
        &path,
        json!({"config_json":{"bot_token":"client-injected"}}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
    // Recreate Host + service with the same database and master key after a pending begin.
    service
        .authorize_instance(
            &actor,
            id,
            Operation::Begin {
                action: "qr".into(),
            },
        )
        .await
        .unwrap();
    runtime.host.stop().await.unwrap();
    let reconstructed = RuntimeFixture::new(&state);
    // RuntimeFixture creates an empty composition; production boot restores persisted
    // installations before serving requests. Reuse the same activation owner here.
    reconstructed
        .composition
        .rebuild_installation(installation_id)
        .await
        .unwrap();
    let restored = NotificationService::new(
        reconstructed.store.clone(),
        state.provider_secret_master_key.clone(),
    )
    .with_provider(Arc::new(ManagedNotificationProvider::new(
        reconstructed.composition.clone(),
    )));
    assert_eq!(
        restored
            .authorize_instance(&actor, id, Operation::Poll)
            .await
            .unwrap()
            .status,
        extension_contracts::NotificationAuthorizationStatus::Authorized
    );
    assert_eq!(
        restored
            .authorize_instance(&actor, id, Operation::Maintain)
            .await
            .unwrap()
            .status,
        extension_contracts::NotificationAuthorizationStatus::Authorized
    );
    assert!(restored
        .send(
            &actor,
            domain::NotificationSource::Api,
            id,
            json!({"text":"after-restart"})
        )
        .await
        .is_ok());
    // Legacy system notification contract is unchanged through the same sender service.
    let system = restored
        .list_instances(&actor)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.record.type_code == "system")
        .unwrap();
    assert!(system.authorization.is_none());
    let legacy = restored
        .send(
            &actor,
            domain::NotificationSource::Api,
            system.record.id,
            json!({"user_ids":[member_id],"title":"Legacy","content":"Committed"}),
        )
        .await
        .unwrap();
    assert_eq!(legacy.result["recipient_count"], 1);
    let logs = restored.logs(&actor, Default::default()).await.unwrap();
    let public = serde_json::to_string(&logs).unwrap();
    for secret in [
        "fixture-qr-credential",
        "fixture-bot-token",
        "fixture-context",
        "fixture-scanned-peer",
        "private-config-token",
        "private-provider-secret",
    ] {
        assert!(!public.contains(secret));
    }
    let instances = serde_json::to_string(&restored.list_instances(&actor).await.unwrap()).unwrap();
    for secret in [
        "fixture-qr-credential",
        "fixture-bot-token",
        "fixture-context",
        "fixture-scanned-peer",
        "private-config-token",
    ] {
        assert!(!instances.contains(secret));
    }
    reconstructed.host.stop().await.unwrap();
}
