//! Root #2007 AC-003/004, AUTH-03/04/06: real package + grants + SDK processes + HTTP/MCP.
use super::create_pair;
use crate::{
    _tests::{
        mcp_protocol_routes::{
            call_mcp, create_api_key, create_mcp_instance, create_model_probe_tool, response_json,
        },
        support::{
            create_member, create_role, login_and_capture_cookie, replace_member_roles,
            replace_role_permissions, test_api_state_with_database_url,
        },
    },
    app_state::ApiState,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices},
};
use axum::{http::StatusCode, Router};
use control_plane::{
    plugin_management::*,
    ports::{AuthRepository, PluginRepository},
};
use extension_contracts::ManagedHookHostFrame;
use runtime_extension_host::RuntimeArtifactResolver;
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use uuid::Uuid;

const PHASES: [&str; 6] = [
    "authorization",
    "admission",
    "before",
    "after",
    "failure",
    "completion",
];
pub(super) struct Fixture {
    pub(super) state: Arc<ApiState>,
    pub(super) app: Router,
    pub(super) cookie: String,
    pub(super) csrf: String,
    pub(super) token: String,
    pub(super) actor: domain::ActorContext,
    pub(super) installation_id: Uuid,
    pub(super) worker: PathBuf,
}

/// The production managed binding uses installation IDs; cfg(test) Provider bindings use
/// installed package paths. Both fixture dialects resolve through the same durable owner.
struct FixtureArtifactResolver {
    store: storage_durable_postgres::MainDurableStore,
    node_id: String,
    install_root: PathBuf,
    installed: ApiRuntimeArtifactResolver,
}

#[async_trait::async_trait]
impl RuntimeArtifactResolver for FixtureArtifactResolver {
    async fn resolve(
        &self,
        artifact: &runtime_core::runtime_backend::RuntimeArtifactReference,
    ) -> Result<PathBuf, runtime_core::runtime_backend::RuntimeBackendError> {
        use runtime_core::runtime_backend::{RuntimeArtifactReference, RuntimeBackendError};
        let reject = || {
            RuntimeBackendError::InvalidRequest(
                "fixture artifact does not identify an exact current-node installation".into(),
            )
        };
        let declared_path = Path::new(artifact.as_str());
        if declared_path.is_absolute() {
            let root = std::fs::canonicalize(&self.install_root).map_err(|_| reject())?;
            let canonical = std::fs::canonicalize(declared_path).map_err(|_| reject())?;
            if canonical != declared_path || !canonical.starts_with(&root) {
                return Err(reject());
            }
            let artifacts = self
                .store
                .list_artifact_instances(&self.node_id)
                .await
                .map_err(|_| reject())?;
            let mut matches = artifacts.iter().filter(|record| {
                record.node_id == self.node_id
                    && record.local_path.as_deref() == Some(artifact.as_str())
            });
            let installation_id = matches.next().ok_or_else(reject)?.installation_id;
            if matches.next().is_some() {
                return Err(reject());
            }
            let reference = RuntimeArtifactReference::new(installation_id.to_string())?;
            let resolved = self.installed.resolve(&reference).await?;
            // A changed durable mapping cannot retarget this exact path admission.
            if resolved != declared_path {
                return Err(reject());
            }
            Ok(resolved)
        } else {
            Uuid::parse_str(artifact.as_str()).map_err(|_| reject())?;
            self.installed.resolve(artifact).await
        }
    }
}

fn package() -> Vec<u8> {
    let raw = std::fs::read_to_string(
        crate::api_workspace_root()
            .unwrap()
            .join("plugins/fixtures/acme.composition-a/manifest.yaml"),
    )
    .unwrap();
    let mut manifest = raw.split("data_models:\n").next().unwrap().to_string();
    manifest.push_str("managed:\n  module:\n    bus_version: v1\n    module_id: acme.composition-a\n    module_version: 1.0.0\n    module_kind: runtime\n    contributions:\n");
    for phase in PHASES {
        manifest.push_str(&format!("      - contribution_id: acme.composition-a.{phase}\n        contributor_module_id: acme.composition-a\n        point_id: 1flowbase.model-definitions.create.{phase}\n        contract_version: 1\n        required_permissions: [hook.model_definitions.create.{phase}]\n        mode: append\n"));
    }
    manifest.push_str("  execution_bindings:\n");
    for phase in PHASES {
        manifest.push_str(&format!("    - contribution_id: acme.composition-a.{phase}\n      execution_mode: process_per_call\n      runtime:\n        protocol: stdio_json\n        entry: bin/worker.py\n      handler: trace.{phase}\n"));
    }
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", manifest.as_bytes())
        .unwrap();
    let executable = std::env::var_os("MANAGED_HOOK_WORKER_FIXTURE")
        .expect("build the real SDK managed_hook_worker example first");
    archive
        .append_path_with_name(executable, "bin/worker.py")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}
