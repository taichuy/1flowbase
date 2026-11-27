//! Real-Postgres integration tests for Privacy Masking runtime policies, atomic lifecycle fencing,
//! workspace isolation, keyset log pagination, and zero-plaintext audit guarantees (AC-001 ~ AC-004).
use std::sync::Arc;

use control_plane::{
    audit::audit_log,
    ports::{
        CommitPluginFamilyUninstallInput, GrantContributionAuthorizationInput,
        ListPrivacyMaskingLogsQuery, PluginContributionAuthorityRepository, PluginRepository,
        PrivacyMaskingInvalidationObserver, PrivacyMaskingRepository,
        RevokeContributionAuthorizationInput, UpdatePluginDesiredStateInput,
        UpsertPluginArtifactInstanceInput, UpsertPluginInstallationInput,
    },
    privacy_masking::{
        list_builtin_rule_descriptors, PrivacyInvocationContext, PrivacyMaskingKernel,
        BUILTIN_EMAIL,
    },
};
use domain::{
    ContributionResourceScope, ExtensionCategory, ExtensionSignatureStatus,
    PluginArtifactInstanceStatus, PluginAvailabilityStatus, PluginDesiredState,
    PluginRuntimeStatus, PluginVerificationStatus,
};
use extension_contracts::provider_contract::{
    ProviderInvocationInput, ProviderMessage, ProviderMessageRole,
};
use extension_contracts::{
    PrivacyMaskingRuleDto, PrivacyMaskingRuleKind, PrivacyMaskingRuleMode,
    PrivacyMaskingSubjectBindingDto, PrivacyMaskingSubjectKind, PrivacyMaskingWorkspacePolicyDto,
    PRIVACY_MASKING_CONTRACT_ID, PRIVACY_POLICY_SYNC_PERMISSION,
};
use serde_json::json;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn setup_store_and_workspaces() -> (
    PgControlPlaneStore,
    Arc<PrivacyMaskingKernel>,
    Uuid,
    Uuid,
    Uuid,
) {
    let schema = postgres_test_support::PostgresTestSchema::create(&database_url())
        .await
        .expect("isolated schema should be available");
    let pool = schema.connect().await.expect("schema should connect");
    run_migrations(&pool)
        .await
        .expect("migrations should apply");
    let base_store = PgControlPlaneStore::new(pool);
    let kernel = Arc::new(
        PrivacyMaskingKernel::new(Arc::new(base_store.clone()), "pg-integration-master-key-v1")
            .expect("kernel init"),
    );
    let store = base_store.with_privacy_masking_invalidation_observer(
        kernel.clone() as Arc<dyn PrivacyMaskingInvalidationObserver>,
    );

    let tenant = store.upsert_root_tenant().await.expect("tenant");
    let ws_a = store
        .upsert_workspace(tenant.id, "privacy-ws-a")
        .await
        .expect("ws_a");
    let ws_b = store
        .upsert_workspace(tenant.id, "privacy-ws-b")
        .await
        .expect("ws_b");
    store
        .upsert_login_entry(&domain::LoginEntryRecord {
            id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            connection_id: domain::PASSWORD_LOCAL_CONNECTION_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: json!({}),
        })
        .await
        .expect("login entry");
    let actor = store
        .upsert_root_user(
            ws_a.id,
            "privacy-root",
            "privacy-root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Privacy",
            "Root",
        )
        .await
        .expect("root user");

    (store, kernel, actor.id, ws_a.id, ws_b.id)
}

async fn seed_active_privacy_plugin(
    store: &PgControlPlaneStore,
    actor_id: Uuid,
    version: &str,
) -> (Uuid, domain::PluginContributionAuthoritySnapshot) {
    let installation_id = Uuid::now_v7();
    let installed = PluginRepository::upsert_installation(
        store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: ExtensionCategory::RuntimeExtensions,
            organization: "1flowbase".to_string(),
            provider_code: "privacy-masking".to_string(),
            plugin_id: format!("1flowbase/privacy-masking@{version}"),
            plugin_version: version.to_string(),
            contract_version: "1flowbase.extension-bus/v1".to_string(),
            protocol: "stdio_json_worker".to_string(),
            display_name: "Privacy Masking".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "verified_official".to_string(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({
                "binding_targets": ["system"],
                "managed_service": { "scope": "system" }
            }),
            is_system_reserved: false,
            actor_user_id: actor_id,
        },
    )
    .await
    .expect("installation upsert");

    let snapshot = PluginContributionAuthorityRepository::grant_contribution_authorization(
        store,
        &GrantContributionAuthorizationInput {
            candidate_node_id: None,
            expected_installation_updated_at: installed.updated_at,
            installation_id,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            contribution_id: "privacy-masking.service".to_string(),
            point_id: plugin_framework::MANAGED_SERVICE_POINT.to_string(),
            permission: PRIVACY_POLICY_SYNC_PERMISSION.to_string(),
            resource_scope: ContributionResourceScope::System,
            permission_contract_id: PRIVACY_MASKING_CONTRACT_ID.to_string(),
            permission_contract_version: "1".to_string(),
            actor_user_id: actor_id,
            audit_log: audit_log(
                None,
                Some(actor_id),
                "plugin_contribution_authorization",
                Some(installation_id),
                "grant",
                json!({}),
            ),
        },
    )
    .await
    .expect("grant privacy.policy.sync");

    (installation_id, snapshot)
}

