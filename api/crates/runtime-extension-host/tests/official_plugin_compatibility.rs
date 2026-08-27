use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use extension_contracts::{
    ProviderCountTokensInput, ProviderFinishReason, ProviderInvocationInput, ProviderStreamEvent,
};
use extension_package_runtime::{
    parse_plugin_manifest, PluginConsumptionKind, PluginExecutionMode,
};
use runtime_core::runtime_backend::{
    NetworkEgressRuntimePort, ProviderRuntimePort, RuntimeArtifactReference, RuntimeBackendError,
    RuntimeExecutionPort, RuntimeExecutionRequest, RuntimeNetworkEgressActivation,
    RuntimePackageActivation, RuntimeRequestId, RuntimeStreamSinks, RuntimeTargetId,
};
use runtime_extension_host::{RuntimeArtifactResolver, RuntimeExtensionHost};

const PROVIDERS: [&str; 7] = [
    "aliyun_bailian",
    "anthropic",
    "chatgpt-codex",
    "deepseek",
    "gemini",
    "openai",
    "openai_compatible",
];

struct StagedPackages(PathBuf);

impl Drop for StagedPackages {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct StagedArtifactResolver(PathBuf);

#[async_trait::async_trait]
impl RuntimeArtifactResolver for StagedArtifactResolver {
    async fn resolve(
        &self,
        artifact: &RuntimeArtifactReference,
    ) -> Result<PathBuf, RuntimeBackendError> {
        Ok(self.0.join(artifact.as_str()))
    }
}

fn official_root() -> PathBuf {
    std::env::var_os("ONEFLOWBASE_OFFICIAL_PLUGIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../..")
                .join("1flowbase-official-plugins")
        })
}

fn copy_runtime_assets(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).expect("stage official package directory");
    for entry in fs::read_dir(source).expect("read official package source") {
        let entry = entry.expect("official package source entry");
        if matches!(
            entry.file_name().to_str(),
            Some("demo" | "scripts" | "src" | "tests" | "target")
        ) {
            continue;
        }
        let destination = destination.join(entry.file_name());
        if entry.path().is_dir() {
            copy_runtime_assets(&entry.path(), &destination);
        } else if entry.path().is_file() {
            fs::copy(entry.path(), destination).expect("copy official runtime asset");
        }
    }
}

fn stage_built_packages(official_root: &Path) -> StagedPackages {
    let stage_root = std::env::temp_dir().join(format!(
        "oneflowbase-official-runtime-conformance-{}-{}",
        std::process::id(),
        time::OffsetDateTime::now_utc().unix_timestamp_nanos()
    ));
    let extension_root = official_root.join("runtime-extensions/@taichuy");
    for plugin_id in PROVIDERS.into_iter().chain(["clash-proxy"]) {
        let source = extension_root.join(plugin_id);
        let destination = stage_root.join(plugin_id);
        copy_runtime_assets(&source, &destination);
        let manifest = parse_plugin_manifest(
            &fs::read_to_string(source.join("manifest.yaml")).expect("official manifest"),
        )
        .expect("official manifest contract");
        let entry = Path::new(&manifest.runtime.entry);
        let binary_name = entry.file_name().expect("runtime entry binary name");
        let built_binary = source.join("target/debug").join(binary_name);
        assert!(
            built_binary.is_file(),
            "{} must be built before host conformance",
            built_binary.display()
        );
        let staged_binary = destination.join(entry);
        fs::create_dir_all(staged_binary.parent().expect("runtime entry parent"))
            .expect("stage runtime bin directory");
        fs::copy(&built_binary, &staged_binary).expect("stage built official executable");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&staged_binary).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&staged_binary, permissions).unwrap();
        }
    }
    StagedPackages(stage_root)
}

fn count_tokens_input(provider_code: &str) -> ProviderCountTokensInput {
    ProviderCountTokensInput::from_invocation(ProviderInvocationInput {
        provider_instance_id: format!("conformance-{provider_code}"),
        provider_code: provider_code.to_string(),
        protocol: "openai_chat".to_string(),
        model: "conformance-model".to_string(),
        ..ProviderInvocationInput::default()
    })
}

fn generate_input(provider_code: &str) -> ProviderInvocationInput {
    ProviderInvocationInput {
        provider_instance_id: format!("conformance-{provider_code}"),
        provider_code: provider_code.to_string(),
        protocol: "openai_chat".to_string(),
        model: "conformance-model".to_string(),
        ..ProviderInvocationInput::default()
    }
}

fn versioned_plugin_id(extension_root: &Path, plugin_id: &str) -> String {
    let manifest_path = extension_root.join(plugin_id).join("manifest.yaml");
    let manifest = parse_plugin_manifest(
        &fs::read_to_string(&manifest_path).expect("official manifest must be readable"),
    )
    .unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
    format!("{}@{}", manifest.plugin_id, manifest.version)
}

