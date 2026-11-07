//! Candidate-bound process + PostgreSQL + canonical HTTP/MCP acceptance. No ACL simulator.
mod cases;
mod package;
mod streaming;
use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_pool_capacity, test_config,
};
use crate::{
    app_state::ApiState,
    managed_publication::ManagedGenerationPublisher,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices},
};
use axum::{
    body::{to_bytes, Body},
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use control_plane::{plugin_management::*, ports::PluginRepository};
use package::{BASE, CODE};
use runtime_core::runtime_backend::{RuntimeArtifactReference, RuntimeBackendError};
use runtime_extension_host::{RuntimeArtifactResolver, RuntimeExtensionHost};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Clone)]
struct Credential {
    cookie: String,
    csrf: String,
    token: String,
}
impl Credential {
    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", self.cookie.parse().unwrap());
        headers.insert("x-csrf-token", self.csrf.parse().unwrap());
        headers
    }
}
struct Fixture {
    app: Router,
    publisher: Arc<ManagedGenerationPublisher>,
    host: Arc<RuntimeExtensionHost>,
    root: Credential,
    member: Credential,
    role_id: Uuid,
    installation: Uuid,
    worker: PathBuf,
    alpha: String,
    beta: String,
    gamma: String,
}
struct ExactResolver {
    store: storage_durable_postgres::MainDurableStore,
    node: String,
    root: PathBuf,
    installed: ApiRuntimeArtifactResolver,
}
#[async_trait::async_trait]
impl RuntimeArtifactResolver for ExactResolver {
    async fn resolve(
        &self,
        artifact: &RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        let reject = || {
            RuntimeBackendError::InvalidRequest(
                "synthetic fixture must use an exact installed artifact".into(),
            )
        };
        let requested = Path::new(artifact.as_str());
        if !requested.is_absolute() {
            Uuid::parse_str(artifact.as_str()).map_err(|_| reject())?;
            return self.installed.resolve(artifact).await;
        }
        let canonical = std::fs::canonicalize(requested).map_err(|_| reject())?;
        let root = std::fs::canonicalize(&self.root).map_err(|_| reject())?;
        if canonical != requested || !canonical.starts_with(&root) {
            return Err(reject());
        }
        let records = self
            .store
            .list_artifact_instances(&self.node)
            .await
            .map_err(|_| reject())?;
        let mut matched = records
            .iter()
            .filter(|r| r.local_path.as_deref() == Some(artifact.as_str()));
        let installation = matched.next().ok_or_else(reject)?.installation_id;
        if matched.next().is_some() {
            return Err(reject());
        }
        let resolved = self
            .installed
            .resolve(&RuntimeArtifactReference::new(installation.to_string())?)
            .await?;
        if resolved != requested {
            return Err(reject());
        }
        Ok(resolved)
    }
}
async fn http(
    app: &Router,
    credential: &Credential,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", &credential.cookie)
                .header("x-csrf-token", &credential.csrf)
                .header("content-type", "application/json")
                .header("origin", "https://record-fixture.example")
                .body(
                    body.map(|b| Body::from(b.to_string()))
                        .unwrap_or_else(Body::empty),
                )
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| json!({"non_json":String::from_utf8_lossy(&bytes)}));
    (status, value)
}
async fn credential(app: &Router, account: &str, password: &str) -> Credential {
    let (cookie, csrf) = login_and_capture_cookie(app, account, password).await;
    let mut result = Credential {
        cookie,
        csrf,
        token: String::new(),
    };
    let (status, body) = http(
        app,
        &result,
        "POST",
        "/api/console/user-api-keys",
        Some(json!({"name":"record permission acceptance","expiration_policy":"never"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    result.token = body["data"]["token"].as_str().unwrap().into();
    result
}
impl Fixture {
    async fn new() -> Self {
        // Fail on a missing finite peer before creating durable fixture resources.
        let package = package::package();
        // ManagedPluginDataService retains one authority transaction while its real inner
        // PluginDataPort executes an effect transaction. This pre-existing service needs two
        // connections to the same isolated DB; the ordinary factory intentionally budgets one.
        let (initial, _) =
            test_api_state_with_database_pool_capacity(std::num::NonZeroU32::new(2).unwrap()).await;
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            vec![],
        )
        .unwrap();
        let host = Arc::new(
            RuntimeExtensionHost::new_with_artifact_resolver_and_plugin_data(
                time::OffsetDateTime::now_utc(),
                Arc::new(ExactResolver {
                    store: initial.store.clone(),
                    node: initial.api_node_id.clone(),
                    root: PathBuf::from(&initial.provider_install_root),
                    installed: ApiRuntimeArtifactResolver::new(
                        initial.store.clone(),
                        &initial.api_node_id,
                        &initial.provider_install_root,
                    ),
                }),
                Arc::new(ManagedPluginDataService::new(
                    Arc::new(initial.store.clone()),
                    Arc::new(initial.store.clone()),
                )),
            )
            .unwrap(),
        );
        host.mark_ready().unwrap();
        let services = Arc::new(
            ApiRuntimeServices::new_with_runtime_backend(
                host.clone(),
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
        let initial_app = crate::app_with_state(state.clone());
        let publisher =
            ManagedGenerationPublisher::new(&state, test_config(), vec![], initial_app).unwrap();
        services
            .managed_composition()
            .unwrap()
            .attach_generation_publisher(Arc::downgrade(&publisher))
            .unwrap();
        let app = publisher.ingress();
        let root = credential(&app, "root", "change-me").await;
        let context = crate::middleware::require_session::require_session(&state, &root.headers())
            .await
            .unwrap();
        let management = PluginManagementService::new(
            state.store.clone(),
            ApiProviderRuntime::new(services),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id)
        .with_managed_activation();
        let installed = management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: context.actor.user_id,
                file_name: "record_fixture.1flowbasepkg".into(),
                package_bytes: package,
            })
            .await
            .unwrap();
        let installation = installed.installation.id;
        management
            .enable_plugin(EnablePluginCommand {
                actor_user_id: context.actor.user_id,
                installation_id: installation,
            })
            .await
            .unwrap();
        let worker = Path::new(installed.local_artifact.local_path.as_ref().unwrap())
            .join("bin/record_permission_worker");
        assert!(publisher
            .current_generation()
            .unwrap()
            .console_surface_registry
            .managed_services()
            .iter()
            .any(|s| s.installation_id == installation));
        let (status,body)=http(&app,&root,"POST","/api/console/mcp/instances",Some(json!({"instance_id":"taichuy","name":"Record acceptance","description_short":null,"status":"enabled","default_entry_path":"/"}))).await;
        assert_eq!(status, StatusCode::CREATED, "{body}");
        let role = "record_operator";
        create_role(&app, &root.cookie, &root.csrf, role).await;
        // Reuse the existing API-key service's legacy eligibility plus its current console policy.
        replace_role_permissions(
            &app,
            &root.cookie,
            &root.csrf,
            role,
            &["settings_feature.access.system.api-key-authentication"],
        )
        .await;
        let (status,body)=http(&app,&root,"PUT",&format!("/api/console/settings/roles/{role}/console-policy"),Some(json!({"groups":[{"kind":"plugin","group_id":CODE,"enabled":true,"strategy":"full","operations":[]},{"kind":"settings_feature","group_id":"system.api-key-authentication","enabled":true,"strategy":"full","operations":[]}]}))).await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{body}");
        let member_id =
            create_member(&app, &root.cookie, &root.csrf, "record-member", "temp-pass").await;
        replace_member_roles(&app, &root.cookie, &root.csrf, &member_id, &[role]).await;
        let member = credential(&app, "record-member", "temp-pass").await;
        for name in ["alpha", "beta", "gamma"] {
            let (status, body) = http(
                &app,
                &root,
                "POST",
                &format!("{BASE}/records"),
                Some(json!({"name":name})),
            )
            .await;
            assert_eq!(
                status,
                StatusCode::OK,
                "{body}; finite worker stages: {}",
                std::fs::read_to_string(worker.with_extension("trace"))
                    .unwrap_or_default()
                    .chars()
                    .take(4000)
                    .collect::<String>()
            );
        }
        let (status, body) = http(
            &app,
            &root,
            "GET",
            &format!("{BASE}/records?limit=100&offset=0"),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["data"]["total"], 3);
        let id = |name: &str| {
            body["data"]["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["name"] == name)
                .unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned()
        };
        let alpha = id("alpha");
        let beta = id("beta");
        let gamma = id("gamma");
        let current = publisher.current_generation().unwrap();
        let role_id =
            control_plane::record_permissions::RecordPermissionService::new(current.store.clone())
                .roles(&context.actor)
                .await
                .unwrap()
                .into_iter()
                .find(|r| r.code == role)
                .unwrap()
                .role_id;
        Self {
            app,
            publisher,
            host,
            root,
            member,
            role_id,
            installation,
            worker,
            alpha,
            beta,
            gamma,
        }
    }
    fn state(&self) -> Arc<ApiState> {
        self.publisher.current_generation().unwrap().state
    }
    async fn request(
        &self,
        root: bool,
        method: &str,
        suffix: &str,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        http(
            &self.app,
            if root { &self.root } else { &self.member },
            method,
            &format!("{BASE}{suffix}"),
            body,
        )
        .await
    }
    async fn policy(&self, id: &str, mode: &str, actions: &[&str]) -> Value {
        let path=format!("/api/console/settings/roles/record-permissions?plugin_code={CODE}&resource_code=records&record_id={id}");
        let (status, before) = http(&self.app, &self.root, "GET", &path, None).await;
        assert_eq!(status, StatusCode::OK, "{before}");
        let grants = if actions.is_empty() {
            json!([])
        } else {
            json!([{"role_id":self.role_id,"actions":actions}])
        };
        let (status, after) = http(
            &self.app,
            &self.root,
            "PUT",
            &path,
            Some(
                json!({"mode":mode,"grants":grants,"expected_revision":before["data"]["revision"]}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{after}");
        assert!(
            after["data"].get("resource").is_none(),
            "PUT contract is a policy, not the editor aggregate"
        );
        after["data"].clone()
    }
    async fn rpc(&self, root: bool, request: Value) -> Value {
        let token = if root {
            &self.root.token
        } else {
            &self.member.token
        };
        let response = self
            .app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/mcp/taichuy")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .header("mcp-protocol-version", "2025-11-25")
                    .body(Body::from(request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
    }
    async fn mcp(&self, root: bool, handler: &str, args: Value) -> Value {
        self.rpc(root,json!({"jsonrpc":"2.0","id":42,"method":"tools/call","params":{"name":"mcp_call","arguments":{"tool_id":format!("plugin.{CODE}.{handler}"),"max_inline_chars":12000,"arguments":args}}})).await
    }
    async fn actor(&self, root: bool) -> domain::ActorContext {
        crate::middleware::require_session::require_session(
            &self.state(),
            &if root {
                self.root.headers()
            } else {
                self.member.headers()
            },
        )
        .await
        .unwrap()
        .actor
    }
}
fn mcp_success(result: &Value) -> &Value {
    assert!(result.get("error").is_none(), "{result}");
    assert_ne!(result["result"]["isError"], true, "{result}");
    &result["result"]["structuredContent"]
}
fn mcp_denied(result: &Value) {
    assert!(
        result.get("error").is_some() || result["result"]["isError"] == true,
        "unexpected success: {result}"
    );
}

#[tokio::test]
#[ignore = "requires RECORD_PERMISSION_WORKER_FIXTURE built by record-permissions CI"]
async fn real_http_mcp_record_acl_sessions_and_stream_completion() {
    use futures_util::FutureExt;
    let fixture = Fixture::new().await;
    let outcome = std::panic::AssertUnwindSafe(async {
        tokio::time::timeout(Duration::from_secs(120), cases::run(&fixture))
            .await
            .expect("finite record permission acceptance deadline");
    })
    .catch_unwind()
    .await;
    // Shared session retention and process resources are reaped even when an assertion fails.
    fixture
        .host
        .drain()
        .await
        .expect("fixture Runtime Host drain");
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
}
