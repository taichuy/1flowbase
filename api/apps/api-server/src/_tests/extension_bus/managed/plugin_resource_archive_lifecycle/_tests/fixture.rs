use super::{
    inputs::{sha, Archive},
    loopback_ssh::LoopbackSsh,
};
use crate::_tests::extension_bus::managed_snapshot_tests::RuntimeFixture;
use crate::{
    managed_publication::ManagedGenerationPublisher, provider_runtime::ApiProviderRuntime,
};
use anyhow::{ensure, Context, Result};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use control_plane::{plugin_management::*, ports::*, record_permissions::RecordPermissionService};
use control_plane_contracts::PluginCredentialRepository;
use domain::*;
use extension_contracts::*;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::Arc,
};
use storage_durable_postgres::{MainDurableStore, PgPluginCredentialRepository};
use tower::ServiceExt;
use uuid::Uuid;

pub struct Fixture {
    pub state: Arc<crate::app_state::ApiState>,
    pub runtime: RuntimeFixture,
    pub management: crate::app_state::ApiPluginManagementService,
    pub publisher: Arc<ManagedGenerationPublisher>,
    pub ingress: Router,
    pub actor: ActorContext,
    pub member: ActorContext,
    cookie: String,
    csrf: String,
    member_cookie: String,
    member_csrf: String,
}
impl Fixture {
    pub async fn new() -> Result<Self> {
        let (initial, _) = crate::_tests::support::test_api_state_with_database_pool_capacity(
            // Two live sources each retain consumer authority while using the adapter;
            // retain one connection for HTTP/session/ACL observation (normal local default).
            std::num::NonZeroU32::new(5).unwrap(),
        )
        .await;
        let runtime = RuntimeFixture::new_with_managed_host_calls(&initial);
        let mut owned = (*initial).clone();
        owned.store = runtime.store.clone();
        owned.provider_runtime = runtime.services.clone();
        let state = Arc::new(owned);
        let account = std::env::var("BOOTSTRAP_ROOT_ACCOUNT").unwrap_or_else(|_| "root".into());
        let password =
            std::env::var("BOOTSTRAP_ROOT_PASSWORD").unwrap_or_else(|_| "change-me".into());
        let id = sqlx::query_scalar("select id from users where account=$1")
            .bind(&account)
            .fetch_one(state.store.pool())
            .await?;
        let actor = AuthRepository::load_actor_context_for_user(&state.store, id).await?;
        ensure!(actor.is_root, "real bootstrap Root required");
        let member = state
            .store
            .create_member_with_default_role(&CreateMemberInput {
                actor_user_id: id,
                workspace_id: actor.current_workspace_id,
                account: "archive-member".into(),
                email: "archive-member@example.invalid".into(),
                phone: None,
                password_hash: "$argon2id$v=19$m=19456,t=2,p=1$test$test".into(),
                name: "Archive Member".into(),
                nickname: "Archive Member".into(),
                introduction: String::new(),
                email_login_enabled: true,
                phone_login_enabled: false,
            })
            .await?;
        let member = AuthRepository::load_actor_context_for_user(&state.store, member.id).await?;
        ensure!(!member.is_root, "real nonroot member required");
        let initial_app = crate::app_with_state(state.clone());
        let publisher = ManagedGenerationPublisher::new(
            &state,
            crate::_tests::support::test_config(),
            vec![],
            initial_app,
        )?;
        runtime
            .composition
            .attach_generation_publisher(Arc::downgrade(&publisher))?;
        let ingress = publisher.ingress();
        let (cookie, csrf) =
            crate::_tests::support::login_and_capture_cookie(&ingress, &account, &password).await;
        // Reuse the repository session helper; authentication still reloads the actual user,
        // role and console policy through the real HTTP middleware on every request.
        let user = state
            .store
            .find_user_by_id(member.user_id)
            .await?
            .context("member user missing")?;
        let session_id = Uuid::now_v7().to_string();
        let member_csrf = Uuid::now_v7().to_string();
        crate::_tests::support::seed_session(
            &state,
            SessionRecord {
                session_id: session_id.clone(),
                user_id: member.user_id,
                tenant_id: member.tenant_id,
                current_workspace_id: member.current_workspace_id,
                active_role_code: member.effective_display_role.clone(),
                session_version: user.session_version,
                csrf_token: member_csrf.clone(),
                expires_at_unix: time::OffsetDateTime::now_utc().unix_timestamp() + 3600,
            },
        )
        .await;
        let member_cookie = format!("{}={session_id}", state.cookie_name);
        let management = PluginManagementService::new(
            runtime.store.clone(),
            ApiProviderRuntime::new(runtime.services.clone()),
            state.official_plugin_source.clone(),
            &state.provider_install_root,
        )
        .with_node_id(&state.api_node_id)
        .with_managed_activation();
        Ok(Self {
            state,
            runtime,
            management,
            publisher,
            ingress,
            actor,
            member,
            cookie,
            csrf,
            member_cookie,
            member_csrf,
        })
    }
    pub fn authority(&self) -> PluginContributionAuthorityService<MainDurableStore> {
        PluginContributionAuthorityService::new(
            self.runtime.store.clone(),
            HostContributionGrantPolicy::root_composition(),
        )
        .with_node_id(&self.state.api_node_id)
    }
    pub async fn upload(
        &self,
        a: &Archive,
    ) -> Result<(
        InstallPluginResult,
        plugin_framework::PluginManifestV1,
        String,
    )> {
        let installed = self
            .management
            .install_uploaded_plugin(InstallUploadedPluginCommand {
                actor_user_id: self.actor.user_id,
                file_name: a.filename.clone(),
                package_bytes: a.bytes.clone(),
            })
            .await
            .context("actual uploaded archive intake failed")?;
        let i = &installed.installation;
        ensure!(
            i.organization == a.publisher
                && i.provider_code == a.code
                && i.plugin_version == a.version,
            "archive identity mismatch"
        );
        ensure!(
            i.expected_checksum.as_deref() == Some(format!("sha256:{}", a.sha).as_str()),
            "installed checksum mismatch"
        );
        self.inactive(i.id).await?;
        ensure!(
            self.authority()
                .query(&self.actor, i.id)
                .await?
                .authorizations
                .is_empty(),
            "upload alone minted grants"
        );
        let local = Path::new(
            installed
                .local_artifact
                .local_path
                .as_ref()
                .context("installed local path")?,
        )
        .canonicalize()?;
        ensure!(
            local.starts_with(Path::new(&self.state.provider_install_root).canonicalize()?),
            "artifact escaped test root"
        );
        let source = fs::read(local.join("manifest.yaml"))?;
        let manifest = plugin_framework::parse_plugin_manifest(std::str::from_utf8(&source)?)?;
        ensure!(
            manifest.publisher_namespace == a.publisher
                && manifest.plugin_code()? == a.code
                && manifest.version == a.version,
            "manifest identity mismatch"
        );
        if let Some(expected) = &a.source {
            ensure!(
                &source == expected,
                "manifest differs from approved BUSINESS source"
            );
        }
        let runtime_sha = sha(&fs::read(local.join(&manifest.runtime.entry))?);
        if let Some(expected) = &a.runtime_sha {
            ensure!(
                &runtime_sha == expected,
                "archive does not contain fresh BUSINESS executable"
            );
        }
        Ok((installed, manifest, runtime_sha))
    }
    pub async fn active(&self, id: Uuid) -> Result<()> {
        let i = self
            .runtime
            .store
            .get_installation(id)
            .await?
            .context("installation missing")?;
        let a = self
            .runtime
            .store
            .get_artifact_instance(&self.state.api_node_id, id)
            .await?
            .context("artifact missing")?;
        ensure!(
            i.desired_state == PluginDesiredState::ActiveRequested
                && a.runtime_status == PluginRuntimeStatus::Active,
            "real enable did not reach active runtime"
        );
        Ok(())
    }
    pub async fn inactive(&self, id: Uuid) -> Result<()> {
        let i = self
            .runtime
            .store
            .get_installation(id)
            .await?
            .context("installation missing")?;
        let a = self
            .runtime
            .store
            .get_artifact_instance(&self.state.api_node_id, id)
            .await?
            .context("artifact missing")?;
        ensure!(
            i.desired_state == PluginDesiredState::Disabled
                && a.runtime_status == PluginRuntimeStatus::Inactive,
            "runtime not disabled/inactive"
        );
        Ok(())
    }
    pub fn generation(&self) -> Result<Uuid> {
        Ok(self
            .publisher
            .current_state()
            .context("serving state")?
            .extension_boot_snapshot
            .as_ref()
            .context("boot snapshot")?
            .generation_id())
    }
    pub async fn allow_member_hosts_list(&self) -> Result<()> {
        self.allow_member_operations(false).await
    }
    pub async fn allow_member_aggregate(&self) -> Result<()> {
        self.allow_member_operations(true).await
    }
    async fn allow_member_operations(&self, aggregate: bool) -> Result<()> {
        let role_code = sqlx::query_scalar(
            "select code from roles where workspace_id=$1 and is_default_member_role=true",
        )
        .bind(self.member.current_workspace_id)
        .fetch_one(self.runtime.store.pool())
        .await?;
        RoleRepository::replace_role_console_policy(
            &self.runtime.store,
            &ReplaceRoleConsolePolicyInput {
                actor_user_id: self.actor.user_id,
                workspace_id: self.member.current_workspace_id,
                role_code,
                groups: vec![RoleConsoleGroupPolicy::custom(
                    ConsolePolicyGroup::plugin("compute-monitor")?,
                    [
                        "compute-monitor.hosts.list",
                        "compute-monitor.monitor.snapshot",
                    ]
                    .into_iter()
                    .chain(
                        aggregate
                            .then_some([
                                "compute-monitor.monitor.aggregate",
                                "compute-monitor.monitor.stream",
                                "compute-monitor.monitor.resolve",
                            ])
                            .into_iter()
                            .flatten(),
                    )
                    .map(|operation| {
                        Ok(ConsoleOperationPolicy::simple(
                            ConsoleOperationId::try_from(operation)?,
                            true,
                        ))
                    })
                    .collect::<Result<Vec<_>>>()?,
                )],
            },
        )
        .await?;
        Ok(())
    }
    pub fn stream_auth(&self, member: bool) -> (&str, &str) {
        if member {
            (&self.member_cookie, &self.member_csrf)
        } else {
            (&self.cookie, &self.csrf)
        }
    }
    pub async fn get(&self, path: &str, authenticated: bool) -> Result<(StatusCode, Value)> {
        self.get_with_cookie(path, authenticated.then_some(self.cookie.as_str()))
            .await
    }
    pub async fn get_as_member(&self, path: &str) -> Result<(StatusCode, Value)> {
        self.get_with_cookie(path, Some(&self.member_cookie)).await
    }
    async fn get_with_cookie(
        &self,
        path: &str,
        cookie: Option<&str>,
    ) -> Result<(StatusCode, Value)> {
        self.request("GET", path, cookie, None, None).await
    }
    pub async fn post(&self, path: &str, body: Value, member: bool) -> Result<(StatusCode, Value)> {
        let (cookie, csrf) = if member {
            (&self.member_cookie, &self.member_csrf)
        } else {
            (&self.cookie, &self.csrf)
        };
        self.request("POST", path, Some(cookie), Some(csrf), Some(body))
            .await
    }
    pub async fn delete(&self, path: &str) -> Result<(StatusCode, Value)> {
        self.request("DELETE", path, Some(&self.cookie), Some(&self.csrf), None)
            .await
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        csrf: Option<&str>,
        body: Option<Value>,
    ) -> Result<(StatusCode, Value)> {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        if let Some(csrf) = csrf {
            request = request.header("x-csrf-token", csrf);
        }
        let body = match body {
            Some(body) => {
                request = request.header("content-type", "application/json");
                Body::from(body.to_string())
            }
            None => Body::empty(),
        };
        let response = self.ingress.clone().oneshot(request.body(body)?).await?;
        let status = response.status();
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await?;
        Ok((
            status,
            serde_json::from_slice(&bytes).context("actual HTTP response must be JSON")?,
        ))
    }
    async fn mcp(&self, token: &str, name: &str, arguments: Value) -> Result<Value> {
        let response = self
            .ingress
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/mcp/compute_monitor")
                    .header("authorization", format!("Bearer {token}"))
                    .header("content-type", "application/json")
                    .header("mcp-protocol-version", "2025-11-25")
                    .body(Body::from(
                        json!({"jsonrpc":"2.0","id":2896,"method":"tools/call",
                "params":{"name":name,"arguments":arguments}})
                        .to_string(),
                    ))?,
            )
            .await?;
        let status = response.status();
        let wire: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await?)?;
        ensure!(
            status == StatusCode::OK
                && wire["error"].is_null()
                && wire["result"]["isError"] == false,
            "actual MCP {name} failed: {status}; {wire}"
        );
        Ok(wire["result"]["structuredContent"].clone())
    }
    pub async fn mcp_snapshot(&self, host: &str) -> Result<Value> {
        // Use the genuine package's enabled instance/tool. No replacement tool or mock dispatch.
        let (status, key) = self
            .post(
                "/api/console/user-api-keys",
                json!({"name":"Archive loopback snapshot","expiration_policy":"never"}),
                false,
            )
            .await?;
        ensure!(
            status == StatusCode::CREATED,
            "cannot provision disposable Root MCP key"
        );
        let token = key["data"]["token"].as_str().context("MCP token missing")?;
        let tool_id = "plugin.compute-monitor.monitor_snapshot";
        let listed = self
            .mcp(
                token,
                "mcp_list",
                json!({"path":"/compute-monitor","depth":1,"limit":50}),
            )
            .await?;
        ensure!(
            listed["result"]
                .as_array()
                .is_some_and(|items| items.iter().any(|item| item["id"] == tool_id)),
            "genuine monitor snapshot absent from actual MCP discovery"
        );
        let tool = self
            .mcp(token, "mcp_get", json!({"tool_id":tool_id}))
            .await?;
        ensure!(
            tool["tool_id"] == tool_id
                && tool["input_schema"]["properties"]["host_id"]["type"] == "string"
                && tool["input_schema"]["properties"]["sample_interval_ms"]["type"] == "integer"
                && tool["result_schema"]["properties"]["host_id"]["type"] == "string",
            "actual MCP schema does not describe the production snapshot target"
        );
        ensure!(
            tool["description_tool_id"].is_null(),
            "unexpected separate description tool: do not bypass it"
        );
        self.mcp(
            token,
            "mcp_call",
            json!({"tool_id":tool_id,"des_id":tool["des_id"],
            "max_inline_chars":512000,"arguments":{"host_id":host,"sample_interval_ms":250}}),
        )
        .await
    }
}