fn grant(phase: &str) -> GrantContributionPermission {
    GrantContributionPermission {
        contribution_id: format!("acme.composition-a.{phase}"),
        permission: format!("hook.model_definitions.create.{phase}"),
        resource_scope: domain::ContributionResourceScope::Workspace,
        permission_contract_id: "managed-hook".into(),
        permission_contract_version: "1".into(),
    }
}
impl Fixture {
    async fn new() -> Self {
        Self::new_with_package(package(), PHASES.into_iter().map(grant).collect()).await
    }
    pub(super) async fn new_with_package(
        package_bytes: Vec<u8>,
        grants: Vec<GrantContributionPermission>,
    ) -> Self {
        let (initial, _) = test_api_state_with_database_url().await;
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            vec![],
        )
        .unwrap();
        let host = Arc::new(
            runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
                time::OffsetDateTime::now_utc(),
                Arc::new(FixtureArtifactResolver {
                    store: initial.store.clone(),
                    node_id: initial.api_node_id.clone(),
                    install_root: PathBuf::from(&initial.provider_install_root),
                    installed: ApiRuntimeArtifactResolver::new(
                        initial.store.clone(),
                        &initial.api_node_id,
                        &initial.provider_install_root,
                    ),
                }),
            )
            .unwrap(),
        );
        host.mark_ready().unwrap();
        let services = Arc::new(
            ApiRuntimeServices::new_with_runtime_backend(
                host,
                Arc::new(assembly.compile_graph().unwrap()),
            )
            .unwrap()
            .with_managed_composition(
                initial.store.clone(),
                initial.api_node_id.clone(),
                assembly.module_descriptors().to_vec(),
            ),
        );
        let mut owned = (*initial).clone();
        owned.provider_runtime = services.clone();
        let state = Arc::new(owned);
        let app = crate::app_with_state(state.clone());
        let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
        create_mcp_instance(&app, &cookie, &csrf).await;
        create_model_probe_tool(&app, &cookie, &csrf).await;
        let token = create_api_key(&app, &cookie, &csrf).await;
        let user_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
            .fetch_one(state.store.pool())
            .await
            .unwrap();
        let actor = state
            .store
            .load_actor_context_for_user(user_id)
            .await
            .unwrap();
        let management = PluginManagementService::new(
            state.store.clone(),
            ApiProviderRuntime::new(services),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id);
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: user_id,
                file_name: "acme.composition-a.1flowbasepkg".into(),
                package_bytes,
            })
            .await
            .unwrap();
        let installation_id = installed.installation.id;
        management
            .assign_plugin(AssignPluginCommand {
                actor_user_id: user_id,
                installation_id,
            })
            .await
            .unwrap();
        let authority = PluginContributionAuthorityService::new(
            state.store.clone(),
            HostContributionGrantPolicy::root_composition(),
        );
        for grant in grants {
            authority
                .grant(&actor, installation_id, grant)
                .await
                .unwrap();
        }
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: user_id,
                installation_id,
            })
            .await
            .unwrap();
        let worker =
            Path::new(installed.local_artifact.local_path.as_ref().unwrap()).join("bin/worker.py");
        Self {
            state,
            app,
            cookie,
            csrf,
            token,
            actor,
            installation_id,
            worker,
        }
    }
    pub(super) fn mode(&self, mode: &str) {
        std::fs::write(self.worker.with_extension("mode"), mode).unwrap();
        let _ = std::fs::remove_file(self.worker.with_extension("trace"));
        let _ = std::fs::remove_file(self.worker.with_extension("started"));
        let _ = std::fs::remove_file(self.worker.with_extension("release"));
    }
    fn trace(&self) -> Vec<ManagedHookHostFrame> {
        std::fs::read_to_string(self.worker.with_extension("trace"))
            .unwrap_or_default()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    pub(super) async fn call(&self, mcp: bool, body: Value) -> (u16, Value) {
        if mcp {
            let response = call_mcp(&self.app, &self.token, create_pair::mcp_request(body)).await;
            if response.get("error").is_some() {
                (
                    response["error"]["data"]["http_status"]
                        .as_u64()
                        .unwrap_or(500) as u16,
                    response,
                )
            } else {
                assert_ne!(
                    response["result"]["isError"], true,
                    "protocol=MCP body={response}"
                );
                (201, response["result"]["structuredContent"].clone())
            }
        } else {
            let response = create_pair::http(&self.app, &self.cookie, &self.csrf, body).await;
            let status = response.status().as_u16();
            if status == 422 {
                let bytes = axum::body::to_bytes(response.into_body(), 1024 * 1024)
                    .await
                    .expect("protocol=HTTP status=422: read rejection body");
                return (status, json!(String::from_utf8_lossy(&bytes)));
            }
            let response = response_json(response).await;
            (
                status,
                if status == 201 {
                    response["data"].clone()
                } else {
                    response
                },
            )
        }
    }
    async fn model_count(&self, code: &str) -> i64 {
        sqlx::query_scalar("select count(*) from model_definitions where code=$1")
            .bind(code)
            .fetch_one(self.state.store.pool())
            .await
            .unwrap()
    }
    async fn wait_started(&self) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while !self.worker.with_extension("started").is_file() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the real worker must reach the barrier");
    }
}
fn body(code: &str) -> Value {
    let mut body = create_pair::input();
    body["code"] = json!(code);
    body
}
fn assert_trace(fixture: &Fixture, expected: &[&str]) {
    let trace = fixture.trace();
    assert_eq!(
        trace
            .iter()
            .map(|frame| frame.handler.strip_prefix("trace.").unwrap())
            .collect::<Vec<_>>(),
        expected
    );
    if let Some(first) = trace.first() {
        for frame in &trace {
            assert_eq!(
                frame.context.invocation.invocation_id,
                first.context.invocation.invocation_id
            );
            assert_eq!(
                frame.context.invocation.registry_fingerprint,
                first.context.invocation.registry_fingerprint
            );
            assert_eq!(
                frame.context.invocation.graph_fingerprint,
                first.context.invocation.graph_fingerprint
            );
            assert_eq!(
                frame.context.invocation.authority_revision,
                first.context.invocation.authority_revision
            );
            assert_eq!(
                frame.context.execution_identity.workspace_id().as_str(),
                fixture.actor.current_workspace_id.to_string()
            );
            assert_eq!(
                frame.context.execution_identity.installation_id().as_str(),
                fixture.installation_id.to_string()
            );
            assert_eq!(
                frame.context.actor_id.as_deref(),
                Some(fixture.actor.user_id.to_string().as_str())
            );
        }
    }
}

