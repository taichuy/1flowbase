use super::*;
fn context(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "GET".into(),
        path: "/fixture".into(),
    }
}

#[test]
fn backup_binary_reasons_are_phase_specific() {
    let import = context("system_backups.import");
    let download = context("system_backups.download");
    assert!(matches!(
        SystemBackupsInput::document_input_unavailable(&import),
        Some(ApiEventUnavailableReason::BinaryRequest)
    ));
    assert!(SystemBackupsOutput::document_output_unavailable(&import).is_none());
    assert!(SystemBackupsInput::document_input_unavailable(&download).is_none());
    assert!(matches!(
        SystemBackupsOutput::document_output_unavailable(&download),
        Some(ApiEventUnavailableReason::BinaryResponse)
    ));
    let id = Uuid::nil();
    assert_eq!(
        SystemBackupsInput::Download { backup_set_id: id }
            .project_document_input(&download)
            .unwrap(),
        json!({"path":{"backup_set_id":id}})
    );
}
#[test]
fn recovery_credentials_are_excluded_in_value_and_schema() {
    let c = context("system_backups.recovery.intent");
    let input = SystemBackupsInput::RecoveryIntent {
        backup_set_id: Uuid::nil(),
        request: CreateRecoveryIntentRequest {
            confirm_missing_plugins: true,
            challenge_token: Uuid::new_v4(),
            exact_backup_name: "actual backup".into(),
            plan_digest: "actual digest".into(),
            backup_password: Some("credential-sentinel".into()),
        },
    };
    let value = input.project_document_input(&c).unwrap();
    assert_eq!(value["body"]["exact_backup_name"], "actual backup");
    for field in CREDENTIALS {
        assert!(value["body"].get(*field).is_none());
    }
    assert!(!value.to_string().contains("credential-sentinel"));
    let schema = SystemBackupsInput::document_input_schema(&c).unwrap();
    for field in CREDENTIALS {
        assert!(schema["properties"]["body"]["properties"]
            .get(*field)
            .is_none());
    }
    let c = context("system_backups.recovery.reauth");
    let response = RecoveryReauthResponse {
        challenge_token: Uuid::new_v4(),
        expires_at: "2030-01-01T00:00:00Z".into(),
    };
    assert!(to_value(&response)
        .unwrap()
        .get("challenge_token")
        .is_some());
    assert_eq!(
        SystemBackupsOutput::RecoveryReauth(response)
            .project_document_output(&c)
            .unwrap(),
        json!({"expires_at":"2030-01-01T00:00:00Z"})
    );
    assert!(
        SystemBackupsOutput::document_output_schema(&c).unwrap()["properties"]
            .get("challenge_token")
            .is_none()
    );
}
