use super::*;
use crate::routes::_tests::time_response_cases::assert_timestamp_mapping;

#[test]
fn plugin_task_response_timestamp_contract() {
    for field in 0..3 {
        assert_timestamp_mapping(|at| {
            let valid = time::OffsetDateTime::UNIX_EPOCH;
            let response = to_task_response(domain::PluginTaskRecord {
                id: Uuid::nil(),
                installation_id: None,
                workspace_id: None,
                provider_code: "provider".into(),
                task_kind: domain::PluginTaskKind::Install,
                status: domain::PluginTaskStatus::Succeeded,
                status_message: None,
                detail_json: serde_json::json!({}),
                created_by: None,
                created_at: if field == 0 { at } else { valid },
                updated_at: if field == 1 { at } else { valid },
                finished_at: Some(if field == 2 { at } else { valid }),
            })?;
            Ok(vec![match field {
                0 => response.created_at,
                1 => response.updated_at,
                _ => response.finished_at.expect("present completion timestamp"),
            }])
        });
    }
    assert_eq!(format_optional_time(None).unwrap(), None);
}
