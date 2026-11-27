//! Comprehensive unit tests for Privacy Masking built-in rules, regex engine, sequential chaining,
//! chunk-boundary streaming restoration, native_transport digest integrity, and zero-plaintext logs.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use control_plane_contracts::ports::{
    AppendPrivacyMaskingLogInput, ListPrivacyMaskingLogsQuery, PrivacyMaskingInvalidationObserver,
    PrivacyMaskingLogsQueryPage, PrivacyMaskingRepository, PrivacyMaskingRuntimePolicyRecord,
    UpsertPrivacyMaskingRuntimePolicyInput,
};
use extension_contracts::provider_contract::{
    NativePromptBlock, ProviderFinishReason, ProviderInvocationInput, ProviderInvocationResult,
    ProviderMessage, ProviderMessageRole, ProviderNativeTransport, ProviderOutputItemPhase,
    ProviderStreamEvent, ProviderToolCall,
};
use extension_contracts::{
    PrivacyMaskingRuleDto, PrivacyMaskingRuleKind, PrivacyMaskingRuleMode,
    PrivacyMaskingSubjectBindingDto, PrivacyMaskingSubjectKind, PrivacyMaskingWorkspacePolicyDto,
};
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use super::*;

#[derive(Default)]
struct InMemoryPrivacyRepo {
    policies: Mutex<HashMap<(Uuid, Uuid), PrivacyMaskingRuntimePolicyRecord>>,
    logs: Mutex<Vec<AppendPrivacyMaskingLogInput>>,
    fail_reads: Mutex<bool>,
}

#[async_trait]
impl PrivacyMaskingRepository for InMemoryPrivacyRepo {
    async fn upsert_privacy_masking_policy(
        &self,
        input: &UpsertPrivacyMaskingRuntimePolicyInput,
    ) -> anyhow::Result<PrivacyMaskingRuntimePolicyRecord> {
        let mut policies = self.policies.lock().unwrap();
        let next_version = policies
            .get(&(input.installation_id, input.workspace_id))
            .map(|r| r.policy_version + 1)
            .unwrap_or(1);
        let record = PrivacyMaskingRuntimePolicyRecord {
            installation_id: input.installation_id,
            workspace_id: input.workspace_id,
            policy_version: next_version,
            policy: input.policy.clone(),
            updated_by: input.updated_by,
            updated_at: OffsetDateTime::now_utc(),
        };
        policies.insert((input.installation_id, input.workspace_id), record.clone());
        Ok(record)
    }

    async fn list_active_privacy_masking_policies(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<PrivacyMaskingRuntimePolicyRecord>> {
        if *self.fail_reads.lock().unwrap() {
            anyhow::bail!("simulated_privacy_db_outage");
        }
        let policies = self.policies.lock().unwrap();
        let mut out: Vec<_> = policies
            .values()
            .filter(|r| r.workspace_id == workspace_id)
            .cloned()
            .collect();
        out.sort_by_key(|r| r.installation_id);
        Ok(out)
    }

    async fn delete_privacy_masking_policies_for_installation(
        &self,
        installation_id: Uuid,
    ) -> anyhow::Result<u64> {
        let mut policies = self.policies.lock().unwrap();
        let before = policies.len();
        policies.retain(|(inst, _), _| *inst != installation_id);
        Ok((before - policies.len()) as u64)
    }

    async fn append_privacy_masking_log(
        &self,
        input: &AppendPrivacyMaskingLogInput,
    ) -> anyhow::Result<()> {
        self.logs.lock().unwrap().push(input.clone());
        Ok(())
    }

    async fn list_privacy_masking_logs(
        &self,
        _query: &ListPrivacyMaskingLogsQuery,
    ) -> anyhow::Result<PrivacyMaskingLogsQueryPage> {
        Ok(PrivacyMaskingLogsQueryPage {
            items: Vec::new(),
            next_cursor: None,
        })
    }
}

fn all_builtin_rules(enable_public_ip: bool) -> Vec<PrivacyMaskingRuleDto> {
    list_builtin_rule_descriptors()
        .into_iter()
        .enumerate()
        .map(|(idx, d)| PrivacyMaskingRuleDto {
            rule_id: d.builtin_code.clone(),
            name: d.display_name_zh,
            kind: PrivacyMaskingRuleKind::Builtin,
            builtin_code: Some(d.builtin_code.clone()),
            placeholder_kind: d.placeholder_kind,
            pattern: None,
            enabled: d.default_enabled || (enable_public_ip && d.builtin_code == BUILTIN_IPV4_PUBLIC),
            priority: idx as i32,
        })
        .collect()
}

fn test_kernel() -> (Arc<InMemoryPrivacyRepo>, PrivacyMaskingKernel) {
    let repo = Arc::new(InMemoryPrivacyRepo::default());
    let kernel = PrivacyMaskingKernel::new(repo.clone(), "test-master-key-for-privacy-hmac-v1")
        .expect("kernel init");
    (repo, kernel)
}

#[tokio::test]
async fn builtin_rules_mask_real_secrets_and_resist_false_positives() {
    let (_repo, kernel) = test_kernel();
    let workspace_id = Uuid::now_v7();
    let policy = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec!["sk-allowlisted-demo-token-1234567890".into()],
        rules: all_builtin_rules(true),
        subject_bindings: vec![],
    };

