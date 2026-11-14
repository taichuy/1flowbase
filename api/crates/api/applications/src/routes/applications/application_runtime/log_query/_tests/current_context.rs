use super::*;
use serde_json::json;

fn record(id: Uuid, kind: &str) -> control_plane::ports::ApplicationLogRecordSummary {
    serde_json::from_value(json!({
        "record_id":id,"application_id":Uuid::nil(),"source_kind":kind,
        "source_id":null,"source_client":null,"source_session_id":null,"source_task_id":null,
        "native_run_id":null,"log_conversation_id":null,"requested_model_id":null,"reasoning_effort":null,
        "status":"running","outcome":"in_progress","title":"fixture",
        "total_tokens":null,"input_tokens":null,"output_tokens":null,"input_cache_hit_tokens":null,
        "total_cost":null,"cost_breakdown":{"total_cost":null},"started_at":"","finished_at":null,
        "created_at":"","updated_at":"","available_views":[]
    })).unwrap()
}

#[test]
fn current_context_returns_canonical_record_and_never_chooses_from_ambiguous_matches() {
    let context = McpClientContext {
        thread_id: Some("thread".into()),
        turn_id: Some("turn".into()),
        ..Default::default()
    };
    let id = Uuid::now_v7();
    for kind in ["native", "imported"] {
        let mut response = CurrentRunContextResponse::from_context(&context);
        response.resolve(ApplicationLogRecordsPage {
            items: vec![record(id, kind)],
            next_cursor: None,
        });
        assert_eq!(response.status, CurrentRunContextStatus::Resolved);
        assert_eq!(response.record_id, Some(id));
        assert_eq!(response.native_run_id, (kind == "native").then_some(id));
    }
    for page in [
        ApplicationLogRecordsPage {
            items: vec![record(id, "native"), record(Uuid::now_v7(), "native")],
            next_cursor: None,
        },
        ApplicationLogRecordsPage {
            items: vec![record(id, "imported")],
            next_cursor: Some("more".into()),
        },
    ] {
        let mut response = CurrentRunContextResponse::from_context(&context);
        response.resolve(page);
        assert_eq!(response.status, CurrentRunContextStatus::Ambiguous);
        assert_eq!(response.record_id, None);
        assert_eq!(response.application_id, None);
        assert_eq!(response.native_run_id, None);
    }
    let mut response = CurrentRunContextResponse::from_context(&context);
    response.resolve(ApplicationLogRecordsPage {
        items: vec![],
        next_cursor: None,
    });
    assert_eq!(response.status, CurrentRunContextStatus::NotFound);
    for context in [
        McpClientContext::default(),
        McpClientContext {
            session_id: Some("session".into()),
            ..Default::default()
        },
    ] {
        assert_eq!(
            CurrentRunContextResponse::from_context(&context).status,
            CurrentRunContextStatus::MissingIdentity
        );
        assert!(context_filter(&context).is_none());
    }
}
