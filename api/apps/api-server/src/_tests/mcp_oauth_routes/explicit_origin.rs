use super::*;

async fn forwarded(
    app: &Router,
    method: &str,
    address: &str,
    content_type: &str,
    body: String,
    cookie: Option<&str>,
    bearer: Option<&str>,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method(method)
        .uri(address)
        .header("host", "127.0.0.1")
        .header("x-forwarded-proto", "http")
        .header("origin", ISSUER)
        .header("content-type", content_type);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    if let Some(bearer) = bearer {
        request = request.header("authorization", format!("Bearer {bearer}"));
    }
    app.clone()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap()
}

fn query(path: &str, fields: &[(&str, &str)]) -> String {
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.extend_pairs(fields.iter().copied());
    format!("{path}?{}", serializer.finish())
}

#[tokio::test]
async fn oauth_explicit_origin_survives_host_rewriting_through_the_complete_flow() {
    let (state, app, key, _) = setup().await;
    let config = forwarded(
        &app,
        "GET",
        &query(
            "/api/public/mcp-oauth/config",
            &[("instance_id", "taichuy"), ("origin", ISSUER)],
        ),
        "application/json",
        String::new(),
        None,
        None,
    )
    .await;
    assert_eq!(config.status(), StatusCode::OK);
    let config = json_response(config).await;
    let resource = config["server_url"].as_str().unwrap();
    assert_eq!(
        url::Url::parse(resource)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "origin")
            .unwrap()
            .1,
        ISSUER
    );
    assert!(resource.starts_with(ISSUER));
    let probe = forwarded(
        &app,
        "POST",
        resource,
        "text/plain",
        String::new(),
        None,
        None,
    )
    .await;
    assert_eq!(probe.status(), StatusCode::UNAUTHORIZED);
    let challenge = probe.headers()["www-authenticate"].to_str().unwrap();
    let metadata_url = challenge
        .split("resource_metadata=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let metadata = forwarded(
        &app,
        "GET",
        metadata_url,
        "application/json",
        String::new(),
        None,
        None,
    )
    .await;
    assert_eq!(metadata.status(), StatusCode::OK);
    let metadata = json_response(metadata).await;
    assert_eq!(metadata["resource"], resource);
    let issuer = metadata["authorization_servers"][0].as_str().unwrap();
    assert!(issuer.starts_with(ISSUER));
    assert!(url::Url::parse(issuer).unwrap().query().is_none());
    let discovery = forwarded(
        &app,
        "GET",
        &format!("{issuer}/.well-known/openid-configuration"),
        "application/json",
        String::new(),
        None,
        None,
    )
    .await;
    assert_eq!(discovery.status(), StatusCode::OK);
    let discovery = json_response(discovery).await;
    assert_eq!(discovery["issuer"], issuer);
    for field in [
        "authorization_endpoint",
        "token_endpoint",
        "registration_endpoint",
    ] {
        assert!(discovery[field]
            .as_str()
            .unwrap()
            .starts_with(&format!("{issuer}/")));
    }
    let registered = forwarded(
        &app,
        "POST",
        discovery["registration_endpoint"].as_str().unwrap(),
        "application/json",
        json!({"redirect_uris":[CALLBACK],"token_endpoint_auth_method":"none"}).to_string(),
        None,
        None,
    )
    .await;
    assert_eq!(registered.status(), StatusCode::CREATED);
    let client = json_response(registered).await["client_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(VERIFIER.as_bytes()));
    let authorize = query(
        discovery["authorization_endpoint"].as_str().unwrap(),
        &[
            ("client_id", &client),
            ("redirect_uri", CALLBACK),
            ("response_type", "code"),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", "public-origin-state"),
            ("resource", resource),
            ("scope", "mcp:invoke"),
        ],
    );
    let started = forwarded(
        &app,
        "GET",
        &authorize,
        "application/json",
        String::new(),
        None,
        None,
    )
    .await;
    assert_eq!(started.status(), StatusCode::FOUND);
    let cookie_header = started.headers()["set-cookie"].to_str().unwrap();
    assert!(cookie_header.contains("Secure"));
    let cookie = cookie_header.split(';').next().unwrap().to_owned();
    let location = url::Url::parse(started.headers()["location"].to_str().unwrap()).unwrap();
    assert_eq!(location.origin().ascii_serialization(), ISSUER);
    assert_eq!(
        location
            .query_pairs()
            .find(|(k, _)| k == "origin")
            .unwrap()
            .1,
        ISSUER
    );
    let id = location
        .query_pairs()
        .find(|(k, _)| k == "request_id")
        .unwrap()
        .1
        .into_owned();
    let view = forwarded(
        &app,
        "GET",
        &query(
            "/api/public/mcp-oauth/authorization",
            &[("request_id", &id), ("origin", ISSUER)],
        ),
        "application/json",
        String::new(),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(view.status(), StatusCode::OK);
    let wrong_verify = forwarded(
        &app,
        "POST",
        &query(
            "/api/public/mcp-oauth/verify",
            &[("origin", "https://other.example")],
        ),
        "application/json",
        json!({"request_id":id,"api_key":key}).to_string(),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(wrong_verify.status(), StatusCode::BAD_REQUEST);
    let verified = forwarded(
        &app,
        "POST",
        &query("/api/public/mcp-oauth/verify", &[("origin", ISSUER)]),
        "application/json",
        json!({"request_id":id,"api_key":key}).to_string(),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(verified.status(), StatusCode::OK);
    let verified = json_response(verified).await;
    let decision = forwarded(
        &app,
        "POST",
        &query("/api/public/mcp-oauth/decision", &[("origin", ISSUER)]),
        "application/json",
        json!({"request_id":id,"approval_token":verified["approval_token"],"approved":true})
            .to_string(),
        Some(&cookie),
        None,
    )
    .await;
    assert_eq!(decision.status(), StatusCode::OK);
    let decision = json_response(decision).await;
    let callback = url::Url::parse(decision["redirect_uri"].as_str().unwrap()).unwrap();
    assert_eq!(
        callback.query_pairs().find(|(k, _)| k == "iss").unwrap().1,
        issuer
    );
    let code = callback
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let form = query(
        "",
        &[
            ("grant_type", "authorization_code"),
            ("client_id", &client),
            ("code", &code),
            ("redirect_uri", CALLBACK),
            ("code_verifier", VERIFIER),
            ("resource", resource),
        ],
    )
    .trim_start_matches('?')
    .to_owned();
    let token = forwarded(
        &app,
        "POST",
        discovery["token_endpoint"].as_str().unwrap(),
        "application/x-www-form-urlencoded",
        form,
        None,
        None,
    )
    .await;
    assert_eq!(token.status(), StatusCode::OK);
    let token = json_response(token).await;
    let access = token["access_token"].as_str().unwrap();
    let invocation = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"mcp_list","arguments":{"depth":2,"limit":100}}}).to_string();
    let wrong = forwarded(
        &app,
        "POST",
        &query("/api/mcp/taichuy", &[("origin", "https://other.example")]),
        "application/json",
        invocation.clone(),
        None,
        Some(access),
    )
    .await;
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let called = forwarded(
        &app,
        "POST",
        resource,
        "application/json",
        invocation.clone(),
        None,
        Some(access),
    )
    .await;
    assert_eq!(called.status(), StatusCode::OK);
    let called = json_response(called).await;
    assert_eq!(called["result"]["isError"], false);
    assert!(called["result"]["structuredContent"].is_object());
    let refresh = token["refresh_token"].as_str().unwrap();
    let form = query(
        "",
        &[
            ("grant_type", "refresh_token"),
            ("client_id", &client),
            ("refresh_token", refresh),
            ("resource", resource),
        ],
    )
    .trim_start_matches('?')
    .to_owned();
    let rotated = forwarded(
        &app,
        "POST",
        discovery["token_endpoint"].as_str().unwrap(),
        "application/x-www-form-urlencoded",
        form,
        None,
        None,
    )
    .await;
    assert_eq!(rotated.status(), StatusCode::OK);
    let key_hash = control_plane::auth::hash_api_key_token(&key);
    sqlx::query("update api_keys set expires_at=now()-interval '1 second' where token_hash=$1")
        .bind(&key_hash)
        .execute(state.store.pool())
        .await
        .unwrap();
    let revoked = forwarded(
        &app,
        "POST",
        resource,
        "application/json",
        invocation,
        None,
        Some(access),
    )
    .await;
    assert_eq!(revoked.status(), StatusCode::UNAUTHORIZED);
}