fn sample_policy_for_instance(
    instance_id: Uuid,
    restore_output: bool,
) -> PrivacyMaskingWorkspacePolicyDto {
    let email_desc = list_builtin_rule_descriptors()
        .into_iter()
        .find(|d| d.builtin_code == BUILTIN_EMAIL)
        .expect("email builtin exists");
    PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![PrivacyMaskingRuleDto {
            rule_id: email_desc.builtin_code.clone(),
            name: email_desc.display_name_zh,
            kind: PrivacyMaskingRuleKind::Builtin,
            builtin_code: Some(email_desc.builtin_code),
            placeholder_kind: email_desc.placeholder_kind,
            pattern: None,
            enabled: true,
            priority: 1,
        }],
        subject_bindings: vec![PrivacyMaskingSubjectBindingDto {
            binding_id: format!("bind-{instance_id}"),
            subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
            subject_id: instance_id.to_string(),
            subject_name: "Prod Model".to_string(),
            enabled: true,
            restore_output,
            rule_mode: PrivacyMaskingRuleMode::AllEnabled,
            rule_ids: vec![],
        }],
    }
}

#[tokio::test]
async fn workspace_isolation_log_pagination_and_zero_plaintext() {
    let (store, kernel, actor_id, ws_a, ws_b) = setup_store_and_workspaces().await;
    let (installation_id, _) = seed_active_privacy_plugin(&store, actor_id, "0.1.0").await;
    let instance_id = Uuid::now_v7();

    let summary = kernel
        .sync_workspace_policy(
            installation_id,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync policy for ws_a");
    assert_eq!(summary.policy_version, 1);
    assert_eq!(summary.enabled_rule_count, 1);
    assert_eq!(summary.active_subject_count, 1);

    // Workspace B must NOT see Workspace A's policy
    let ws_b_policies = store
        .list_active_privacy_masking_policies(ws_b)
        .await
        .expect("list ws_b");
    assert!(ws_b_policies.is_empty());

    // Execute two masking invocations in Workspace A and write logs
    let flow_run_1 = Uuid::now_v7();
    for (email, run_id) in [
        ("secret1@acme.org", flow_run_1),
        ("secret2@acme.org", Uuid::now_v7()),
    ] {
        let mut input = ProviderInvocationInput {
            model: "gpt-4o".into(),
            protocol: "openai_chat".into(),
            provider_instance_id: instance_id.to_string(),
            messages: vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: format!("Please email {email} immediately"),
                name: None,
                tool_call_id: None,
                is_error: None,
                tool_calls: None,
                content_blocks: None,
            }],
            ..Default::default()
        };
        let ctx = PrivacyInvocationContext {
            workspace_id: ws_a,
            flow_run_id: Some(run_id),
            application_id: None,
            provider_instance_id: instance_id,
            provider_instance_name: "Prod Model".into(),
            model_id: "gpt-4o".into(),
            protocol: "openai_chat".into(),
        };
        let prepared = kernel
            .prepare_invocation(&ctx, &mut input)
            .await
            .expect("prepare")
            .expect("matched");
        assert!(!input.messages[0].content.contains(email));
        let restored = prepared
            .restoration_stack
            .restore_str(&input.messages[0].content);
        assert!(restored.contains(email));
        kernel.finalize_invocation(&prepared, None).await;
    }

    // Keyset pagination with limit = 1
    let page1 = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_a,
            limit: 1,
            ..Default::default()
        })
        .await
        .expect("page1");
    assert_eq!(page1.items.len(), 1);
    assert!(page1.next_cursor.is_some());

    let page2 = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_a,
            limit: 10,
            cursor: page1.next_cursor,
            ..Default::default()
        })
        .await
        .expect("page2");
    assert_eq!(page2.items.len(), 1);
    assert!(page2.next_cursor.is_none());

    // Zero plaintext in every persisted log record
    for entry in page1.items.iter().chain(page2.items.iter()) {
        let raw = serde_json::to_string(entry).unwrap();
        assert!(!raw.contains("secret1@acme.org"), "leaked plaintext: {raw}");
        assert!(!raw.contains("secret2@acme.org"), "leaked plaintext: {raw}");
        assert_eq!(entry.restored_count, 1);
    }

    // Workspace B sees zero logs
    let ws_b_logs = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_b,
            limit: 10,
            ..Default::default()
        })
        .await
        .expect("ws_b logs");
    assert!(ws_b_logs.items.is_empty());

    // Filter by flow_run_id & placeholder_kind
    let filtered = store
        .list_privacy_masking_logs(&ListPrivacyMaskingLogsQuery {
            workspace_id: ws_a,
            flow_run_id: Some(flow_run_1),
            placeholder_kind: Some("EMAIL".into()),
            limit: 10,
            ..Default::default()
        })
        .await
        .expect("filtered");
    assert_eq!(filtered.items.len(), 1);
    assert_eq!(
        filtered.items[0].flow_run_id.as_deref(),
        Some(flow_run_1.to_string().as_str())
    );
}

