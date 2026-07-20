use super::*;

#[test]
fn parse_run_archive_accepts_selected_runs_trace_zip() {
    let zip_bytes = selected_runs_trace_zip_fixture();

    let archive = parse_run_archive_v1(&zip_bytes).expect("trace zip should parse as run archive");

    assert_eq!(archive.archive_version, 1);
    assert_eq!(
        archive.source.source_kind,
        "application_run_trace_export_zip"
    );
    assert_eq!(archive.entries.len(), 1);
    assert_eq!(
        archive.entries[0].source_run_id,
        "019ef419-9a7d-7eb1-a777-963d56131f65"
    );
    assert_eq!(archive.entries[0].node_runs.len(), 1);
    assert_eq!(archive.manifest.run_count, 1);
    assert_eq!(
        archive.source.exported_by_user_id,
        "019ef3db-7807-7d90-8bd6-eee445d357ca"
    );
}

#[test]
fn trace_archive_import_keeps_non_user_principals_without_a_created_by_identity() {
    for principal_kind in ["public", "scheduler", "user_api_key"] {
        let mut run_export = trace_export_fixture("019ef419-9a7d-7eb1-a777-963d56131f65");
        run_export.run.principal = application_logs::ApplicationRunPrincipalResponse {
            kind: principal_kind.to_string(),
            id: (principal_kind == "user_api_key")
                .then(|| "019ef3db-7807-7d90-8bd6-eee445d357ca".to_string()),
            display_name: None,
        };
        let manifest = ApplicationRunSelectedExportManifestResponse {
            export_version: 1,
            exported_at: "2026-06-24T07:58:00.458337961Z".to_string(),
            export_status: "complete".to_string(),
            application_id: run_export.run.application_id.clone(),
            run_count: 1,
            selected_run_ids: vec![run_export.run.id.clone()],
            entries: vec![ApplicationRunSelectedExportManifestRunResponse {
                run_id: run_export.run.id.clone(),
                title: run_export.run.title.clone(),
                started_at: run_export.run.started_at.clone(),
                filename: format!("runs/{principal_kind}.json"),
                export_status: "complete".to_string(),
                export_warning_count: 0,
            }],
        };

        let archive = build_archive_from_trace_exports(manifest, vec![run_export])
            .expect("non-user trace export should remain importable");

        assert_eq!(archive.source.exported_by_user_id, "", "{principal_kind}");
    }
}