#[tokio::test]
async fn root_2007_ac_003_004_managed_create_pair() {
    let mut normalized = Vec::new();
    for mcp in [false, true] {
        let fixture = Fixture::new().await;
        fixture.mode("");
        let before = create_pair::persisted(fixture.state.store.pool()).await;
        let started = time::OffsetDateTime::now_utc();
        let (status, model) = fixture.call(mcp, create_pair::input()).await;
        assert_eq!(status, 201, "{model}");
        assert_trace(
            &fixture,
            &[
                "authorization",
                "admission",
                "before",
                "after",
                "completion",
            ],
        );
        let mut after = create_pair::persisted(fixture.state.store.pool()).await;
        assert_eq!(
            after["models"].as_i64().unwrap(),
            before["models"].as_i64().unwrap() + 1
        );
        create_pair::assert_persisted_create(
            fixture.state.store.pool(),
            &model,
            &mut after,
            fixture.actor.current_workspace_id,
            started,
        )
        .await;
        normalized.push(create_pair::normalized_model(
            model,
            fixture.actor.current_workspace_id,
        ));
        for (phase, expected) in [
            (
                "authorization",
                vec!["authorization", "failure", "completion"],
            ),
            (
                "admission",
                vec!["authorization", "admission", "failure", "completion"],
            ),
            (
                "before",
                vec![
                    "authorization",
                    "admission",
                    "before",
                    "failure",
                    "completion",
                ],
            ),
        ] {
            fixture.mode(&format!("deny.{phase}"));
            let code = format!("root2007_deny_{phase}");
            assert_eq!(fixture.call(mcp, body(&code)).await.0, 403);
            assert_eq!(fixture.model_count(&code).await, 0);
            assert_trace(&fixture, &expected);
        }
        fixture.mode("");
        let mut invalid = body("root2007_invalid_template");
        invalid["template_code"] = json!("no-such-template");
        assert!(fixture.call(mcp, invalid).await.0 >= 400);
        assert_eq!(fixture.model_count("root2007_invalid_template").await, 0);
        assert_trace(
            &fixture,
            &[
                "authorization",
                "admission",
                "before",
                "failure",
                "completion",
            ],
        );
        fixture.mode("fail.after");
        assert_eq!(
            fixture.call(mcp, body("root2007_observer_failed")).await.0,
            201
        );
        assert_eq!(fixture.model_count("root2007_observer_failed").await, 1);
        assert_trace(
            &fixture,
            &[
                "authorization",
                "admission",
                "before",
                "after",
                "completion",
            ],
        );
        fixture.mode("timeout.after");
        assert_eq!(
            tokio::time::timeout(
                Duration::from_secs(10),
                fixture.call(mcp, body("root2007_observer_timeout"))
            )
            .await
            .unwrap()
            .0,
            201
        );
        assert_eq!(fixture.model_count("root2007_observer_timeout").await, 1);
        let phases = fixture
            .trace()
            .into_iter()
            .map(|frame| frame.handler)
            .collect::<Vec<_>>();
        assert_eq!(
            &phases[..4],
            [
                "trace.authorization",
                "trace.admission",
                "trace.before",
                "trace.after"
            ]
        );
        fixture.mode("");
        let mut system = body("root2007_system_create");
        system["scope_kind"] = json!("system");
        assert_eq!(fixture.call(mcp, system).await.0, 201);
        assert!(
            fixture.trace().is_empty(),
            "workspace worker must never inspect a system target"
        );
        fixture.mode("");
        let forged_scope = Uuid::now_v7();
        assert_ne!(forged_scope, fixture.actor.current_workspace_id);
        let mut forged = body("root2007_forged_scope");
        forged["scope_id"] = json!(forged_scope);
        let before_forged = create_pair::persisted(fixture.state.store.pool()).await;
        let (status, response) = fixture.call(mcp, forged).await;
        let protocol = if mcp { "MCP" } else { "HTTP" };
        let diagnostic = format!("protocol={protocol} status={status} body={response}");
        if mcp {
            // The probe maps only six legal fields. Unmapped scope_id cannot override
            // the sealed actor's workspace; verify committed facts, not raw DTO rejection.
            assert_eq!(status, 201, "{diagnostic}");
            let rows = sqlx::query_scalar::<_, Value>(
                "select to_jsonb(m) from model_definitions m where code=$1",
            )
            .bind("root2007_forged_scope")
            .fetch_all(fixture.state.store.pool())
            .await
            .unwrap();
            assert_eq!(rows.len(), 1, "{diagnostic}: exactly one committed model");
            assert_eq!(rows[0]["id"], response["id"], "{diagnostic}");
            assert_eq!(rows[0]["scope_kind"], "workspace", "{diagnostic}");
            assert_eq!(
                rows[0]["scope_id"],
                fixture.actor.current_workspace_id.to_string(),
                "{diagnostic}: database ownership must use trusted workspace"
            );
            assert_eq!(
                rows[0]["created_by"],
                fixture.actor.user_id.to_string(),
                "{diagnostic}: database creator must use trusted actor"
            );
            assert_eq!(response["scope_id"], rows[0]["scope_id"], "{diagnostic}");
            let forged_writes: i64 = sqlx::query_scalar(
                "select (select count(*) from model_definitions where scope_id=$1)
                    + (select count(*) from model_fields where scope_id=$1)
                    + (select count(*) from scope_data_model_grants where scope_id=$1)
                    + (select count(*) from audit_logs where scope_id=$1 or workspace_id=$1)",
            )
            .bind(forged_scope)
            .fetch_one(fixture.state.store.pool())
            .await
            .unwrap();
            assert_eq!(forged_writes, 0, "{diagnostic}: no forged workspace writes");
            // Checks all five phases plus invocation/registry/graph/revision correlation,
            // workspace, installation and actor identity on every real worker frame.
            assert_trace(
                &fixture,
                &[
                    "authorization",
                    "admission",
                    "before",
                    "after",
                    "completion",
                ],
            );
        } else {
            assert_eq!(status, 422, "{diagnostic}: HTTP rejects unknown scope_id");
            assert!(
                fixture.trace().is_empty(),
                "{diagnostic}: no worker admission"
            );
            assert_eq!(
                fixture.model_count("root2007_forged_scope").await,
                0,
                "{diagnostic}"
            );
            assert_eq!(
                create_pair::persisted(fixture.state.store.pool()).await,
                before_forged,
                "{diagnostic}: rejection must not persist model, fields, grants or audits"
            );
        }
        core_deny(&fixture, mcp).await;
        frozen_graph(&fixture, mcp).await;
        revoked_between_stages(&fixture, mcp).await;
        cancelled_and_observer_receipt(&fixture).await;
    }
    assert_eq!(normalized[0], normalized[1]);
}