#[tokio::test]
#[ignore = "requires built official RuntimeExtension executables"]
async fn d_008_eight_official_runtime_extensions_execute_through_the_real_host() {
    let official_root = official_root();
    let staged = stage_built_packages(&official_root);
    let extension_root = official_root.join("runtime-extensions/@taichuy");
    let expected = BTreeSet::from([
        "aliyun_bailian".to_string(),
        "anthropic".to_string(),
        "chatgpt-codex".to_string(),
        "clash-proxy".to_string(),
        "deepseek".to_string(),
        "gemini".to_string(),
        "openai".to_string(),
        "openai_compatible".to_string(),
    ]);
    let mut actual = BTreeSet::new();
    let mut modes = BTreeSet::new();
    let mut protocols = BTreeSet::new();

    for entry in fs::read_dir(&extension_root).expect("official runtime-extension directory") {
        let entry = entry.expect("official runtime-extension entry");
        let manifest_path = entry.path().join("manifest.yaml");
        if !manifest_path.is_file() {
            continue;
        }
        let manifest = parse_plugin_manifest(&fs::read_to_string(&manifest_path).unwrap())
            .unwrap_or_else(|error| panic!("{}: {error}", manifest_path.display()));
        assert_eq!(
            manifest.consumption_kind,
            PluginConsumptionKind::RuntimeExtension
        );
        assert!(matches!(
            manifest.execution_mode,
            PluginExecutionMode::ProcessPerCall
                | PluginExecutionMode::StatefulProviderWorker
                | PluginExecutionMode::StatefulRuntimeWorker
        ));
        actual.insert(manifest.plugin_id.clone());
        modes.insert(manifest.execution_mode.as_str());
        protocols.insert(manifest.runtime.protocol.clone());
    }
    assert_eq!(actual, expected);
    assert_eq!(
        modes,
        BTreeSet::from([
            "process_per_call",
            "stateful_provider_worker",
            "stateful_runtime_worker",
        ])
    );
    assert_eq!(
        protocols,
        BTreeSet::from(["stdio_json".to_string(), "stdio_json_worker".to_string()])
    );

    let host = RuntimeExtensionHost::new_with_artifact_resolver(
        time::OffsetDateTime::now_utc(),
        Arc::new(StagedArtifactResolver(staged.0.clone())),
    )
    .unwrap();
    host.mark_ready().unwrap();
    for plugin_id in PROVIDERS {
        let runtime_plugin_id = versioned_plugin_id(&extension_root, plugin_id);
        host.activate_provider(RuntimePackageActivation {
            plugin_id: runtime_plugin_id.clone(),
            artifact: RuntimeArtifactReference::new(plugin_id).unwrap(),
            source_identity: Some(format!("official-conformance:{plugin_id}")),
            legacy_eligibility: None,
        })
        .await
        .unwrap_or_else(|error| panic!("{plugin_id} must load: {error}"));

        let validation = host
            .provider_validate(&runtime_plugin_id, serde_json::json!({}))
            .await;
        match validation {
            Ok(output) => assert!(
                output.is_object() || output.is_null(),
                "{plugin_id} config.validate must return its real result envelope"
            ),
            Err(RuntimeBackendError::Contract(error)) => assert!(
                !error.to_string().trim().is_empty(),
                "{plugin_id} config.validate must preserve its real contract error"
            ),
            Err(error) => panic!("{plugin_id} config.validate wire failed: {error}"),
        }
        let counted = host
            .provider_count_tokens(&runtime_plugin_id, count_tokens_input(plugin_id))
            .await
            .unwrap_or_else(|error| panic!("{plugin_id} CountTokens wire failed: {error}"));
        assert_eq!(
            counted.operation,
            extension_contracts::ProviderWireOperation::CountTokens
        );

        let generate = host
            .execute_stream(
                RuntimeExecutionRequest {
                    request_id: RuntimeRequestId::new(format!("generate-error-{plugin_id}"))
                        .unwrap(),
                    target: RuntimeTargetId::new(runtime_plugin_id).unwrap(),
                    input: generate_input(plugin_id),
                },
                RuntimeStreamSinks::default(),
            )
            .await;
        match generate {
            Ok(outcome) => {
                assert_eq!(
                    outcome.result.finish_reason,
                    Some(ProviderFinishReason::Error)
                );
                assert!(
                    outcome.events.iter().any(|event| matches!(
                        event,
                        ProviderStreamEvent::Error { error }
                            if !error.message.trim().is_empty()
                    )),
                    "{plugin_id} must preserve its real streaming error event"
                );
            }
            Err(RuntimeBackendError::Contract(error)) => assert!(
                !error.to_string().trim().is_empty(),
                "{plugin_id} must preserve its real terminal contract error"
            ),
            Err(error) => panic!("{plugin_id} Generate wire failed: {error}"),
        }
    }

    let clash_plugin_id = versioned_plugin_id(&extension_root, "clash-proxy");
    let clash_error = host
        .network_egress_preflight(RuntimeNetworkEgressActivation {
            runtime_id: "clash-conformance".to_string(),
            plugin_id: clash_plugin_id,
            artifact: RuntimeArtifactReference::new("clash-proxy").unwrap(),
            source_identity: "official-conformance:clash-proxy".to_string(),
            secret_json: serde_json::json!({}),
        })
        .await
        .expect_err("empty Clash secret must return a real worker error");
    assert!(!clash_error.to_string().trim().is_empty());

    host.stop().await.unwrap();
}
