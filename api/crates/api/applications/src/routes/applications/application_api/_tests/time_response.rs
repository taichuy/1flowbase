use super::*;
use crate::routes::_tests::time_response_cases::assert_timestamp_mapping;

fn schedule(at: OffsetDateTime) -> WorkflowScheduleTriggerRecord {
    WorkflowScheduleTriggerRecord {
        id: Uuid::nil(),
        workspace_id: Uuid::nil(),
        application_id: Uuid::nil(),
        enabled: true,
        cron: "0 * * * *".into(),
        timezone: "UTC".into(),
        input_payload: serde_json::json!({}),
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: at,
        updated_at: at,
    }
}

#[test]
fn schedule_response_timestamp_contract() {
    for field in 0..2 {
        assert_timestamp_mapping(|at| {
            let mut record = schedule(OffsetDateTime::UNIX_EPOCH);
            if field == 0 {
                record.created_at = at;
            } else {
                record.updated_at = at;
            }
            let response = to_workflow_schedule_trigger_response(record)?;
            Ok(vec![if field == 0 {
                response.created_at
            } else {
                response.updated_at
            }])
        });
    }
}

#[test]
fn optional_timestamp_preserves_absence_and_propagates_format_failure() {
    assert_eq!(format_optional_time(None).unwrap(), None);
    assert_timestamp_mapping(|at| {
        Ok(vec![
            format_optional_time(Some(at))?.expect("present timestamp")
        ])
    });
}

fn api_key() -> domain::ApiKeyRecord {
    domain::ApiKeyRecord {
        id: Uuid::nil(),
        name: "key".into(),
        token_hash: "hash".into(),
        token_prefix: "prefix".into(),
        key_kind: domain::ApiKeyKind::ApplicationApiKey,
        application_id: Some(Uuid::nil()),
        role_code: None,
        creator_user_id: Uuid::nil(),
        tenant_id: Uuid::nil(),
        scope_kind: domain::DataModelScopeKind::Workspace,
        scope_id: Uuid::nil(),
        enabled: true,
        expires_at: None,
        last_used_at: None,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[test]
fn api_key_response_timestamp_contract() {
    for created in [false, true] {
        for field in 0..4 {
            assert_timestamp_mapping(|at| {
                let mut record = api_key();
                match field {
                    0 => record.created_at = at,
                    1 => record.updated_at = at,
                    2 => record.expires_at = Some(at),
                    _ => record.last_used_at = Some(at),
                }
                let value = if created {
                    let response = to_created_api_key_response(record, "token".into())?;
                    match field {
                        0 => response.created_at,
                        1 => response.updated_at,
                        2 => response.expires_at.unwrap(),
                        _ => response.last_used_at.unwrap(),
                    }
                } else {
                    let response = to_api_key_response(record)?;
                    match field {
                        0 => response.created_at,
                        1 => response.updated_at,
                        2 => response.expires_at.unwrap(),
                        _ => response.last_used_at.unwrap(),
                    }
                };
                Ok(vec![value])
            });
        }
    }
    let response = to_api_key_response(api_key()).unwrap();
    assert_eq!(response.expires_at, None);
    assert_eq!(response.last_used_at, None);
}
