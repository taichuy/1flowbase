use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_legacy_permissions_only, replace_role_permissions, seed_workspace,
    test_api_state_with_database_url, test_app_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::{I18nCatalogRepository, UpsertCatalogTranslationInput};
use domain::{CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogTranslation};
use serde_json::Value;
use tower::ServiceExt;
use uuid::Uuid;

const DATA_MODELS_FEATURE_PERMISSION: &str = "settings_feature.access.system.data-models";

#[tokio::test]
async fn ac_012_013_model_definition_dto_localizes_existing_title_fields_server_side() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let workspace_id = state.bootstrap_workspace_id;
    let catalog_state =
        I18nCatalogRepository::bootstrap_workspace_catalog_state(&state.store, workspace_id)
            .await
            .unwrap();
    I18nCatalogRepository::upsert_catalog_override(
        &state.store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: CatalogTranslation::new(
                CatalogMessageIdentity::new(
                    CatalogModuleId::new("@taichuy/platform/system-metadata").unwrap(),
                    "Users",
                )
                .unwrap(),
                CatalogLocale::new("zh_Hans").unwrap(),
                "用户",
            )
            .unwrap(),
            expected_revision: catalog_state.revision(),
        },
    )
    .await
    .unwrap();

    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        "update model_definitions set title = '用户' where code = 'users' and scope_kind = 'system'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "update model_fields set title = 'Administrator Account Label' where data_model_id = (select id from model_definitions where code = 'users' and scope_kind = 'system') and code = 'account'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "update model_fields set title = '邮箱' where data_model_id = (select id from model_definitions where code = 'users' and scope_kind = 'system') and code = 'email'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "update model_fields set title = '文件名' where data_model_id = (select id from model_definitions where code = 'attachments' and scope_kind = 'system') and code = 'filename'",
    )
    .execute(&pool)
    .await
    .unwrap();

    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &root_cookie)
                .header("x-1flowbase-locale", "zh-Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    let users = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["code"] == "users")
        .unwrap();
    assert_eq!(users["title"], "用户");
    assert!(users.get("localized_title").is_none());
    let account = users["fields"]
        .as_array()
        .unwrap()
        .iter()
        .find(|field| field["code"] == "account")
        .unwrap();
    assert_eq!(account["title"], "Administrator Account Label");
    assert!(account.get("localized_title").is_none());

    let english_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &root_cookie)
                .header("x-1flowbase-locale", "en_US")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(english_response.status(), StatusCode::OK);
    let english_body = response_json(english_response).await;
    let english_users = english_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["code"] == "users")
        .unwrap();
    assert_eq!(english_users["title"], "Users");
    assert_eq!(
        english_users["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["code"] == "email")
            .unwrap()["title"],
        "Email"
    );
    assert_eq!(
        english_users["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["code"] == "account")
            .unwrap()["title"],
        "Administrator Account Label"
    );
    let attachments = english_body["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["code"] == "attachments")
        .unwrap();
    assert_eq!(
        attachments["fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|field| field["code"] == "filename")
            .unwrap()["title"],
        "Filename"
    );
}

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn register_data_models_feature_permission(database_url: &str) {
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .expect("test database should be available")
        .store;
    store
        .upsert_permission_catalog(&[domain::PermissionDefinition {
            code: DATA_MODELS_FEATURE_PERMISSION.to_string(),
            resource: "settings_feature".to_string(),
            action: "access".to_string(),
            scope: "system.data-models".to_string(),
            name: "settings_feature:access:system.data-models".to_string(),
        }])
        .await
        .expect("data-models feature permission should be seeded");
}

async fn current_workspace_id(app: &axum::Router, cookie: &str) -> Uuid {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/session")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    Uuid::parse_str(
        response_json(response).await["data"]["session"]["current_workspace_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

async fn seed_foreign_workspace_model(database_url: &str, workspace_id: Uuid) -> String {
    let pool = sqlx::PgPool::connect(database_url).await.unwrap();
    let actor_user_id: Uuid = sqlx::query_scalar("select id from users where account = 'root'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let model_id = Uuid::now_v7();
    let code = format!("foreign_{}", model_id.simple());
    sqlx::query(
        r#"
        insert into model_definitions (
            id, scope_kind, scope_id, source_kind, code, title,
            physical_table_name, acl_namespace, audit_namespace,
            availability_status, status, owner_kind, is_protected,
            created_by, updated_by
        ) values (
            $1, 'workspace', $2, 'main_source', $3, 'Foreign model',
            $4, $5, $6, 'available', 'published', 'core', false, $7, $7
        )
        "#,
    )
    .bind(model_id)
    .bind(workspace_id)
    .bind(&code)
    .bind(format!("foreign_{}", model_id.simple()))
    .bind(format!("data_model.{code}"))
    .bind(format!("data_model.{code}"))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    code
}

// AC-003/AC-004: the feature alone owns both Settings lists, while the
// model-definition query must still exclude another workspace's rows.
#[tokio::test]
async fn data_models_feature_only_lists_sources_and_current_scope_model_definitions() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_data_models_feature_permission(&database_url).await;
    create_role(&app, &root_cookie, &root_csrf, "data_models_feature_only").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "data_models_feature_only",
        &[DATA_MODELS_FEATURE_PERMISSION],
    )
    .await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "data-models-feature-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["data_models_feature_only"],
    )
    .await;
    let (actor_cookie, _) =
        login_and_capture_cookie(&app, "data-models-feature-actor", "temp-pass").await;
    let actor_workspace_id = current_workspace_id(&app, &actor_cookie).await;
    let actor_workspace_id_text = actor_workspace_id.to_string();
    let foreign_workspace_id = seed_workspace(&database_url, "Foreign data models").await;
    let foreign_model_code =
        seed_foreign_workspace_model(&database_url, foreign_workspace_id).await;

    let sources = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/data-sources")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(sources.status(), StatusCode::OK);
    assert!(response_json(sources).await["data"].is_array());

    let models = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models.status(), StatusCode::OK);
    let models = response_json(models).await;
    let rows = models["data"].as_array().unwrap();
    assert!(!rows.iter().any(|row| row["code"] == foreign_model_code));
    assert!(rows.iter().all(|row| {
        row["scope_kind"] == "system"
            || row["scope_id"].as_str() == Some(actor_workspace_id_text.as_str())
    }));

    let unregistered = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/unregistered")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unregistered.status(), StatusCode::FORBIDDEN);

    create_role(&app, &root_cookie, &root_csrf, "legacy_data_model_actions").await;
    replace_role_legacy_permissions_only(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_data_model_actions",
        &["state_model.view.all", "external_data_source.view.all"],
    )
    .await;
    let legacy_actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy-data-model-action-actor",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &legacy_actor_id,
        &["legacy_data_model_actions"],
    )
    .await;
    let (legacy_cookie, _) =
        login_and_capture_cookie(&app, "legacy-data-model-action-actor", "temp-pass").await;
    let legacy_actions_only = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", legacy_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_actions_only.status(), StatusCode::FORBIDDEN);
}

// AC-004: even the root Settings entry queries model definitions through the
// actor's current workspace projection instead of returning another workspace.
#[tokio::test]
async fn data_models_model_definition_list_excludes_foreign_workspace() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let root_workspace_id = current_workspace_id(&app, &root_cookie).await;
    let root_workspace_id_text = root_workspace_id.to_string();
    let foreign_workspace_id = seed_workspace(&database_url, "Foreign model list").await;
    let foreign_model_code =
        seed_foreign_workspace_model(&database_url, foreign_workspace_id).await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    let rows = payload["data"].as_array().unwrap();
    assert!(!rows.iter().any(|row| row["code"] == foreign_model_code));
    assert!(rows.iter().all(|row| {
        row["scope_kind"] == "system"
            || row["scope_id"].as_str() == Some(root_workspace_id_text.as_str())
    }));
}