    let sample = concat!(
        "OpenAI key: sk-proj-AbCdEf1234567890GhIjKlMnOpQrStUv, ",
        "Allowlisted: sk-allowlisted-demo-token-1234567890, ",
        "DB: postgres://admin:Sup3rSecr3tPwd!@db.prod.local:5432/app, ",
        "Email: alice.wang@acme-corp.io, ",
        "ExampleEmail: test@example.com, ",
        "Phone: 13812345678, ",
        "OrderId: order_id=13812345678, ",
        "CNID: 11010519491231002X, ",
        "BadCNID: 110105194912310021, ",
        "Card: 4532-0151-1283-0366, ",
        "BadCard: 4532-0151-1283-0367, ",
        "CJK: 我的密码是 MyPass9988!，",
        "PublicIP: 8.8.8.8, PrivateIP: 192.168.1.10"
    );

    let out = kernel
        .preview_masking(workspace_id, sample, Some(&policy), true)
        .await
        .expect("preview succeeds");

    // Secrets & real PII MUST be masked
    assert!(!out.masked_text.contains("sk-proj-AbCdEf1234567890GhIjKlMnOpQrStUv"));
    assert!(!out.masked_text.contains("Sup3rSecr3tPwd!"));
    assert!(!out.masked_text.contains("alice.wang@acme-corp.io"));
    assert!(!out.masked_text.contains("Phone: 13812345678"));
    assert!(!out.masked_text.contains("11010519491231002X"));
    assert!(!out.masked_text.contains("4532-0151-1283-0366"));
    assert!(!out.masked_text.contains("MyPass9988!"));
    assert!(!out.masked_text.contains("8.8.8.8"));

    // False-positive guards & allowlist MUST stay untouched
    assert!(out.masked_text.contains("sk-allowlisted-demo-token-1234567890"));
    assert!(out.masked_text.contains("test@example.com"));
    assert!(out.masked_text.contains("order_id=13812345678"));
    assert!(out.masked_text.contains("110105194912310021"));
    assert!(out.masked_text.contains("4532-0151-1283-0367"));
    assert!(out.masked_text.contains("192.168.1.10"));
    // URL scheme/user/host preserved, only password replaced
    assert!(out.masked_text.contains("postgres://admin:{{PASSWORD_"));
    assert!(out.masked_text.contains("@db.prod.local:5432/app"));

    // Round-trip restoration must reproduce exact original input
    assert_eq!(out.restored_text.as_deref(), Some(sample));
}

#[test]
fn custom_regex_validation_rejects_empty_match_and_invalid_syntax() {
    let bad_syntax = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![PrivacyMaskingRuleDto {
            rule_id: "custom-bad".into(),
            name: "Bad".into(),
            kind: PrivacyMaskingRuleKind::CustomRegex,
            builtin_code: None,
            placeholder_kind: "CUSTOM".into(),
            pattern: Some("[unclosed".into()),
            enabled: true,
            priority: 1,
        }],
        subject_bindings: vec![],
    };
    let err = CompiledWorkspacePolicy::compile(None, 1, &bad_syntax)
        .err()
        .expect("invalid regex must fail");
    assert_eq!(err.code(), "privacy_custom_regex_invalid");

    let empty_match = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![PrivacyMaskingRuleDto {
            rule_id: "custom-empty".into(),
            name: "Empty".into(),
            kind: PrivacyMaskingRuleKind::CustomRegex,
            builtin_code: None,
            placeholder_kind: "CUSTOM".into(),
            pattern: Some("a*".into()),
            enabled: true,
            priority: 1,
        }],
        subject_bindings: vec![],
    };
    let err = CompiledWorkspacePolicy::compile(None, 1, &empty_match)
        .err()
        .expect("empty-matching regex must fail");
    assert_eq!(err.code(), "privacy_custom_regex_matches_empty");
}

