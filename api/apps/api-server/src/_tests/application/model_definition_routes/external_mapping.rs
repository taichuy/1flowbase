use crate::_tests::support::{login_and_capture_cookie, test_app_with_database_url};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::json;
use sqlx::Row;
use tower::ServiceExt;

async fn seed_external_data_source_instance(database_url: &str) -> String {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let actor = sqlx::query(
        r#"
        select users.id as user_id, workspace_memberships.workspace_id as workspace_id
        from users
        join workspace_memberships on workspace_memberships.user_id = users.id
        where users.account = 'root'
        order by workspace_memberships.created_at asc
        limit 1
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_user_id: uuid::Uuid = actor.get("user_id");
    let workspace_id: uuid::Uuid = actor.get("workspace_id");
    let installation_id = uuid::Uuid::now_v7();
    let data_source_instance_id = uuid::Uuid::now_v7();
    let provider_code = format!("route_external_source_{}", data_source_instance_id.simple());

    sqlx::query(
        r#"
        insert into plugin_installations (
            id, provider_code, plugin_id, plugin_version, contract_version, protocol,
            display_name, source_kind, trust_level, verification_status, desired_state,
            artifact_status, runtime_status, availability_status, installed_path,
            metadata_json, created_by
        ) values (
            $1, $2, $3, '0.1.0', '1flowbase.data_source/v1', 'stdio_json',
            'Route External Source', 'uploaded', 'unverified', 'valid', 'active_requested',
            'ready', 'active', 'available', '/tmp/route-external-source',
            '{}', $4
        )
        "#,
    )
    .bind(installation_id)
    .bind(&provider_code)
    .bind(format!("{provider_code}@0.1.0"))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into data_source_instances (
            id, workspace_id, installation_id, source_code, display_name, status,
            config_json, metadata_json, default_data_model_status,
            created_by
        ) values (
            $1, $2, $3, 'route_external_source', 'Route External Source', 'ready',
            '{}', '{}', 'published', $4
        )
        "#,
    )
    .bind(data_source_instance_id)
    .bind(workspace_id)
    .bind(installation_id)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    data_source_instance_id.to_string()
}

async fn seed_existing_external_model(
    database_url: &str,
    external_resource_key: &str,
    external_table_id: Option<&str>,
) -> (String, String) {
    let data_source_instance_id = seed_external_data_source_instance(database_url).await;
    let data_source_instance_uuid = uuid::Uuid::parse_str(&data_source_instance_id).unwrap();
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let actor = sqlx::query(
        r#"
        select users.id as user_id, workspace_memberships.workspace_id as workspace_id
        from users
        join workspace_memberships on workspace_memberships.user_id = users.id
        where users.account = 'root'
        order by workspace_memberships.created_at asc
        limit 1
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_user_id: uuid::Uuid = actor.get("user_id");
    let workspace_id: uuid::Uuid = actor.get("workspace_id");
    let model_id = uuid::Uuid::now_v7();
    let code = format!("external_{}", model_id.simple());

    sqlx::query(
        r#"
        insert into model_definitions (
            id, scope_kind, scope_id, data_source_instance_id, source_kind,
            external_resource_key, external_table_id, external_capability_snapshot,
            code, title, physical_table_name, acl_namespace, audit_namespace,
            availability_status, status, owner_kind, is_protected, created_by, updated_by
        ) values (
            $1, 'system', $2, $3, 'external_source', $4, $5, '{}',
            $6, 'External Contacts', $7, $8, $9,
            'available', 'published', 'user', false, $10, $10
        )
        "#,
    )
    .bind(model_id)
    .bind(domain::SYSTEM_SCOPE_ID)
    .bind(data_source_instance_uuid)
    .bind(external_resource_key)
    .bind(external_table_id)
    .bind(&code)
    .bind(format!("external_{}", model_id.simple()))
    .bind(format!("data_model.{code}"))
    .bind(format!("data_model.{code}"))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into scope_data_model_grants (
            id, scope_kind, scope_id, data_model_id, enabled, permission_profile, created_by
        ) values ($1, 'workspace', $2, $3, true, 'scope_all', $4)
        "#,
    )
    .bind(uuid::Uuid::now_v7())
    .bind(workspace_id)
    .bind(model_id)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    (data_source_instance_id, model_id.to_string())
}

#[tokio::test]
async fn create_external_model_and_field_mapping_keys() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (data_source_instance_id, model_id) =
        seed_existing_external_model(&database_url, "contacts", Some("crm.contacts")).await;

    let list_model_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/models?data_source_id={data_source_instance_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_model_response.status(), StatusCode::OK);
    let listed_models: serde_json::Value = serde_json::from_slice(
        &to_bytes(list_model_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let created_model = &listed_models["data"][0];
    assert_eq!(
        created_model["data_source_id"],
        json!(data_source_instance_id)
    );
    assert!(created_model.get("data_source_instance_id").is_none());
    assert_eq!(created_model["source_kind"], json!("external_source"));
    assert_eq!(created_model["external_resource_key"], json!("contacts"));
    assert_eq!(created_model["external_table_id"], json!("crm.contacts"));
    let update_model_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "external_table_id": "crm.contacts.v2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_model_response.status(), StatusCode::OK);
    let updated_model: serde_json::Value = serde_json::from_slice(
        &to_bytes(update_model_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        updated_model["data"]["external_table_id"],
        json!("crm.contacts.v2")
    );

    let create_field_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/fields"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "email",
                        "title": "Email",
                        "external_field_key": "properties.email",
                        "field_kind": "string"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_field_response.status(), StatusCode::CREATED);
    let created_field: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        created_field["data"]["external_field_key"],
        json!("properties.email")
    );
}

#[tokio::test]
async fn unsafe_external_system_all_scope_grant_route_requires_confirmation() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (_data_source_instance_id, model_id) =
        seed_existing_external_model(&database_url, "unsafe.contacts", None).await;

    let without_confirmation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/scope-grants"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "system",
                        "scope_id": domain::SYSTEM_SCOPE_ID,
                        "enabled": true,
                        "permission_profile": "system_all"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(without_confirmation.status(), StatusCode::BAD_REQUEST);

    let with_confirmation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/scope-grants"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "system",
                        "scope_id": domain::SYSTEM_SCOPE_ID,
                        "enabled": true,
                        "permission_profile": "system_all",
                        "confirm_unsafe_external_source_system_all": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(with_confirmation.status(), StatusCode::CREATED);
    let created_grant: serde_json::Value = serde_json::from_slice(
        &to_bytes(with_confirmation.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let grant_id = created_grant["data"]["id"].as_str().unwrap();

    let update_without_confirmation = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/models/{model_id}/scope-grants/{grant_id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "enabled": false }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        update_without_confirmation.status(),
        StatusCode::BAD_REQUEST
    );

    let update_with_confirmation = app
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/models/{model_id}/scope-grants/{grant_id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "enabled": false,
                        "confirm_unsafe_external_source_system_all": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_with_confirmation.status(), StatusCode::OK);
}