#[tokio::test]
async fn atomic_lifecycle_fencing_purges_policies_on_disable_revoke_and_uninstall() {
    let (store, kernel, actor_id, ws_a, _ws_b) = setup_store_and_workspaces().await;
    let instance_id = Uuid::now_v7();

    // 1. Disable purges runtime policy and invalidates in-process cache immediately
    let (inst_disable, _) = seed_active_privacy_plugin(&store, actor_id, "0.1.0").await;
    kernel
        .sync_workspace_policy(
            inst_disable,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync");
    assert!(kernel.has_active_policies(ws_a).await.unwrap());

    PluginRepository::update_desired_state(
        &store,
        &UpdatePluginDesiredStateInput {
            installation_id: inst_disable,
            desired_state: PluginDesiredState::Disabled,
            actor_user_id: actor_id,
        },
    )
    .await
    .expect("disable installation");

    assert!(
        store
            .list_active_privacy_masking_policies(ws_a)
            .await
            .unwrap()
            .is_empty(),
        "disabling plugin must atomically remove active policies"
    );
    assert!(
        !kernel.has_active_policies(ws_a).await.unwrap(),
        "in-process kernel cache must be invalidated immediately on disable"
    );

    // 2. Revoking `privacy.policy.sync` purges runtime policy and invalidates cache
    let (inst_revoke, snapshot) = seed_active_privacy_plugin(&store, actor_id, "0.2.0").await;
    kernel
        .sync_workspace_policy(
            inst_revoke,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync");
    assert!(kernel.has_active_policies(ws_a).await.unwrap());

    let auth_id = snapshot
        .authorizations
        .iter()
        .find(|a| a.permission == PRIVACY_POLICY_SYNC_PERMISSION)
        .expect("grant exists")
        .id;
    PluginContributionAuthorityRepository::revoke_contribution_authorization(
        &store,
        &RevokeContributionAuthorizationInput {
            installation_id: inst_revoke,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            authorization_id: auth_id,
            expected_revision: snapshot.revision,
            actor_user_id: actor_id,
            audit_log: audit_log(
                None,
                Some(actor_id),
                "plugin_contribution_authorization",
                Some(auth_id),
                "revoke",
                json!({}),
            ),
        },
    )
    .await
    .expect("revoke");

    assert!(store
        .list_active_privacy_masking_policies(ws_a)
        .await
        .unwrap()
        .is_empty());
    assert!(!kernel.has_active_policies(ws_a).await.unwrap());

    // 3. Uninstall purges runtime policy and invalidates cache
    let (inst_uninstall, _) = seed_active_privacy_plugin(&store, actor_id, "0.3.0").await;
    kernel
        .sync_workspace_policy(
            inst_uninstall,
            ws_a,
            Some(actor_id),
            &sample_policy_for_instance(instance_id, true),
        )
        .await
        .expect("sync");
    assert!(kernel.has_active_policies(ws_a).await.unwrap());

    PluginRepository::commit_plugin_family_uninstall(
        &store,
        &CommitPluginFamilyUninstallInput {
            artifact_instances: vec![UpsertPluginArtifactInstanceInput {
                node_id: "test-node".to_string(),
                installation_id: inst_uninstall,
                local_version: None,
                local_checksum: None,
                local_path: None,
                package_path: None,
                manifest_fingerprint: None,
                artifact_status: PluginArtifactInstanceStatus::Missing,
                runtime_status: PluginRuntimeStatus::Inactive,
                availability_status: PluginAvailabilityStatus::ArtifactMissing,
                checked_at: OffsetDateTime::now_utc(),
                last_error: None,
                is_current: false,
            }],
            artifact_cleanups: vec![],
            audit_log: audit_log(
                None,
                Some(actor_id),
                "plugin_installation",
                Some(inst_uninstall),
                "uninstall",
                json!({}),
            ),
        },
    )
    .await
    .expect("commit uninstall");

    assert!(store
        .list_active_privacy_masking_policies(ws_a)
        .await
        .unwrap()
        .is_empty());
    assert!(!kernel.has_active_policies(ws_a).await.unwrap());
}