#[tokio::test]
async fn deterministic_placeholder_consistency_and_idempotency() {
    let (_repo, kernel) = test_kernel();
    let workspace_id = Uuid::now_v7();
    let policy = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: all_builtin_rules(false),
        subject_bindings: vec![],
    };
    let text = "First alice@acme.io and again alice@acme.io";
    let first = kernel
        .preview_masking(workspace_id, text, Some(&policy), true)
        .await
        .unwrap();
    let second = kernel
        .preview_masking(workspace_id, &first.masked_text, Some(&policy), true)
        .await
        .unwrap();

    // Both occurrences get the exact same placeholder
    assert_eq!(first.hits.len(), 1);
    assert_eq!(first.hits[0].hit_count, 2);
    // Re-running masking on already-masked text must be a no-op (idempotent)
    assert_eq!(second.total_hits, 0);
    assert_eq!(second.masked_text, first.masked_text);
}

#[tokio::test]
async fn sequential_chaining_dual_subjects_and_lifo_streaming_restoration() {
    let (repo, kernel) = test_kernel();
    let workspace_id = Uuid::now_v7();
    let installation_id = Uuid::now_v7();
    let app_id = Uuid::now_v7();
    let instance_id = Uuid::now_v7();

    let policy = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: vec![
            PrivacyMaskingRuleDto {
                rule_id: "r-email".into(),
                name: "Email".into(),
                kind: PrivacyMaskingRuleKind::Builtin,
                builtin_code: Some(BUILTIN_EMAIL.into()),
                placeholder_kind: "EMAIL".into(),
                pattern: None,
                enabled: true,
                priority: 10,
            },
            PrivacyMaskingRuleDto {
                rule_id: "r-emp".into(),
                name: "Employee Code".into(),
                kind: PrivacyMaskingRuleKind::CustomRegex,
                builtin_code: None,
                placeholder_kind: "EMP_ID".into(),
                pattern: Some(r"EMP-\d{6}".into()),
                enabled: true,
                priority: 20,
            },
            // Hypothetical stage-2 rule that matches around an already-created stage-1 placeholder
            PrivacyMaskingRuleDto {
                rule_id: "r-wrap".into(),
                name: "Tag Wrapper".into(),
                kind: PrivacyMaskingRuleKind::CustomRegex,
                builtin_code: None,
                placeholder_kind: "WRAPPED".into(),
                pattern: Some(r"USER<[^>]+>".into()),
                enabled: true,
                priority: 30,
            },
        ],
        subject_bindings: vec![
            PrivacyMaskingSubjectBindingDto {
                binding_id: "b-agent".into(),
                subject_kind: PrivacyMaskingSubjectKind::AgentFlow,
                subject_id: app_id.to_string(),
                subject_name: "Support Bot".into(),
                enabled: true,
                restore_output: true,
                rule_mode: PrivacyMaskingRuleMode::SelectedRules,
                rule_ids: vec!["r-email".into()],
            },
            PrivacyMaskingSubjectBindingDto {
                binding_id: "b-model".into(),
                subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
                subject_id: instance_id.to_string(),
                subject_name: "OpenAI Main".into(),
                enabled: true,
                restore_output: true,
                rule_mode: PrivacyMaskingRuleMode::SelectedRules,
                rule_ids: vec!["r-emp".into(), "r-wrap".into()],
            },
        ],
    };

    kernel
        .sync_workspace_policy(installation_id, workspace_id, None, &policy)
        .await
        .expect("sync ok");

    let raw_wire = json!({
        "model": "gpt-4o",
        "input": [{"role": "user", "content": "Contact USER<bob@corp.io> and EMP-123456"}]
    });
    let mut input = ProviderInvocationInput {
        model: "gpt-4o".into(),
        protocol: "openai_responses".into(),
        provider_instance_id: instance_id.to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "Contact USER<bob@corp.io> and EMP-123456".into(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        system: vec![NativePromptBlock::text("Support USER<bob@corp.io>")],
        native_transport: Some(ProviderNativeTransport {
            protocol: "openai_responses".into(),
            wire_body: raw_wire,
            digest: "sha256:stale".into(),
            size_bytes: 0,
        }),
        ..Default::default()
    };

    let ctx = PrivacyInvocationContext {
        workspace_id,
        flow_run_id: Some(Uuid::now_v7()),
        application_id: Some(app_id),
        provider_instance_id: instance_id,
        provider_instance_name: "OpenAI Main".into(),
        model_id: "gpt-4o".into(),
        protocol: "openai_responses".into(),
    };

    let prepared = kernel
        .prepare_invocation(&ctx, &mut input)
        .await
        .expect("prepare ok")
        .expect("both subjects matched");

    assert_eq!(prepared.stages.len(), 2);
    assert!(prepared.all_stages_restore);
    assert!(prepared.persisted_input_override.is_none());

    // Upstream wire input must contain ZERO plaintext
    assert!(!input.messages[0].content.contains("bob@corp.io"));
    assert!(!input.messages[0].content.contains("EMP-123456"));
    // native_transport digest must be recomputed
    let transport = input.native_transport.as_ref().unwrap();
    assert_ne!(transport.digest, "sha256:stale");
    assert!(transport.size_bytes > 0);
    assert!(!transport.wire_body.to_string().contains("bob@corp.io"));

    // Extract the outer placeholders sent upstream
    let upstream_prompt = input.messages[0].content.clone();
    // Simulate upstream model echoing the masked prompt 1 character at a time across chunks!
    let mut restorer = prepared.new_stream_restorer();
    let mut reconstructed = String::new();
    for ch in upstream_prompt.chars() {
        for ev in restorer.process_event(ProviderStreamEvent::TextDelta {
            delta: ch.to_string(),
        }) {
            if let ProviderStreamEvent::TextDelta { delta } = ev {
                reconstructed.push_str(&delta);
            }
        }
    }
    // Also test ToolCallCommit & OutputItem restoration
    let tool_ev = restorer.process_event(ProviderStreamEvent::ToolCallCommit {
        call: ProviderToolCall {
            id: "call_1".into(),
            name: "send_mail".into(),
            arguments: json!({"target": upstream_prompt}),
            provider_metadata: json!({}),
        },
    });
    let output_item_ev = restorer.process_event(ProviderStreamEvent::OutputItem {
        phase: ProviderOutputItemPhase::Done,
        output_index: 0,
        item: json!({"type": "message", "text": upstream_prompt}),
    });
    for ev in restorer.process_event(ProviderStreamEvent::Finish {
        reason: ProviderFinishReason::Stop,
    }) {
        if let ProviderStreamEvent::TextDelta { delta } = ev {
            reconstructed.push_str(&delta);
        }
    }

    // LIFO restoration unwinds BOTH Stage 2 (WRAPPED + EMP_ID) and Stage 1 (EMAIL) losslessly!
    assert_eq!(reconstructed, "Contact USER<bob@corp.io> and EMP-123456");
    assert!(tool_ev.iter().any(|ev| match ev {
        ProviderStreamEvent::ToolCallCommit { call } => {
            call.arguments["target"] == "Contact USER<bob@corp.io> and EMP-123456"
        }
        _ => false,
    }));
    assert!(output_item_ev.iter().any(|ev| match ev {
        ProviderStreamEvent::OutputItem { item, .. } => {
            item["text"] == "Contact USER<bob@corp.io> and EMP-123456"
        }
        _ => false,
    }));

    // Finalize and verify zero plaintext in audit logs (both stages logged separately)
    kernel.finalize_invocation(&prepared, Some("resp_1")).await;
    let logs = repo.logs.lock().unwrap();
    assert_eq!(logs.len(), 2);
    for log in logs.iter() {
        let serialized = serde_json::to_string(&log.matched_rules).unwrap();
        assert!(
            !serialized.contains("bob@corp.io") && !serialized.contains("EMP-123456"),
            "audit log must never contain plaintext: {serialized}"
        );
        assert!(log.restored_count > 0);
    }
}

