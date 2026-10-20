use super::*;

fn proof(actor_id: Uuid, session_id: &str) -> PasswordSetupProof {
    PasswordSetupProof {
        actor_id,
        session_hash: transaction::digest(session_id),
        identity_id: Uuid::now_v7(),
        login_entry_id: Uuid::now_v7(),
        provider_fingerprint: "verified-provider-fingerprint".into(),
        authority_namespace: "managed:fixture/provider:auth".into(),
        entry_fingerprint: "entry-fingerprint".into(),
        connection_fingerprint: "connection-fingerprint".into(),
        nonce: transaction::random_token(),
        expires_at: expires_at(),
    }
}

#[tokio::test]
async fn password_proof_is_actor_session_and_expiry_bound_and_never_transferred() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor = Uuid::now_v7();
    let session = "password-setup-session";
    assert!(read(&state, actor, session).await.unwrap().is_none());
    issue(&state, proof(actor, session)).await.unwrap();
    assert!(read(&state, actor, session).await.unwrap().is_some());
    assert!(read(&state, Uuid::now_v7(), session)
        .await
        .unwrap()
        .is_none());
    assert!(read(&state, actor, "another-session")
        .await
        .unwrap()
        .is_none());
    assert!(claim(&state, actor, "another-session").await.is_err());
    let mut expired = proof(actor, session);
    expired.expires_at = time::OffsetDateTime::now_utc().unix_timestamp() - 1;
    issue(&state, expired).await.unwrap();
    assert!(read(&state, actor, session).await.unwrap().is_none());
    assert!(claim(&state, actor, session).await.is_err());
}

#[tokio::test]
async fn password_proof_claim_is_atomic_single_use_and_new_nonce_survives_old_claim() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor = Uuid::now_v7();
    let session = "one-use-password-proof-session";
    let first = proof(actor, session);
    let identity = first.identity_id;
    issue(&state, first).await.unwrap();
    let (one, two) = tokio::join!(claim(&state, actor, session), claim(&state, actor, session));
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    let claimed = one.ok().or_else(|| two.ok()).unwrap();
    assert_eq!(claimed.identity_id, identity);
    assert!(claim(&state, actor, session).await.is_err());
    assert!(read(&state, actor, session).await.unwrap().is_none());
    let replacement = proof(actor, session);
    let nonce = replacement.nonce.clone();
    issue(&state, replacement).await.unwrap();
    assert_eq!(claim(&state, actor, session).await.unwrap().nonce, nonce);
}

#[tokio::test]
async fn password_proof_rejects_a_cache_record_with_a_mismatched_session_hash() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor = Uuid::now_v7();
    let session = "cache-session-selector";
    let mut forged = proof(actor, session);
    forged.session_hash = transaction::digest("different-bound-session");
    state
        .infrastructure
        .cache_store()
        .set_json(
            &key(&transaction::digest(session)),
            serde_json::to_value(forged).unwrap(),
            Some(time::Duration::seconds(TTL)),
        )
        .await
        .unwrap();
    assert!(read(&state, actor, session).await.unwrap().is_none());
    assert!(claim(&state, actor, session).await.is_err());
}
