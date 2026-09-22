use super::*;
use control_plane::capability_plugin_runtime::*;
use control_plane::ports::{ProviderRuntimeInvocationOutput, ProviderRuntimePort};
use control_plane_contracts::ports::{
    CreateModelProviderInstanceInput, CreatePluginAssignmentInput, ModelProviderRepository,
    PluginRepository, UpsertPluginArtifactInstanceInput, UpsertPluginInstallationInput,
};
use plugin_framework::provider_contract::{
    ProviderFinishReason, ProviderInvocationInput, ProviderInvocationResult,
    ProviderModelDescriptor,
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Mutex,
};

#[derive(Clone)]
pub(super) struct PausedProvider {
    pub entered: Arc<tokio::sync::Semaphore>,
    pub release: Arc<tokio::sync::Semaphore>,
    pub inputs: Arc<Mutex<Vec<ProviderInvocationInput>>>,
    pub executions: Arc<AtomicUsize>,
}
impl PausedProvider {
    pub fn new() -> Self {
        Self {
            entered: Arc::new(tokio::sync::Semaphore::new(0)),
            release: Arc::new(tokio::sync::Semaphore::new(0)),
            inputs: Arc::new(Mutex::new(vec![])),
            executions: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub async fn wait_until_entered(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(20), self.entered.acquire())
            .await
            .expect("real consumer must reach provider execution port")
            .unwrap()
            .forget();
    }
}
#[async_trait::async_trait]
impl ProviderRuntimePort for PausedProvider {
    async fn ensure_loaded(&self, _: &domain::LocalPluginInstallationRecord) -> anyhow::Result<()> {
        Ok(())
    }
    async fn validate_provider(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: Value,
    ) -> anyhow::Result<Value> {
        Ok(json!({}))
    }
    async fn list_models(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        _: Value,
    ) -> anyhow::Result<Vec<ProviderModelDescriptor>> {
        Ok(vec![])
    }
    async fn invoke_stream(
        &self,
        _: &domain::LocalPluginInstallationRecord,
        input: ProviderInvocationInput,
    ) -> anyhow::Result<ProviderRuntimeInvocationOutput> {
        self.inputs.lock().unwrap().push(input);
        self.entered.add_permits(1);
        // Stop at the external execution boundary, after the real consumer has
        // committed its receipt and claim and reconstructed checkpoint context.
        self.release.acquire().await.unwrap().forget();
        self.executions.fetch_add(1, Ordering::SeqCst);
        Ok(ProviderRuntimeInvocationOutput {
            events: vec![],
            result: ProviderInvocationResult {
                final_content: Some("recovered".into()),
                finish_reason: Some(ProviderFinishReason::Stop),
                ..Default::default()
            },
        })
    }
}
#[async_trait::async_trait]
impl CapabilityPluginRuntimePort for PausedProvider {
    async fn validate_config(&self, _: ValidateCapabilityConfigInput) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn resolve_dynamic_options(
        &self,
        _: ResolveCapabilityOptionsInput,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn resolve_output_schema(
        &self,
        _: ResolveCapabilityOutputSchemaInput,
    ) -> anyhow::Result<Value> {
        anyhow::bail!("unexpected capability")
    }
    async fn execute_node(
        &self,
        _: ExecuteCapabilityNodeInput,
    ) -> anyhow::Result<CapabilityExecutionOutput> {
        anyhow::bail!("unexpected capability")
    }
}

pub(super) struct ProviderPackage(std::path::PathBuf);
impl Drop for ProviderPackage {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) async fn seed_runtime_consumer(
    store: &PgControlPlaneStore,
    seeded: &RuntimeSeedState,
    compiled: &domain::CompiledPlanRecord,
    checkpoint: &domain::CheckpointRecord,
) -> (
    control_plane::orchestration_runtime::OrchestrationRuntimeService<
        PgControlPlaneStore,
        PausedProvider,
    >,
    PausedProvider,
    ProviderPackage,
) {
    // Authorize the fixture owner through the real actor repository.
    let role = Uuid::now_v7();
    sqlx::query("insert into roles (id, scope_kind, code, name) values ($1, 'system', 'root', 'Root') on conflict do nothing").bind(role).execute(store.pool()).await.unwrap();
    sqlx::query("insert into user_role_bindings (id, user_id, role_id) select $1, $2, id from roles where code='root' and scope_kind='system'").bind(Uuid::now_v7()).bind(seeded.actor_user_id).execute(store.pool()).await.unwrap();
    let root = ProviderPackage(
        std::env::temp_dir().join(format!("semantic-resume-provider-{}", Uuid::now_v7())),
    );
    write_provider_package(&root.0);
    let installation_id = Uuid::now_v7();
    store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id,
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "1flowbase-tests".into(),
            provider_code: "fixture_provider".into(),
            plugin_id: "fixture_provider@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.provider/v2".into(),
            protocol: "stdio_json".into(),
            display_name: "Fixture".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: seeded.actor_user_id,
        })
        .await
        .unwrap();
    store
        .upsert_artifact_instance(&UpsertPluginArtifactInstanceInput {
            node_id: "semantic-fixture".into(),
            installation_id,
            local_version: Some("0.1.0".into()),
            local_checksum: None,
            local_path: Some(root.0.to_string_lossy().into_owned()),
            package_path: None,
            manifest_fingerprint: None,
            artifact_status: domain::PluginArtifactInstanceStatus::Ready,
            runtime_status: domain::PluginRuntimeStatus::Active,
            availability_status: domain::PluginAvailabilityStatus::Available,
            checked_at: OffsetDateTime::now_utc(),
            last_error: None,
            is_current: true,
        })
        .await
        .unwrap();
    store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id,
            workspace_id: seeded.workspace_id,
            provider_code: "fixture_provider".into(),
            actor_user_id: seeded.actor_user_id,
        })
        .await
        .unwrap();
    let instance_id = Uuid::now_v7();
    ModelProviderRepository::create_instance(
        store,
        &CreateModelProviderInstanceInput {
            instance_id,
            workspace_id: seeded.workspace_id,
            installation_id,
            provider_code: "fixture_provider".into(),
            protocol: "openai_compatible".into(),
            display_name: "Fixture".into(),
            status: domain::ModelProviderInstanceStatus::Ready,
            config_json: json!({"base_url":"https://example.invalid"}),
            configured_models: vec![],
            enabled_model_ids: vec!["gpt-5.4-mini".into()],
            included_in_main: Some(true),
            created_by: seeded.actor_user_id,
        },
    )
    .await
    .unwrap();
    let plan = json!({"flow_id":seeded.flow_id,"source_draft_id":seeded.draft_id.to_string(),"schema_version":"1flowbase.flow/v2","topological_order":["node-llm"],"nodes":{
        "node-llm":{"node_id":"node-llm","node_type":"llm","alias":"LLM","container_id":null,"dependency_node_ids":[],"downstream_node_ids":[],"bindings":{},"outputs":[{"key":"text","title":"Text","value_type":"string","selector":[]}],"config":{"model":"gpt-5.4-mini","provider_instance_id":instance_id},"llm_runtime":{"provider_instance_id":instance_id,"provider_code":"fixture_provider","protocol":"openai_compatible","model":"gpt-5.4-mini"}}
    }});
    sqlx::query("update flow_compiled_plans set plan=$2 where id=$1")
        .bind(compiled.id)
        .bind(plan)
        .execute(store.pool())
        .await
        .unwrap();
    let snapshot = json!({"node-llm":{"__llm_tool_callback":{"callback_kind":"llm_tool_calls","pending_tool_calls":[{"id":"call-1-0","name":"Bash","arguments":{"command":"step-1-0"}}],"system":[],"history":[{"role":"user","content":"original"},{"role":"assistant","content":"","tool_calls":[{"id":"call-1-0","name":"Bash","arguments":{"command":"step-1-0"}}]}]}}});
    sqlx::query(
        "update flow_run_checkpoints set locator_payload=$2, variable_snapshot=$3 where id=$1",
    )
    .bind(checkpoint.id)
    .bind(json!({"node_id":"node-llm","next_node_index":0,"active_node_ids":["node-llm"]}))
    .bind(snapshot)
    .execute(store.pool())
    .await
    .unwrap();
    let provider = PausedProvider::new();
    let runtime = control_plane::orchestration_runtime::OrchestrationRuntimeService::new(
        store.clone(),
        provider.clone(),
        Arc::new(runtime_core::runtime_engine::RuntimeEngine::for_tests()),
        "fixture-key",
        Arc::new(storage_ephemeral::MemoryProviderTransportStore::new(
            Duration::minutes(5),
            1024 * 1024,
        )),
        false,
    )
    .with_node_artifact_context("semantic-fixture", root.0.clone());
    (runtime, provider, root)
}