#[tokio::test]
async fn restore_disabled_keeps_masked_output_and_persists_masked_input() {
    let (_repo, kernel) = test_kernel();
    let workspace_id = Uuid::now_v7();
    let installation_id = Uuid::now_v7();
    let instance_id = Uuid::now_v7();

    let policy = PrivacyMaskingWorkspacePolicyDto {
        enabled: true,
        allowlist: vec![],
        rules: all_builtin_rules(false),
        subject_bindings: vec![PrivacyMaskingSubjectBindingDto {
            binding_id: "b-no-restore".into(),
            subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
            subject_id: instance_id.to_string(),
            subject_name: "Strict Model".into(),
            enabled: true,
            restore_output: false,
            rule_mode: PrivacyMaskingRuleMode::AllEnabled,
            rule_ids: vec![],
        }],
    };
    kernel
        .sync_workspace_policy(installation_id, workspace_id, None, &policy)
        .await
        .unwrap();

    let mut input = ProviderInvocationInput {
        model: "gpt-4o".into(),
        protocol: "openai_chat".into(),
        provider_instance_id: instance_id.to_string(),
        messages: vec![ProviderMessage {
            role: ProviderMessageRole::User,
            content: "My email is strict@company.com".into(),
            name: None,
            tool_call_id: None,
            is_error: None,
            tool_calls: None,
            content_blocks: None,
        }],
        ..Default::default()
    };
    let ctx = PrivacyInvocationContext {
        workspace_id,
        flow_run_id: None,
        application_id: None,
        provider_instance_id: instance_id,
        provider_instance_name: "Strict Model".into(),
        model_id: "gpt-4o".into(),
        protocol: "openai_chat".into(),
    };

    let prepared = kernel
        .prepare_invocation(&ctx, &mut input)
        .await
        .unwrap()
        .unwrap();

    assert!(!prepared.all_stages_restore);
    assert!(!prepared.should_restore_output());
    let persisted = prepared
        .persisted_input_override
        .as_ref()
        .expect("persisted input override must be masked when restore_output=false");
    assert!(!persisted.messages[0].content.contains("strict@company.com"));
    assert!(persisted.messages[0].content.contains("{{EMAIL_"));

    let mut result = ProviderInvocationResult {
        final_content: Some(input.messages[0].content.clone()),
        ..Default::default()
    };
    let mut events = vec![ProviderStreamEvent::TextDelta {
        delta: input.messages[0].content.clone(),
    }];
    prepared.restore_completed_output(&mut events, &mut result);
    // Output must remain masked
    assert!(!result.final_content.as_deref().unwrap().contains("strict@company.com"));
}

