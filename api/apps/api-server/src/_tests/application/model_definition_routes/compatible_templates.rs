use super::*;

#[tokio::test]
async fn compatible_template_catalog_projects_descriptor_system_fields() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/data-models/model-templates?data_source_id=main")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let payload: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    let templates = payload["data"].as_array().unwrap();
    let general = templates
        .iter()
        .find(|template| template["template_code"] == json!("general"))
        .unwrap();
    let ordered_tree = templates
        .iter()
        .find(|template| template["template_code"] == json!("ordered_tree"))
        .unwrap();

    assert_eq!(general["system_fields"].as_array().unwrap().len(), 6);
    assert_eq!(ordered_tree["system_fields"].as_array().unwrap().len(), 9);
    assert_eq!(
        general["system_fields"][0],
        json!({
            "code": "id",
            "summary": "id system field",
            "description": "Core-managed `id` field.",
            "field_kind": "string",
            "required": true
        })
    );
    let ordered_tree_fields = ordered_tree["system_fields"].as_array().unwrap();
    let created_at = ordered_tree_fields
        .iter()
        .find(|field| field["code"] == json!("created_at"))
        .unwrap();
    assert_eq!(created_at["field_kind"], json!("datetime"));
    let parent_id = ordered_tree_fields
        .iter()
        .find(|field| field["code"] == json!("parent_id"))
        .unwrap();
    assert_eq!(
        *parent_id,
        json!({
            "code": "parent_id",
            "summary": "parent_id system field",
            "description": "Core-managed `parent_id` field.",
            "field_kind": "string",
            "required": false
        })
    );
}