async fn core_deny(fixture: &Fixture, mcp: bool) {
    let role = "root2007_key_only";
    create_role(&fixture.app, &fixture.cookie, &fixture.csrf, role).await;
    replace_role_permissions(
        &fixture.app,
        &fixture.cookie,
        &fixture.csrf,
        role,
        &["settings_feature.access.system.api-key-authentication"],
    )
    .await;
    let member = create_member(
        &fixture.app,
        &fixture.cookie,
        &fixture.csrf,
        "root2007-denied",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &fixture.app,
        &fixture.cookie,
        &fixture.csrf,
        &member,
        &[role],
    )
    .await;
    let (cookie, csrf) =
        login_and_capture_cookie(&fixture.app, "root2007-denied", "temp-pass").await;
    let token = create_api_key(&fixture.app, &cookie, &csrf).await;
    fixture.mode("");
    if mcp {
        let response = call_mcp(
            &fixture.app,
            &token,
            create_pair::mcp_request(body("root2007_core_denied")),
        )
        .await;
        assert_eq!(response["error"]["data"]["http_status"], 403);
    } else {
        assert_eq!(
            create_pair::http(&fixture.app, &cookie, &csrf, body("root2007_core_denied"))
                .await
                .status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(fixture.model_count("root2007_core_denied").await, 0);
    let phases = fixture
        .trace()
        .iter()
        .map(|frame| frame.handler.clone())
        .collect::<Vec<_>>();
    assert!(
        !phases.iter().any(|phase| matches!(
            phase.as_str(),
            "trace.authorization" | "trace.admission" | "trace.before" | "trace.after"
        )),
        "a worker cannot reverse core deny"
    );
}

async fn frozen_graph(fixture: &Fixture, mcp: bool) {
    let composition = fixture
        .state
        .provider_runtime
        .managed_composition()
        .unwrap();
    let old = composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    fixture.mode("barrier.before");
    let call = fixture.call(mcp, body("root2007_frozen_graph"));
    tokio::pin!(call);
    tokio::select! { result = &mut call => panic!("barrier ended early: {result:?}"), _ = fixture.wait_started() => {} }
    let authority = PluginContributionAuthorityService::new(
        fixture.state.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let granted = authority
        .query(&fixture.actor, fixture.installation_id)
        .await
        .unwrap();
    let after = granted
        .authorizations
        .iter()
        .find(|grant| grant.contribution_id == "acme.composition-a.after")
        .unwrap();
    authority
        .revoke(
            &fixture.actor,
            fixture.installation_id,
            RevokeContributionPermission {
                authorization_id: after.id,
                expected_revision: granted.revision,
            },
        )
        .await
        .unwrap();
    authority
        .grant(&fixture.actor, fixture.installation_id, grant("after"))
        .await
        .unwrap();
    composition
        .rebuild_installation(fixture.installation_id)
        .await
        .unwrap();
    let new = composition
        .snapshot(fixture.actor.current_workspace_id)
        .await
        .unwrap();
    assert_ne!(old.graph.fingerprint(), new.graph.fingerprint());
    std::fs::write(fixture.worker.with_extension("release"), "release").unwrap();
    assert_eq!(call.await.0, 201);
    assert_trace(
        fixture,
        &[
            "authorization",
            "admission",
            "before",
            "after",
            "completion",
        ],
    );
    assert_eq!(
        fixture.trace()[4].context.invocation.graph_fingerprint,
        old.graph.fingerprint().as_str()
    );
}

async fn revoked_between_stages(fixture: &Fixture, mcp: bool) {
    fixture.mode("barrier.before");
    let call = fixture.call(mcp, body("root2007_revoked_after"));
    tokio::pin!(call);
    tokio::select! { result = &mut call => panic!("barrier ended early: {result:?}"), _ = fixture.wait_started() => {} }
    let authority = PluginContributionAuthorityService::new(
        fixture.state.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    );
    let granted = authority
        .query(&fixture.actor, fixture.installation_id)
        .await
        .unwrap();
    let after = granted
        .authorizations
        .iter()
        .find(|grant| grant.contribution_id == "acme.composition-a.after")
        .unwrap();
    authority
        .revoke(
            &fixture.actor,
            fixture.installation_id,
            RevokeContributionPermission {
                authorization_id: after.id,
                expected_revision: granted.revision,
            },
        )
        .await
        .unwrap();
    std::fs::write(fixture.worker.with_extension("release"), "release").unwrap();
    assert_eq!(
        call.await.0,
        201,
        "revoking an observer cannot roll back the committed business result"
    );
    assert_eq!(fixture.model_count("root2007_revoked_after").await, 1);
    assert_trace(
        fixture,
        &["authorization", "admission", "before", "completion"],
    );
    authority
        .grant(&fixture.actor, fixture.installation_id, grant("after"))
        .await
        .unwrap();
    fixture
        .state
        .provider_runtime
        .managed_composition()
        .unwrap()
        .rebuild_installation(fixture.installation_id)
        .await
        .unwrap();
}

async fn cancelled_and_observer_receipt(fixture: &Fixture) {
    use crate::routes::{
        console_interface,
        model_definitions::{
            interface::{managed_hooks, ModelDefinitionsInput, ModelDefinitionsOutput},
            CreateModelDefinitionBody,
        },
    };
    use interface_runtime::*;
    let registry = fixture
        .state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let binding = BindingId::new("http.console.model-definitions.create.v1").unwrap();
    let activated = registry.authentication(&binding).unwrap();
    for cancelled in [false, true] {
        fixture.mode(if cancelled {
            "barrier.before"
        } else {
            "fail.after"
        });
        let cancellation = InvocationCancellation::new();
        let code = if cancelled {
            "root2007_cancelled"
        } else {
            "root2007_observer_receipt"
        };
        let envelope = InvocationEnvelope::with_principal_and_controls(
            InvocationLineage::root(InvocationId::now_v7()),
            binding.clone(),
            InterfaceProtocol::Http,
            activated.adapter().clone(),
            activated.activation().clone(),
            UserPrincipal::server_delegation(fixture.actor.clone()),
            InvocationControls::new(None, cancellation.clone(), None),
            ModelDefinitionsInput::Create(
                serde_json::from_value::<CreateModelDefinitionBody>(body(code)).unwrap(),
            ),
        );
        let envelope = managed_hooks::freeze(&fixture.state, &registry, envelope)
            .await
            .unwrap();
        let kernel = console_interface::console_invocation_kernel(
            &fixture.state,
            crate::extension_bus::ConsoleProtocolAdmission::allowed(),
        );
        let invocation = kernel.invoke::<ModelDefinitionsInput, ModelDefinitionsOutput, console_interface::ConsoleInterfaceTargetError>(registry.clone(), envelope);
        tokio::pin!(invocation);
        if cancelled {
            tokio::select! { _ = &mut invocation => panic!("cancel fixture ended before barrier"), _ = fixture.wait_started() => {} }
            cancellation.cancel();
            let failure = match invocation.await {
                Err(failure) => failure,
                Ok(_) => panic!("cancelled Create cannot succeed"),
            };
            assert_eq!(
                failure.receipt().terminal(),
                InterfaceInvocationTerminal::Cancelled
            );
            assert_eq!(fixture.model_count(code).await, 0);
            assert!(fixture
                .trace()
                .iter()
                .any(|frame| frame.handler == "trace.completion"));
        } else {
            let outcome = invocation.await.unwrap();
            assert_eq!(fixture.model_count(code).await, 1);
            assert_eq!(
                outcome
                    .receipt()
                    .observer_records()
                    .iter()
                    .find(|record| record.point() == InterfaceExtensionPoint::After)
                    .unwrap()
                    .status(),
                InterfaceObserverStatus::Failed
            );
            assert_eq!(
                outcome.receipt().terminal(),
                InterfaceInvocationTerminal::Completed
            );
        }
    }
}