#[tokio::test]
async fn fail_closed_on_repository_error_and_invalidation_clears_cache() {
    let (repo, kernel) = test_kernel();
    let workspace_id = Uuid::now_v7();
    let installation_id = Uuid::now_v7();
    let instance_id = Uuid::now_v7();

    *repo.fail_reads.lock().unwrap() = true;
    let mut input = ProviderInvocationInput::default();
    let ctx = PrivacyInvocationContext {
        workspace_id,
        flow_run_id: None,
        application_id: None,
        provider_instance_id: instance_id,
        provider_instance_name: "Model".into(),
        model_id: "m".into(),
        protocol: "openai_chat".into(),
    };
    assert!(
        kernel.prepare_invocation(&ctx, &mut input).await.is_err(),
        "must fail-closed when policy repository is unavailable"
    );

    *repo.fail_reads.lock().unwrap() = false;
    kernel
        .sync_workspace_policy(
            installation_id,
            workspace_id,
            None,
            &PrivacyMaskingWorkspacePolicyDto {
                enabled: true,
                allowlist: vec![],
                rules: all_builtin_rules(false),
                subject_bindings: vec![PrivacyMaskingSubjectBindingDto {
                    binding_id: "b1".into(),
                    subject_kind: PrivacyMaskingSubjectKind::ModelProviderInstance,
                    subject_id: instance_id.to_string(),
                    subject_name: "Model".into(),
                    enabled: true,
                    restore_output: true,
                    rule_mode: PrivacyMaskingRuleMode::AllEnabled,
                    rule_ids: vec![],
                }],
            },
        )
        .await
        .unwrap();

    // Warm cache
    assert!(kernel.prepare_invocation(&ctx, &mut input).await.unwrap().is_some());
    // Delete policy and invalidate installation -> immediately stops masking
    repo.delete_privacy_masking_policies_for_installation(installation_id)
        .await
        .unwrap();
    kernel.invalidate_installation(installation_id);
    assert!(kernel.prepare_invocation(&ctx, &mut input).await.unwrap().is_none());
}
