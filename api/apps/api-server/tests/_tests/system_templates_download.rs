use super::*;
use serde_json::json;

struct TemplateRoot(std::path::PathBuf);
impl Drop for TemplateRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

async fn post(
    app: &Router,
    path: &str,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn login(app: &Router, account: &str, password: &str) -> (String, String) {
    let response = post(app, "/api/public/auth/sign-in", "", "", json!({
        "authenticator_id":"00000000-0000-0000-0000-000000000001", "identifier":account, "password":password
    })).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    (
        cookie,
        payload["data"]["csrf_token"].as_str().unwrap().to_owned(),
    )
}

#[tokio::test]
async fn template_download_is_authorized_and_confirmation_reuses_the_downloaded_package() {
    let root = TemplateRoot(
        std::env::temp_dir().join(format!("template-download-{}", uuid::Uuid::new_v4())),
    );
    let directory = root.0.join("@fixture/download-once");
    std::fs::create_dir_all(&directory).unwrap();
    let package = json!({
        "schema_version":"1flowbase.portable-template/v2", "pages":[], "applications":[], "data_models":[], "plugins":[],
        "i18n_entries":[{"key":"Fixture download once", "locale":"en_US", "translation":"Downloaded once"}],
        "release":{"template_id":"@fixture/download-once", "release_version":1, "name":"Download once",
            "description":"Download fixture", "exported_at":"2026-10-31T00:00:00Z", "exported_from_system_version":"0.6.0"}
    });
    std::fs::write(directory.join("manifest.json"), json!({
        "schema_version":"1flowbase.application-template-archive/v1", "package":package, "files":[]
    }).to_string()).unwrap();
    let mut config = default_test_config();
    config.application_template_root = root.0.to_string_lossy().into_owned();
    let app = test_app_with_config(config).await;
    let (cookie, csrf) = login(&app, "root", "change-me").await;
    let reference = json!({"catalog_id":"builtin:@fixture/download-once", "release_version":1});
    let base = "/api/console/settings/system-templates";
    let unauthenticated = post(&app, &format!("{base}/download"), "", "", reference.clone()).await;
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    create_member(&app, &cookie, &csrf, "template-member").await;
    let (member_cookie, member_csrf) = login(&app, "template-member", "temp-pass").await;
    let denied = post(
        &app,
        &format!("{base}/download"),
        &member_cookie,
        &member_csrf,
        reference.clone(),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let downloaded = post(
        &app,
        &format!("{base}/download"),
        &cookie,
        &csrf,
        reference.clone(),
    )
    .await;
    let status = downloaded.status();
    let downloaded: Value =
        serde_json::from_slice(&to_bytes(downloaded.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    assert_eq!(status, StatusCode::OK, "{downloaded}");
    let downloaded = downloaded["data"].clone();
    assert_eq!(downloaded["i18n_entries"], package["i18n_entries"]);
    assert_eq!(downloaded["release"], package["release"]);
    std::fs::remove_dir_all(&directory).unwrap();
    for (operation, outcome) in [("preview", "valid"), ("install", "complete")] {
        let response = post(
            &app,
            &format!("{base}/{operation}"),
            &cookie,
            &csrf,
            downloaded.clone(),
        )
        .await;
        let status = response.status();
        let payload: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(status, StatusCode::OK, "{operation}: {payload}");
        assert_eq!(payload["data"][outcome], true, "{operation}: {payload}");
        let rejected = post(
            &app,
            &format!("{base}/{operation}"),
            &cookie,
            &csrf,
            reference.clone(),
        )
        .await;
        assert_eq!(rejected.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
    let exported = post(
        &app,
        &format!("{base}/export"),
        &cookie,
        &csrf,
        json!({"i18n_keys":["Fixture download once"]}),
    )
    .await;
    assert_eq!(exported.status(), StatusCode::OK);
    let exported: Value =
        serde_json::from_slice(&to_bytes(exported.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(exported["data"]["i18n_entries"], package["i18n_entries"]);
    // Keep the router's temporary database guard alive until both requests finish.
    for (session_cookie, session_csrf) in [(&cookie, &csrf), (&member_cookie, &member_csrf)] {
        let logout = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/console/session")
                    .header("cookie", session_cookie)
                    .header("x-csrf-token", session_csrf)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    }
    drop(app);
}
