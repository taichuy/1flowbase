use crate::{
    _tests::{
        mcp_protocol_routes::{create_api_key, create_mcp_instance},
        support::{login_and_capture_cookie, test_api_state_with_database_url},
    },
    app_state::ApiState,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tower::ServiceExt;
const ISSUER: &str = "https://oauth.example.test";
const CALLBACK: &str = "https://chatgpt.com/connector_platform_oauth_redirect";
const VERIFIER: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUV0123456789-._~";
async fn json_response(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap()).unwrap()
}
async fn post(
    app: &Router,
    path: &str,
    body: Value,
    cookie: Option<&str>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri(path)
        .header("content-type", "application/json")
        .header("origin", ISSUER);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn setup() -> (Arc<ApiState>, Router, String, String) {
    let (base, _) = test_api_state_with_database_url().await;
    let state = Arc::new(ApiState {
        mcp_oauth_issuer: Some(ISSUER.into()),
        ..(*base).clone()
    });
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    let key = create_api_key(&app, &cookie, &csrf).await;
    let response=post(&app,"/api/public/mcp-oauth/register",json!({"redirect_uris":[CALLBACK],"token_endpoint_auth_method":"none","client_name":"Untrusted Name"}),None).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let client = json_response(response).await["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    (state, app, key, client)
}
async fn begin(app: &Router, client: &str) -> (String, String) {
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let mut url = url::Url::parse(&format!("{ISSUER}/api/public/mcp-oauth/authorize")).unwrap();
    url.query_pairs_mut().extend_pairs([
        ("client_id", client),
        ("redirect_uri", CALLBACK),
        ("response_type", "code"),
        ("code_challenge", challenge.as_str()),
        ("code_challenge_method", "S256"),
        ("state", "original-state"),
        ("resource", &format!("{ISSUER}/api/mcp/taichuy")),
        ("scope", "mcp:invoke"),
    ]);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("{}?{}", url.path(), url.query().unwrap()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FOUND);
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("SameSite=Lax") && cookie.contains("Secure")
    );
    let cookie = cookie.split(';').next().unwrap().to_owned();
    let url = url::Url::parse(response.headers()["location"].to_str().unwrap()).unwrap();
    let id = url
        .query_pairs()
        .find(|(k, _)| k == "request_id")
        .unwrap()
        .1
        .into_owned();
    (id, cookie)
}
async fn approve(app: &Router, client: &str, key: &str) -> String {
    let (id, cookie) = begin(app, client).await;
    let response = post(
        app,
        "/api/public/mcp-oauth/verify",
        json!({"request_id":id,"api_key":key}),
        Some(&cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let verification = json_response(response).await;
    assert_eq!(verification["instance_name"], "taichuy");
    let response = post(
        app,
        "/api/public/mcp-oauth/decision",
        json!({"request_id":id,"approval_token":verification["approval_token"],"approved":true}),
        Some(&cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = json_response(response).await;
    let url = url::Url::parse(body["redirect_uri"].as_str().unwrap()).unwrap();
    assert!(url
        .query_pairs()
        .any(|(k, v)| k == "state" && v == "original-state"));
    assert!(url.query_pairs().any(|(k, v)| k == "iss" && v == ISSUER));
    url.query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned()
}
async fn token(app: &Router, fields: &[(&str, &str)]) -> axum::response::Response {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(fields.iter().copied());
    let body = serializer.finish();
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/mcp-oauth/token")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap()
}
async fn exchange(app: &Router, client: &str, code: &str) -> axum::response::Response {
    token(
        app,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", client),
            ("code", code),
            ("redirect_uri", CALLBACK),
            ("code_verifier", VERIFIER),
        ],
    )
    .await
}
async fn invoke(app: &Router, bearer: &str, instance: &str) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/mcp/{instance}"))
                .header("content-type", "application/json")
                .header("authorization", format!("Bearer {bearer}"))
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}
#[tokio::test]
async fn oauth_pkce_refresh_replay_resource_and_original_pat_contract() {
    let (_state, app, key, client) = setup().await;
    assert_eq!(invoke(&app, &key, "taichuy").await.status(), StatusCode::OK);
    let challenge = invoke(&app, "invalid", "taichuy").await;
    assert_eq!(challenge.status(), StatusCode::UNAUTHORIZED);
    assert!(challenge.headers()["www-authenticate"]
        .to_str()
        .unwrap()
        .contains("/.well-known/oauth-protected-resource/api/mcp/taichuy"));
    let code = approve(&app, &client, &key).await;
    let wrong = token(
        &app,
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &client),
            ("code", &code),
            ("redirect_uri", CALLBACK),
            (
                "code_verifier",
                "0123456789012345678901234567890123456789012345678901234567890123",
            ),
        ],
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::BAD_REQUEST);
    let (a, b) = tokio::join!(
        exchange(&app, &client, &code),
        exchange(&app, &client, &code)
    );
    assert_eq!(
        [a.status(), b.status()]
            .iter()
            .filter(|s| **s == StatusCode::OK)
            .count(),
        1
    );
    let successful = if a.status() == StatusCode::OK { a } else { b };
    let grant = json_response(successful).await;
    let access = grant["access_token"].as_str().unwrap();
    assert_eq!(
        invoke(&app, access, "taichuy").await.status(),
        StatusCode::OK
    );
    assert_eq!(
        invoke(&app, access, "different-instance").await.status(),
        StatusCode::UNAUTHORIZED
    );
    let refresh = grant["refresh_token"].as_str().unwrap();
    let fields = [
        ("grant_type", "refresh_token"),
        ("client_id", client.as_str()),
        ("refresh_token", refresh),
    ];
    let rotated = token(&app, &fields).await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let rotated = json_response(rotated).await;
    assert_eq!(token(&app, &fields).await.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invoke(&app, rotated["access_token"].as_str().unwrap(), "taichuy")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(invoke(&app, &key, "taichuy").await.status(), StatusCode::OK);
}
#[tokio::test]
async fn oauth_browser_consent_is_key_only_origin_bound_and_one_time() {
    let (_state, app, key, client) = setup().await;
    let (id, cookie) = begin(&app, &client).await;
    let input = json!({"request_id":id,"api_key":key});
    assert_eq!(
        post(&app, "/api/public/mcp-oauth/verify", input.clone(), None)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let evil = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/public/mcp-oauth/verify")
                .header("content-type", "application/json")
                .header("origin", "https://evil.test")
                .header("cookie", &cookie)
                .body(Body::from(input.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(evil.status(), StatusCode::BAD_REQUEST);
    let first = json_response(
        post(
            &app,
            "/api/public/mcp-oauth/verify",
            input.clone(),
            Some(&cookie),
        )
        .await,
    )
    .await;
    let second =
        json_response(post(&app, "/api/public/mcp-oauth/verify", input, Some(&cookie)).await).await;
    let stale = post(
        &app,
        "/api/public/mcp-oauth/decision",
        json!({"request_id":id,"approval_token":first["approval_token"],"approved":true}),
        Some(&cookie),
    )
    .await;
    assert_eq!(stale.status(), StatusCode::BAD_REQUEST);
    assert_ne!(first["approval_token"], second["approval_token"]);
    let deny = post(
        &app,
        "/api/public/mcp-oauth/decision",
        json!({"request_id":id,"approved":false}),
        Some(&cookie),
    )
    .await;
    assert_eq!(deny.status(), StatusCode::OK);
    assert!(json_response(deny).await["redirect_uri"]
        .as_str()
        .unwrap()
        .contains("error=access_denied"));
    assert_eq!(
        post(
            &app,
            "/api/public/mcp-oauth/decision",
            json!({"request_id":id,"approved":false}),
            Some(&cookie)
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}
#[tokio::test]
async fn oauth_grants_follow_key_revocation_expiry_and_permission_changes() {
    let (state, app, key, client) = setup().await;
    let code = approve(&app, &client, &key).await;
    let grant = json_response(exchange(&app, &client, &code).await).await;
    let key_hash = control_plane::auth::hash_api_key_token(&key);
    sqlx::query("update api_keys set expires_at=now()-interval '1 second' where token_hash=$1")
        .bind(&key_hash)
        .execute(state.store.pool())
        .await
        .unwrap();
    assert_eq!(
        invoke(&app, grant["access_token"].as_str().unwrap(), "taichuy")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    sqlx::query("update api_keys set expires_at=null where token_hash=$1")
        .bind(&key_hash)
        .execute(state.store.pool())
        .await
        .unwrap();
    // A detected invalid grant cannot be revived by restoring the underlying key.
    assert_eq!(
        invoke(&app, grant["access_token"].as_str().unwrap(), "taichuy")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let code = approve(&app, &client, &key).await;
    let grant = json_response(exchange(&app, &client, &code).await).await;
    sqlx::query("update roles set updated_at=now() where code='root'")
        .execute(state.store.pool())
        .await
        .unwrap();
    assert_eq!(
        invoke(&app, grant["access_token"].as_str().unwrap(), "taichuy")
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let code = approve(&app, &client, &key).await;
    let grant = json_response(exchange(&app, &client, &code).await).await;
    sqlx::query("update api_keys set enabled=false where token_hash=$1")
        .bind(&key_hash)
        .execute(state.store.pool())
        .await
        .unwrap();
    let fields = [
        ("grant_type", "refresh_token"),
        ("client_id", client.as_str()),
        ("refresh_token", grant["refresh_token"].as_str().unwrap()),
    ];
    assert_eq!(token(&app, &fields).await.status(), StatusCode::BAD_REQUEST);
}
#[tokio::test]
async fn oauth_metadata_and_config_require_explicit_issuer_and_reject_malicious_registration() {
    let (state, _) = test_api_state_with_database_url().await;
    let disabled = crate::app_with_state(state);
    let response = disabled
        .oneshot(
            Request::builder()
                .uri("/api/public/mcp-oauth/config?instance_id=taichuy")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json_response(response).await,
        json!({"enabled":false,"server_url":null,"registration_method":"dynamic_client_registration","scope":"mcp:invoke"})
    );
    let (_state, app, _key, _client) = setup().await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/.well-known/oauth-protected-resource/api/mcp/taichuy")
                .header("host", "evil.test")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        json_response(response).await["authorization_servers"],
        json!([ISSUER])
    );
    let rejected = post(
        &app,
        "/api/public/mcp-oauth/register",
        json!({"redirect_uris":["https://chatgpt.com.evil.test/callback"]}),
        None,
    )
    .await;
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
}
