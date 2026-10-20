use super::*;
use storage_ephemeral::MokaCacheStore;

fn fixture(browser: &str) -> Transaction {
    Transaction {
        login_entry_id: Uuid::new_v4(),
        connection_id: Uuid::new_v4(),
        auth_type: "github".into(),
        installation_id: Uuid::new_v4(),
        contribution_id: "github.authentication".into(),
        provider_fingerprint: "artifact:generation:authority".into(),
        entry_fingerprint: "entry".into(),
        connection_fingerprint: "connection".into(),
        callback_url: "https://app.example/api/public/auth/external/callback".into(),
        browser_hash: digest(browser),
        code_verifier: random_token(),
        expires_at_unix: 1_000,
        binding: None,
        purpose: transaction::Purpose::Bind,
    }
}

#[test]
fn rfc7636_s256_vector_and_state_entropy_shape() {
    assert_eq!(
        challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
    );
    let state = random_token();
    assert!(valid_token(&state));
    assert!(!valid_token(&format!("{state}=")));
    assert_ne!(state, random_token());
    assert!(!key(&state).contains(&state));
}

#[tokio::test]
async fn wrong_browser_does_not_burn_state_then_exact_browser_claims_once() {
    let cache = MokaCacheStore::new("oauth-fixture", 100);
    let state = random_token();
    let browser = random_token();
    store(&cache, &state, &fixture(&browser)).await.unwrap();
    assert!(claim(&cache, &state, &random_token(), 500).await.is_err());
    assert!(claim(&cache, &state, &browser, 500).await.is_ok());
    assert!(claim(&cache, &state, &browser, 500).await.is_err());
}

#[tokio::test]
async fn absent_expired_and_claim_marker_replays_are_rejected() {
    let cache = MokaCacheStore::new("oauth-fixture", 100);
    let state = random_token();
    let browser = random_token();
    assert!(claim(&cache, &state, &browser, 500).await.is_err());
    let transaction = fixture(&browser);
    store(&cache, &state, &transaction).await.unwrap();
    assert!(claim(&cache, &state, &browser, 1_000).await.is_err());
    // Simulate another server having atomically claimed before deleting state.
    cache
        .set_if_absent_json(
            &format!("{}:claimed", key(&state)),
            serde_json::Value::Bool(true),
            Some(time::Duration::minutes(20)),
        )
        .await
        .unwrap();
    assert!(claim(&cache, &state, &browser, 500).await.is_err());
}

#[tokio::test]
async fn concurrent_callbacks_have_exactly_one_winner() {
    let cache = MokaCacheStore::new("oauth-fixture", 100);
    let state = random_token();
    let browser = random_token();
    store(&cache, &state, &fixture(&browser)).await.unwrap();
    let (a, b) = tokio::join!(
        claim(&cache, &state, &browser, 500),
        claim(&cache, &state, &browser, 500)
    );
    assert_ne!(a.is_ok(), b.is_ok());
}

#[test]
fn binding_requires_original_unexpired_session_and_same_actor() {
    let actor = domain::ActorContext::root(Uuid::new_v4(), Uuid::new_v4(), "root");
    let mut session = domain::SessionRecord {
        session_id: random_token(),
        user_id: actor.user_id,
        tenant_id: actor.tenant_id,
        current_workspace_id: actor.current_workspace_id,
        active_role_code: "root".into(),
        session_version: 1,
        csrf_token: random_token(),
        expires_at_unix: 1000,
    };
    let binding = BindingIntent {
        actor_id: actor.user_id,
        session_hash: digest(&session.session_id),
    };
    assert!(binding_matches(&binding, &session, &actor, 500));
    assert!(!binding_matches(&binding, &session, &actor, 1000));
    let original = session.session_id.clone();
    session.session_id = random_token();
    assert!(!binding_matches(&binding, &session, &actor, 500));
    session.session_id = original;
    session.user_id = Uuid::new_v4();
    assert!(!binding_matches(&binding, &session, &actor, 500));
}
