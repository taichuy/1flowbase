use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::io::{Cursor, Write};
use tower::ServiceExt;
use uuid::Uuid;

fn archive_fixture() -> Vec<u8> {
    let applications: Vec<_> = (0..4).map(|index| {
        let workflow = index != 1;
        json!({
            "application": {"application_type": if workflow {"workflow"} else {"agent_flow"},
                "workflow_trigger_type": if workflow {json!("schedule")} else {Value::Null},
                "name": format!("Batch source {index}"), "description": "batch round trip",
                "icon": null, "icon_type": null, "icon_background": null},
            "flow_document": if workflow {domain::default_flow_document_for_application(domain::ApplicationType::Workflow, Uuid::now_v7())}
                else {domain::default_flow_document(Uuid::now_v7())},
            "dependencies": [],
            "workflow_trigger_config": if workflow {json!({"kind":"schedule", "cron":"0 0 * * * *", "timezone":"UTC", "input_payload":{}})} else {Value::Null}
        })
    }).collect();
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&json!({"schema_version":"1flowbase.application-archive/v1", "applications":applications})).unwrap()).unwrap();
    zip.finish().unwrap().into_inner()
}

async fn upload(
    app: &Router,
    cookie: &str,
    csrf: Option<&str>,
    route: &str,
    archive: &[u8],
    selections: Option<Value>,
) -> (StatusCode, Value) {
    let boundary = "archive-batch-test";
    let mut body = format!("--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"applications-4-items.zip\"\r\nContent-Type: application/zip\r\n\r\n").into_bytes();
    body.extend_from_slice(archive);
    body.extend_from_slice(b"\r\n");
    if let Some(selections) = selections {
        body.extend_from_slice(format!("--{boundary}\r\nContent-Disposition: form-data; name=\"applications\"\r\n\r\n{selections}\r\n").as_bytes());
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    let mut request = Request::builder()
        .method("POST")
        .uri(route)
        .header("cookie", cookie)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        );
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (status, value)
}

#[tokio::test]
async fn archive_batch_ac_001_004_mixed_zip_previews_imports_as_drafts_and_rejects_bad_selection() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let archive = archive_fixture();
    let (status, preview) = upload(
        &app,
        &cookie,
        None,
        "/api/console/applications/archive/preview",
        &archive,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{preview}");
    let entries = preview["data"]["applications"].as_array().unwrap();
    assert_eq!(entries.len(), 4);
    for (index, entry) in entries.iter().enumerate() {
        assert_eq!(entry["entry_index"], index);
        assert!(
            entry["preview"]["unresolved_nodes"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{entry}"
        );
    }
    let bad = json!([{"entry_index":4,"name":"Invalid"}]);
    let (status, _) = upload(
        &app,
        &cookie,
        Some(&csrf),
        "/api/console/applications/archive/import",
        &archive,
        Some(bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let selections = Value::Array(
        (0..4)
            .map(|index| json!({"entry_index":index,"name":format!("Batch restored {index}")}))
            .collect(),
    );
    let (status, _) = upload(
        &app,
        &cookie,
        None,
        "/api/console/applications/archive/import",
        &archive,
        Some(selections.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, imported) = upload(
        &app,
        &cookie,
        Some(&csrf),
        "/api/console/applications/archive/import",
        &archive,
        Some(selections),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{imported}");
    let results = imported["data"]["results"].as_array().unwrap();
    assert_eq!(results.len(), 4);
    for (index, result) in results.iter().enumerate() {
        assert_eq!(result["status"], "succeeded", "{result}");
        assert_eq!(
            result["result"]["application"]["name"],
            format!("Batch restored {index}")
        );
        let document = &result["result"]["orchestration"]["draft"]["document"];
        let nodes = document["graph"]["nodes"].as_array().unwrap();
        assert!(nodes.iter().all(|node| node["type"] != "unresolved_node"));
        if index != 1 {
            assert!(nodes.iter().any(|node| node["type"] == "workflow_start"));
        }
        assert!(result["result"]["orchestration"]["versions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|v| v["is_current_publication"] == false));
    }
}

#[tokio::test]
async fn archive_batch_path_conflict_fails_only_conflicting_entry() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let mut package: Value = {
        let mut zip = zip::ZipArchive::new(Cursor::new(archive_fixture())).unwrap();
        serde_json::from_reader(zip.by_name("manifest.json").unwrap()).unwrap()
    };
    for index in [0, 2] {
        package["applications"][index]["application"]["workflow_trigger_type"] = json!("extension");
        package["applications"][index]["workflow_trigger_config"] = json!({
            "kind":"extension", "mapping": {"input":{"query_target":"node-start.query"},"output":{},"extension":{"slug":"batch-conflict", "method":"POST", "response_mode":"async"}}
        });
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(&serde_json::to_vec(&package).unwrap())
        .unwrap();
    let archive = zip.finish().unwrap().into_inner();
    let (status, imported) = upload(
        &app,
        &cookie,
        Some(&csrf),
        "/api/console/applications/archive/import",
        &archive,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{imported}");
    assert_eq!(imported["data"]["succeeded_count"], 3, "{imported}");
    assert_eq!(imported["data"]["failed_count"], 1, "{imported}");
    assert_eq!(imported["data"]["partial_count"], 0, "{imported}");
    assert_eq!(
        imported["data"]["results"][2],
        json!({"entry_index":2,"status":"failed","code":"extension_slug"})
    );
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let list: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(
        !list.to_string().contains("Batch source 2"),
        "conflicting application must roll back"
    );
}

/// Manual regression for an external archive without committing private application content.
#[tokio::test]
#[ignore = "requires APPLICATION_ARCHIVE_TEST_FILE"]
async fn archive_batch_external_file_round_trip() {
    let archive = std::fs::read(std::env::var("APPLICATION_ARCHIVE_TEST_FILE").unwrap()).unwrap();
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, preview) = upload(
        &app,
        &cookie,
        None,
        "/api/console/applications/archive/preview",
        &archive,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let entries = preview["data"]["applications"].as_array().unwrap();
    assert!(!entries.is_empty());
    let (status, imported) = upload(
        &app,
        &cookie,
        Some(&csrf),
        "/api/console/applications/archive/import",
        &archive,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(imported["data"]["succeeded_count"], entries.len());
    assert_eq!(imported["data"]["failed_count"], 0);
    assert_eq!(imported["data"]["partial_count"], 0);
    for result in imported["data"]["results"].as_array().unwrap() {
        let orchestration = &result["result"]["orchestration"];
        assert!(orchestration["versions"]
            .as_array()
            .unwrap()
            .iter()
            .all(|version| version["is_current_publication"] == false));
        assert!(orchestration["draft"]["document"]["graph"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .all(|node| node["type"] != "unresolved_node"));
    }
    let (_, repeated) = upload(
        &app,
        &cookie,
        Some(&csrf),
        "/api/console/applications/archive/import",
        &archive,
        None,
    )
    .await;
    let conflicts = repeated["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|result| result["status"] == "failed")
        .count();
    assert_eq!(repeated["data"]["partial_count"], 0);
    for result in repeated["data"]["results"].as_array().unwrap() {
        if result["status"] == "failed" {
            assert_eq!(result["code"], "extension_slug");
        }
    }
    eprintln!(
        "External archive: {} entries imported as drafts; repeat import: {} path conflicts",
        entries.len(),
        conflicts
    );
}