fn selected_runs_trace_zip_fixture() -> Vec<u8> {
    let run_id = "019ef419-9a7d-7eb1-a777-963d56131f65";
    let entry_path = "runs/001_20260623T104939197566Z_019ef419_hi.json";
    let manifest = ApplicationRunSelectedExportManifestResponse {
        export_version: 1,
        exported_at: "2026-06-24T07:58:00.458337961Z".to_string(),
        export_status: "complete".to_string(),
        application_id: "019ef3f7-bac8-74a2-82c6-daf4b8246766".to_string(),
        run_count: 1,
        selected_run_ids: vec![run_id.to_string()],
        entries: vec![ApplicationRunSelectedExportManifestRunResponse {
            run_id: run_id.to_string(),
            title: "hi".to_string(),
            started_at: "2026-06-23T10:49:39.197566Z".to_string(),
            filename: entry_path.to_string(),
            export_status: "complete".to_string(),
            export_warning_count: 0,
        }],
    };
    let run_export = trace_export_fixture(run_id);
    let cursor = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    writer.start_file("manifest.json", options).unwrap();
    std::io::Write::write_all(&mut writer, &serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    writer.start_file(entry_path, options).unwrap();
    std::io::Write::write_all(
        &mut writer,
        &serde_json::to_vec_pretty(&run_export).unwrap(),
    )
    .unwrap();

    writer.finish().unwrap().into_inner()
}

fn trace_export_fixture(run_id: &str) -> ApplicationRunTraceExportResponse {
    let application_id = "019ef3f7-bac8-74a2-82c6-daf4b8246766";
    let flow_id = "019ef3f7-bf17-7853-b582-43f7a05a95b7";
    let draft_id = "019ef3f7-bf1b-7f12-9979-5a01adbc67e7";
    let user_id = "019ef3db-7807-7d90-8bd6-eee445d357ca";
    let started_at = "2026-06-23T10:49:39.197566Z";
    let finished_at = "2026-06-23T10:49:40.810245Z";
    let created_at = "2026-06-23T10:49:39.198771Z";
    let updated_at = "2026-06-23T10:49:40.810245Z";
    let run = application_logs::ApplicationRunLogResponse {
        id: run_id.to_string(),
        application_id: application_id.to_string(),
        application_type: "agent_flow".to_string(),
        run_object_kind: "application_run".to_string(),
        run_kind: "debug_flow_run".to_string(),
        status: "succeeded".to_string(),
        title: "hi".to_string(),
        execution_stage: "debug".to_string(),
        invocation_source: "debug".to_string(),
        compatibility_mode: None,
        subject: application_logs::ApplicationRunSubjectResponse {
            kind: "agent_flow".to_string(),
            id: Some(flow_id.to_string()),
            draft_id: Some(draft_id.to_string()),
            target_node_id: None,
        },
        principal: application_logs::ApplicationRunPrincipalResponse {
            kind: "user".to_string(),
            id: Some(user_id.to_string()),
            display_name: Some("root".to_string()),
        },
        correlation: application_logs::ApplicationRunCorrelationResponse {
            api_key_id: None,
            publication_version_id: None,
            external_user: None,
            external_conversation_id: None,
            external_trace_id: None,
            compatibility_mode: None,
            idempotency_key: None,
        },
        started_at: started_at.to_string(),
        finished_at: Some(finished_at.to_string()),
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
    };
    let flow_run = FlowRunResponse {
        id: run_id.to_string(),
        application_id: application_id.to_string(),
        flow_id: flow_id.to_string(),
        draft_id: draft_id.to_string(),
        compiled_plan_id: None,
        run_mode: "debug_flow_run".to_string(),
        status: "succeeded".to_string(),
        target_node_id: None,
        title: "hi".to_string(),
        expand_id: None,
        authorized_account: Some("root".to_string()),
        external_conversation_id: Some("conversation-1".to_string()),
        query: Some("hi".to_string()),
        model: None,
        input_payload: serde_json::json!({ "node-start": { "query": "hi" } }),
        output_payload: serde_json::json!({ "answer": "hello" }),
        error_payload: None,
        created_by: user_id.to_string(),
        started_at: started_at.to_string(),
        finished_at: Some(finished_at.to_string()),
        created_at: created_at.to_string(),
        updated_at: updated_at.to_string(),
    };
    let node_run = NodeRunResponse {
        id: "019ef419-9af9-7612-a0c2-39520f1f7a24".to_string(),
        flow_run_id: run_id.to_string(),
        node_id: "node-start".to_string(),
        node_type: "start".to_string(),
        node_alias: "Start".to_string(),
        status: "succeeded".to_string(),
        input_payload: serde_json::json!({ "query": "hi" }),
        input_payload_view: serde_json::json!({ "query": "hi" }),
        output_payload: serde_json::json!({}),
        error_payload: None,
        metrics_payload: serde_json::json!({}),
        debug_payload: serde_json::json!({}),
        started_at: started_at.to_string(),
        finished_at: Some(finished_at.to_string()),
    };
    let event = RunEventResponse {
        id: "019ef419-9ab6-7d70-ae31-13c2a546b006".to_string(),
        flow_run_id: run_id.to_string(),
        node_run_id: None,
        sequence: 1,
        event_type: "flow_run_started".to_string(),
        payload: serde_json::json!({ "run_mode": "debug_flow_run" }),
        created_at: started_at.to_string(),
    };
    let statistics = application_logs::ApplicationRunStatisticsResponse {
        total_tokens: Some(22),
        input_tokens: Some(13),
        output_tokens: Some(9),
        input_cache_hit_tokens: Some(0),
        input_cache_hit_rate: application_logs::input_cache_hit_rate_for_response(
            Some(22),
            Some(0),
        ),
        unique_node_count: 1,
        tool_callback_count: 0,
    };
    let projection_status = ApplicationRunTraceProjectionStatusResponse {
        projection_status: "succeeded".to_string(),
        projection_version: 1,
        source_watermark: "1".to_string(),
        attempt_count: 1,
        last_attempt_at: Some(finished_at.to_string()),
        last_success_at: Some(finished_at.to_string()),
        last_error_code: None,
        last_error_stage: None,
        last_error_source_kind: None,
        last_error_source_locator: None,
        last_error_ref: None,
        retriable: false,
    };
    let detail = application_logs::ApplicationRunTypedDetailResponse {
        kind: "agent_flow".to_string(),
        flow_run: flow_run.clone(),
        answer_snapshot: None,
        node_runs: vec![node_run.clone()],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: vec![event.clone()],
        stitched_trace: Vec::new(),
    };
    let trace_tree = ApplicationRunTraceExportTreeResponse {
        run: run.clone(),
        statistics: statistics.clone(),
        flow_run: flow_run.clone(),
        answer_snapshot: None,
        projection_status,
        nodes: Vec::new(),
    };

    ApplicationRunTraceExportResponse {
        export_version: 1,
        exported_at: "2026-06-24T07:58:00.458337961Z".to_string(),
        export_status: "complete".to_string(),
        export_warnings: Vec::new(),
        run,
        statistics,
        detail,
        flow_run,
        answer_snapshot: None,
        node_runs: vec![node_run],
        checkpoints: Vec::new(),
        callback_tasks: Vec::new(),
        events: vec![event],
        stitched_trace: Vec::new(),
        trace_tree,
    }
}
