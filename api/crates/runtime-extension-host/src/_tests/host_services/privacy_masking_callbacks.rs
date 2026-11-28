use super::*;
use extension_contracts::*;
use serde_json::json;
use std::sync::Mutex;

struct MockPrivacyPort(Mutex<Vec<PrivacyMaskingBinding>>);

impl PrivacyMaskingPort for MockPrivacyPort {
    fn execute<'a>(
        &'a self,
        binding: &'a PrivacyMaskingBinding,
        request: &'a PrivacyMaskingRequest,
    ) -> PrivacyMaskingFuture<'a> {
        Box::pin(async move {
            self.0.lock().unwrap().push(binding.clone());
            match request {
                PrivacyMaskingRequest::InspectCallerContext => {
                    Ok(PrivacyMaskingResponse::CallerContext {
                        caller: binding.caller.clone(),
                    })
                }
                PrivacyMaskingRequest::ListBuiltinRules => {
                    Ok(PrivacyMaskingResponse::BuiltinRules { items: vec![] })
                }
                _ => Ok(PrivacyMaskingResponse::PolicySynced {
                    policy_version: 3,
                    enabled_rule_count: 5,
                    active_subject_count: 2,
                }),
            }
        })
    }
}

#[tokio::test]
async fn privacy_masking_callback_enforces_host_binding_and_rejects_forged_fields() {
    let mut context = super::credential_tests::context(Arc::new(
        super::credential_tests::Credentials(Mutex::new(vec![])),
    ));
    let port = Arc::new(MockPrivacyPort(Mutex::new(vec![])));
    context.privacy_masking = Some((
        PrivacyMaskingBinding {
            subject: ManagedContributionSubject::new(
                ManagedInstallationId::new("installation").unwrap(),
                ManagedWorkspaceId::new("system").unwrap(),
                ContributionId::new("privacy-masking.service").unwrap(),
            ),
            publisher_namespace: "1flowbase".into(),
            plugin_code: "privacy-masking".into(),
            plugin_version: "0.1.0".into(),
            caller: Some(PrivacyMaskingCallerDto {
                user_id: "user-1".into(),
                tenant_id: "tenant-1".into(),
                current_workspace_id: "ws-1".into(),
                is_root: false,
                effective_display_role: "admin".into(),
            }),
            can_read_subjects: true,
            can_sync_policy: true,
            can_read_logs: true,
            deadline_unix_ms: now_unix_ms() + 10_000,
        },
        port.clone(),
    ));

    let denied = execute_callback(
        MultiplexHostService::PrivacyMaskingV1,
        json!({"operation": "list_builtin_rules"}),
        None,
    )
    .await;
    assert_eq!(denied["error"]["code"], "host_service_not_granted");

    // Forged worker-supplied caller or permission field must be rejected by deny_unknown_fields.
    let forged = execute_callback(
        MultiplexHostService::PrivacyMaskingV1,
        json!({"operation": "inspect_caller_context", "caller": {"is_root": true}}),
        Some(context.clone()),
    )
    .await;
    assert_eq!(forged["error"]["code"], "host_service_request_invalid");

    // Invalid log limit fails validation before reaching port.
    let bad_limit = execute_callback(
        MultiplexHostService::PrivacyMaskingV1,
        json!({"operation": "list_logs", "limit": 0}),
        Some(context.clone()),
    )
    .await;
    assert_eq!(bad_limit["error"]["code"], "privacy_logs_limit_invalid");

    // Expired deadline is rejected before reaching port.
    let mut expired = context.clone();
    expired.privacy_masking.as_mut().unwrap().0.deadline_unix_ms = 0;
    let deadline_err = execute_callback(
        MultiplexHostService::PrivacyMaskingV1,
        json!({"operation": "list_builtin_rules"}),
        Some(expired),
    )
    .await;
    assert_eq!(
        deadline_err["error"]["code"],
        "host_service_deadline_exceeded"
    );

    // Valid call receives only host-bound caller identity.
    let ok = execute_callback(
        MultiplexHostService::PrivacyMaskingV1,
        json!({"operation": "inspect_caller_context"}),
        Some(context),
    )
    .await;
    assert_eq!(ok["result"]["caller"]["user_id"], "user-1");
    assert_eq!(ok["result"]["caller"]["current_workspace_id"], "ws-1");
    assert_eq!(ok["result"]["caller"]["is_root"], false);

    let seen = port.0.lock().unwrap();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].publisher_namespace, "1flowbase");
}