fn write_provider_package(root: &std::path::Path) {
    for dir in ["provider", "bin", "models/llm", "i18n"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    std::fs::write(
        root.join("manifest.yaml"),
        r#"manifest_version: 1
plugin_id: fixture_provider@0.1.0
version: 0.1.0
publisher_namespace: 1flowbase-tests
vendor: 1flowbase tests
display_name: Fixture Provider
description: Fixture Provider
icon: icon.svg
source_kind: official_registry
trust_level: verified_official
consumption_kind: runtime_extension
execution_mode: process_per_call
slot_codes:
  - model_provider
binding_targets:
  - workspace
selection_mode: assignment_then_select
minimum_host_version: 0.1.0
contract_version: 1flowbase.provider/v2
schema_version: 1flowbase.plugin.manifest/v1
permissions:
  network: outbound_only
  secrets: provider_instance_only
  storage: none
  mcp: none
  subprocess: deny
runtime:
  protocol: stdio_json
  entry: bin/fixture_provider-provider
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("provider/fixture_provider.yaml"),
        r#"provider_code: fixture_provider
display_name: Fixture Provider
protocol: openai_compatible
help_url: https://example.com/help
default_base_url: https://api.example.com
model_discovery: hybrid
supports_model_fetch_without_credentials: true
config_schema:
  - key: base_url
    type: string
    required: true
  - key: api_key
    type: secret
    required: false
  - key: validate_model
    type: boolean
    required: false
"#,
    )
    .unwrap();
    std::fs::write(
        root.join("bin/fixture_provider-provider"),
        "provider-double-only",
    )
    .unwrap();
    std::fs::write(
        root.join("models/llm/_position.yaml"),
        "items:\n  - fixture_chat\n",
    )
    .unwrap();
    std::fs::write(root.join("models/llm/fixture_chat.yaml"), "model: gpt-5.4-mini\nlabel: Fixture\nfamily: llm\ncapabilities: [stream, tool_call]\ncontext_window: 128000\nmax_output_tokens: 4096\n").unwrap();
    std::fs::write(root.join("i18n/en_US.json"), "{}").unwrap();
}