pub struct Preserved {
    binding: PluginDataBinding,
    reads: Vec<(PluginDataRequest, PluginDataResponse)>,
    credential: PluginCredentialBinding,
    credentials: PgPluginCredentialRepository,
    secret: Value,
    credential_id: String,
    acl_key: RecordPermissionKey,
    acl: RecordPermissionPolicy,
    hidden_acl_key: RecordPermissionKey,
    hidden_acl: RecordPermissionPolicy,
    snapshot: Value,
}
fn field_value(
    field: &PluginOwnedField,
    host: &str,
    credential: &str,
    ssh: &LoopbackSsh,
) -> PluginDataValue {
    let text = match field.field_code.as_str() {
        "host_id" => host.to_owned(),
        "credential_id" => credential.to_owned(),
        "address" => "127.0.0.1".into(),
        "username" => ssh.username.clone(),
        "auth_type" => "private_key".into(),
        "fingerprint" => ssh.fingerprint.clone(),
        _ => format!("fixture-{}", field.field_code),
    };
    match field.field_type {
        PluginDataFieldType::String | PluginDataFieldType::Text => PluginDataValue::String(text),
        PluginDataFieldType::Number => PluginDataValue::Number(
            match field.field_code.as_str() {
                "port" => return PluginDataValue::Number(ssh.port.to_string()),
                "timeout_ms" => "30000",
                "max_output_bytes" => "1048576",
                _ => "12345",
            }
            .into(),
        ),
        PluginDataFieldType::Boolean => PluginDataValue::Boolean(true),
        PluginDataFieldType::Datetime => PluginDataValue::Datetime("2026-11-21T00:00:00Z".into()),
        PluginDataFieldType::Uuid => PluginDataValue::Uuid(Uuid::now_v7().to_string()),
        PluginDataFieldType::Json => PluginDataValue::Json(match field.field_code.as_str() {
            "credential_ids" | "retired_credential_ids" => json!([credential]),
            "jump_hosts" => json!([]),
            "connection_options" => json!({}),
            _ => json!({"preserve":true}),
        }),
    }
}
async fn snapshot(f: &Fixture) -> Result<Value> {
    // Read-only observations. No installation state, readiness or registry fixtures are written.
    Ok(sqlx::query_scalar("select jsonb_build_object('host_oid',to_regclass('plg_6ad77626fdd9_ssh_hosts')::oid::bigint,'credentials',(select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]') from plugin_credentials t),'policies',(select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]') from plugin_record_permission_policies t),'grants',(select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]') from plugin_record_permission_grants t),'revisions',(select coalesce(jsonb_agg(to_jsonb(t) order by to_jsonb(t)::text),'[]') from plugin_record_permission_scope_revisions t))")
        .fetch_one(f.runtime.store.pool()).await?)
}
impl Preserved {
    pub async fn seed(
        f: &Fixture,
        id: Uuid,
        manifest: &plugin_framework::PluginManifestV1,
        ssh: &LoopbackSsh,
        pending_recovery: bool,
    ) -> Result<Self> {
        let host = Uuid::now_v7().to_string();
        let hidden_host = Uuid::now_v7().to_string();
        let credential_id = Uuid::now_v7().to_string();
        let binding = PluginDataBinding {
            managed_subject: None,
            record_permissions: None,
            publisher_namespace: "taichuy".into(),
            plugin_code: "ssh".into(),
            plugin_version: "0.1.21".into(),
            storage_binding: "main".into(),
            workspace_id: SYSTEM_SCOPE_ID.to_string(),
            actor_id: Some(f.actor.user_id.to_string()),
            provider_instance_id: id.to_string(),
            permissions: BTreeSet::from([PluginDataPermission::Read, PluginDataPermission::Write]),
            deadline_unix_ms: i64::MAX,
        };
        let mut reads = Vec::new();
        for model in &manifest.data_models {
            ensure!(
                model.extension_fields.is_empty(),
                "unexpected old extension-field schema"
            );
            for collection in &model.owned_collections {
                let target = PluginDataTarget::OwnedCollection {
                    collection_code: collection.collection_code.clone(),
                };
                // Two provider-owned hosts: one member-view grant and one explicit deny.
                // Other historical collections retain their original single-row coverage.
                // R6 retains an old pending-recovery ledger row through upgrade. R7 uses a
                // clean historical ledger so later, intentional SSH management does not
                // legitimately garbage-collect that row. Both retain all historical host rows.
                let row_ids = if collection.collection_code == "ssh_credential_staging"
                    && !pending_recovery
                {
                    vec![]
                } else if collection.collection_code == "ssh_hosts" {
                    vec![host.as_str(), hidden_host.as_str()]
                } else {
                    vec![host.as_str()]
                };
                let expected_rows = row_ids.len();
                for row_id in row_ids {
                    let values = collection
                        .fields
                        .iter()
                        .map(|field| {
                            (
                                field.field_code.clone(),
                                field_value(field, row_id, &credential_id, ssh),
                            )
                        })
                        .collect::<BTreeMap<_, _>>();
                    f.runtime
                        .store
                        .execute(
                            &binding,
                            &PluginDataRequest {
                                idempotency_key: None,
                                operations: vec![PluginDataOperation::Insert {
                                    target: target.clone(),
                                    values,
                                }],
                            },
                        )
                        .await?;
                }
                let read = PluginDataRequest {
                    idempotency_key: None,
                    operations: vec![PluginDataOperation::Find {
                        target,
                        fields: std::iter::once("id".into())
                            .chain(collection.fields.iter().map(|f| f.field_code.clone()))
                            .collect(),
                        filters: vec![],
                        order: vec![PluginDataOrder {
                            field: "id".into(),
                            direction: PluginDataOrderDirection::Ascending,
                        }],
                        page: PluginDataPage {
                            limit: 3,
                            offset: 0,
                        },
                    }],
                };
                let response = f.runtime.store.execute(&binding, &read).await?;
                ensure!(
                    matches!(&response.results[..], [PluginDataOperationResult::Rows { rows }] if rows.len() == expected_rows),
                    "historical complete rows missing"
                );
                reads.push((read, response));
            }
        }
        ensure!(!reads.is_empty(), "old archive must own data");
        let credential = PluginCredentialBinding {
            contribution_id: "ssh.hosts.test".into(),
            installation_id: id.to_string(),
            publisher_namespace: "taichuy".into(),
            plugin_code: "ssh".into(),
            plugin_version: "0.1.21".into(),
            scope_id: SYSTEM_SCOPE_ID.to_string(),
            deadline_unix_ms: i64::MAX,
        };
        let credentials = PgPluginCredentialRepository::new(
            f.runtime.store.pool().clone(),
            f.state.provider_secret_master_key.clone(),
        )?;
        let secret = json!({"password":null,"private_key":ssh.private_key()?,"passphrase":null});
        credentials
            .execute_plugin_credential(
                &credential,
                &PluginCredentialRequest::Put {
                    credential_id: credential_id.clone(),
                    value: secret.clone(),
                },
            )
            .await?;
        let acl_key = RecordPermissionKey {
            workspace_id: f.actor.current_workspace_id,
            resource: RecordPermissionResourceKey {
                publisher_namespace: "taichuy".into(),
                plugin_code: "ssh".into(),
                resource_code: "hosts".into(),
            },
            record_id: host,
        };
        let service = RecordPermissionService::new(f.runtime.store.clone());
        let roles = RecordPermissionRepository::record_permission_effective_role_ids(
            &f.runtime.store,
            &f.member,
        )
        .await?;
        let role = service
            .roles(&f.actor)
            .await?
            .into_iter()
            .find(|r| roles.contains(&r.role_id))
            .context("member role")?;
        let acl = service
            .replace(
                &f.actor,
                &acl_key,
                &ReplaceRecordPermissions {
                    mode: RecordPermissionMode::Restricted,
                    grants: vec![RecordPermissionGrant {
                        role_id: role.role_id,
                        actions: vec!["view".into()],
                    }],
                    expected_revision: Some(0),
                },
                &["view".into(), "update".into()],
            )
            .await?;
        let hidden_acl_key = RecordPermissionKey {
            record_id: hidden_host,
            ..acl_key.clone()
        };
        let hidden_acl = service
            .replace(
                &f.actor,
                &hidden_acl_key,
                &ReplaceRecordPermissions {
                    mode: RecordPermissionMode::Restricted,
                    grants: vec![],
                    expected_revision: Some(0),
                },
                &["view".into(), "update".into()],
            )
            .await?;
        let snapshot = snapshot(f).await?;
        ensure!(
            !snapshot["host_oid"].is_null(),
            "original taichuy/ssh physical table identity missing"
        );
        Ok(Self {
            binding,
            reads,
            credential,
            credentials,
            secret,
            credential_id,
            acl_key,
            acl,
            hidden_acl_key,
            hidden_acl,
            snapshot,
        })
    }
    pub async fn check_hosts_list(&self, f: &Fixture, ssh: &LoopbackSsh) -> Result<()> {
        let path = "/api/console/managed-services/compute-monitor/hosts?page=1&page_size=50";
        let (status, root) = f.get(path, true).await?;
        ensure!(
            status == StatusCode::OK,
            "authenticated Root worker hosts.list failed: {status}"
        );
        let data = &root["data"];
        let items = data["items"]
            .as_array()
            .context("Root hosts.list items missing")?;
        ensure!(
            items.len() == 2 && data["total"] == 2 && data["page"] == 1 && data["page_size"] == 50,
            "Root must see both original provider hosts and exact count"
        );
        let visible = items
            .iter()
            .find(|row| row["host_id"] == self.acl_key.record_id)
            .context("Root response lacks original member-visible host")?;
        ensure!(
            items
                .iter()
                .any(|row| row["host_id"] == self.hidden_acl_key.record_id),
            "Root response lacks original member-denied host"
        );
        ensure!(
            visible["name"] == "fixture-name"
                && visible["address"] == "127.0.0.1"
                && visible["port"] == ssh.port
                && visible["username"] == ssh.username
                && visible["auth_type"] == "private_key"
                && visible["fingerprint"] == ssh.fingerprint
                && visible["enabled"] == true
                && visible["credential_configured"] == true,
            "worker did not project original taichuy/ssh provider fields"
        );
        ensure!(
            items.iter().all(|row| row.get("credential_id").is_none()
                && row.get("password").is_none()
                && row.get("private_key").is_none())
                && !root.to_string().contains("BEGIN OPENSSH PRIVATE KEY")
                && !root.to_string().contains(&self.credential_id),
            "hosts.list exposed private credential data"
        );
        f.allow_member_hosts_list().await?;
        let (status, member) = f.get_as_member(path).await?;
        ensure!(
            status == StatusCode::OK,
            "authenticated member worker hosts.list failed: {status}; {member}"
        );
        ensure!(
            member["data"]["items"] == json!([visible])
                && member["data"]["total"] == 1
                && member["data"]["page"] == 1
                && member["data"]["page_size"] == 50,
            "member must see the granted provider host only; deny must filter rows AND count"
        );
        Ok(())
    }
    pub async fn check_monitor_snapshot(&self, f: &Fixture, ssh: &LoopbackSsh) -> Result<()> {
        let path = format!(
            "/api/console/managed-services/compute-monitor/hosts/{}/monitor/snapshot",
            self.acl_key.record_id
        );
        let body = json!({"sample_interval_ms":250});
        let before = ssh.sessions()?;
        // Console operation is allowed, but the historical provider ACL grants view, NOT open.
        // The same original host must remain denied even after Root populates snapshot cache.
        for host in [&self.acl_key.record_id, &self.hidden_acl_key.record_id] {
            let denied_path = format!(
                "/api/console/managed-services/compute-monitor/hosts/{host}/monitor/snapshot"
            );
            let (status, denied) = f.post(&denied_path, body.clone(), true).await?;
            ensure!(
                status == StatusCode::FORBIDDEN && denied["code"] == "record_permission_denied",
                "view-only/hidden member must not read monitor snapshot: {status}; {denied}"
            );
        }
        ensure!(
            ssh.sessions()? == before,
            "denied member started SSH commands"
        );
        let started = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64;
        let (status, response) = f.post(&path, body.clone(), false).await?;
        ensure!(
            status == StatusCode::OK,
            "authenticated Root production worker snapshot failed: {status}; {response}"
        );
        ssh.assert_sample(&response["data"], &self.acl_key.record_id, started)?;
        ensure!(
            ssh.sessions()? >= before + 2,
            "HTTP snapshot must execute two real SSH probe commands"
        );
        self.no_secrets(&response)?;
        let (status, denied) = f.post(&path, body, true).await?;
        ensure!(
            status == StatusCode::FORBIDDEN && denied["code"] == "record_permission_denied",
            "Root snapshot/cache bypassed member open ACL: {status}; {denied}"
        );

        eprintln!(
            "R6_HTTP_SNAPSHOT: authenticated Root -> genuine worker -> production credential HostCall -> preserved taichuy/ssh credential -> pinned loopback SSH; member open denied before/after cache"
        );
        // Allow the production projection's 3 * 250ms TTL to expire. Do not delete host
        // cache entries or pretend an HTTP cache hit is a fresh MCP credential/SSH call.
        tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        // This is actual /api/mcp + package discovery/schema/target dispatch, not debug_execute.
        let started = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64;
        let before = ssh.sessions()?;
        let snapshot = f.mcp_snapshot(&self.acl_key.record_id).await?;
        ssh.assert_sample(&snapshot, &self.acl_key.record_id, started)?;
        ensure!(
            ssh.sessions()? >= before + 2,
            "MCP snapshot must execute fresh real SSH probes, not reuse an HTTP-only result"
        );
        self.no_secrets(&snapshot)?;
        eprintln!(
            "R6_MCP_SNAPSHOT: actual package mcp_list/get schema -> mcp_call -> production target -> fresh pinned loopback SSH probes; no credential fields returned"
        );
        Ok(())
    }
    pub fn host_ids(&self) -> (&str, &str) {
        (&self.acl_key.record_id, &self.hidden_acl_key.record_id)
    }
    pub fn no_secrets(&self, value: &Value) -> Result<()> {
        let text = value.to_string();
        ensure!(
            !text.contains(&self.credential_id)
                && !text.contains("BEGIN OPENSSH PRIVATE KEY")
                && !text.contains("credential_id")
                && !text.contains("private_key")
                && !text.contains("password"),
            "snapshot exposed a credential reference or secret"
        );
        Ok(())
    }
    pub async fn check(&self, f: &Fixture, id: Uuid) -> Result<()> {
        for (read, expected) in &self.reads {
            ensure!(
                f.runtime.store.execute(&self.binding, read).await? == *expected,
                "historical full row changed"
            );
        }
        ensure!(
            snapshot(f).await? == self.snapshot,
            "original physical identity, credential ciphertext or record ACL changed"
        );
        let binding = PluginCredentialBinding {
            installation_id: id.to_string(),
            plugin_version: "0.1.25".into(),
            ..self.credential.clone()
        };
        let got = self
            .credentials
            .execute_plugin_credential(
                &binding,
                &PluginCredentialRequest::Get {
                    credential_id: self.credential_id.clone(),
                },
            )
            .await?;
        ensure!(
            got.value.as_ref() == Some(&self.secret),
            "credential no longer decrypts under same taichuy/ssh owner"
        );
        let service = RecordPermissionService::new(f.runtime.store.clone());
        ensure!(
            service.get(&f.actor, &self.acl_key).await? == self.acl,
            "original ACL policy changed"
        );
        service
            .authorize(&f.member, &self.acl_key, "view", true)
            .await?;
        ensure!(
            service.get(&f.actor, &self.hidden_acl_key).await? == self.hidden_acl,
            "original hidden-host ACL policy changed"
        );
        for (key, action) in [(&self.acl_key, "update"), (&self.hidden_acl_key, "view")] {
            let denied = service
                .authorize(&f.member, key, action, true)
                .await
                .err()
                .context("ungranted ACL action succeeded")?;
            ensure!(
                matches!(
                    denied.downcast_ref::<control_plane_contracts::ControlPlaneContractError>(),
                    Some(
                        control_plane_contracts::ControlPlaneContractError::PermissionDenied(
                            "record_permission_denied"
                        )
                    )
                ),
                "wrong ACL denial"
            );
        }
        Ok(())
    }
}
