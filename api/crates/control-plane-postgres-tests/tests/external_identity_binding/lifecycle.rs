use super::*;
use control_plane::{
    auth::{AuthenticationMethodsService, LoginCommand},
    ports::ExternalIdentityLifecycleRepository,
};

// Argon2 upstream known-answer vector: password="password", salt="somesalt".
const PASSWORD_HASH: &str =
    "$argon2id$v=19$m=256,t=2,p=1$c29tZXNhbHQ$nf65EOgLrQMR/uIPnA4rEsF5h7TKyQwu9U1bMCHGi/4";
const AUTHORITY: &str = "managed:fixture/github:github";

fn canonical(
    entry: &domain::LoginEntryRecord,
    subject: &str,
    email: &str,
) -> VerifiedExternalIdentity {
    registration_identity(entry.connection_id, subject,
        serde_json::json!({"email":email,"email_verified":true,"account":format!("github-{subject}"),"nickname":"GitHub account"}))
        .with_authority_namespace(AUTHORITY.into()).unwrap()
}

async fn external_member(
    store: &storage_durable_postgres::PgControlPlaneStore,
    entry: &domain::LoginEntryRecord,
    subject: &str,
) -> (domain::ActorContext, VerifiedExternalIdentity) {
    let identity = canonical(entry, subject, &format!("{subject}@example.com"));
    let result = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    )
    .complete_external_login(CompleteExternalLoginCommand {
        login_entry_id: entry.id,
        identity: identity.clone(),
        registration_eligible: true,
    })
    .await
    .unwrap();
    let payload: serde_json::Value = sqlx::query_scalar("select payload from audit_logs where target_id=$1 and event_code='member.external_registered'")
        .bind(result.actor.user_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(payload["authority_namespace"], AUTHORITY);
    assert_eq!(payload["subject_value"], subject);
    assert_eq!(payload["issuer"], "https://github.com");
    assert_eq!(
        payload["new_owner_user_id"],
        result.actor.user_id.to_string()
    );
    assert!(payload["old_owner_user_id"].is_null() && payload["old_state"].is_null());
    assert_eq!(payload["new_state"], "bound");
    assert!(payload["identity_id"].as_str().is_some());
    (result.actor, identity)
}

#[tokio::test]
async fn canonical_subject_crosses_connections_but_never_authorities() {
    let (store, _, root) = seed_store().await;
    let a = external_entry(&store).await;
    let b = external_entry(&store).await;
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    );
    let actor = AuthRepository::load_actor_context_for_user(&store, root.id)
        .await
        .unwrap();
    let ia = canonical(&a, "same-subject", "new@example.com");
    let ib = canonical(&b, "same-subject", "new@example.com");
    kernel
        .bind_external_identity(&actor, ia.clone())
        .await
        .unwrap();
    kernel
        .bind_external_identity(&actor, ib.clone())
        .await
        .unwrap();
    let login = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: b.id,
            identity: ib.clone(),
            registration_eligible: false,
        })
        .await
        .unwrap();
    assert_eq!(login.actor.user_id, root.id);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from user_auth_identities where authority_namespace=$1",
    )
    .bind(AUTHORITY)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "connection rotation must not duplicate a canonical identity"
    );
    let race_a = canonical(&a, "registration-race", "registration-race@example.com");
    let race_b = canonical(&b, "registration-race", "registration-race@example.com");
    let (first, second) = tokio::join!(
        kernel.complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: a.id,
            identity: race_a,
            registration_eligible: true
        }),
        kernel.complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: b.id,
            identity: race_b,
            registration_eligible: true
        })
    );
    assert_eq!(
        first.unwrap().actor.user_id,
        second.unwrap().actor.user_id,
        "canonical registration lock crosses connections"
    );
    let other = ib
        .with_authority_namespace("managed:other/github:github".into())
        .unwrap();
    assert!(store
        .find_external_identity_binding(&other)
        .await
        .unwrap()
        .is_none());
    let login = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: b.id,
            identity: other,
            registration_eligible: true,
        })
        .await
        .unwrap();
    assert_ne!(
        login.actor.user_id, root.id,
        "equal provider subject does not cross authority namespaces"
    );
    let binding = store
        .find_external_identity_binding(&ia)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(binding.auth_type, "github");
    assert_eq!(binding.profile["nickname"], "GitHub account");
    assert_eq!(binding.state, "bound");
}

#[tokio::test]
async fn unlink_reserves_owner_blocks_signup_and_only_explicit_owner_binding_restores() {
    let (store, _, root) = seed_store().await;
    sqlx::query("update users set password_hash=$2 where id=$1")
        .bind(root.id)
        .bind(PASSWORD_HASH)
        .execute(store.pool())
        .await
        .unwrap();
    let entry = external_entry(&store).await;
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    );
    let actor = AuthRepository::load_actor_context_for_user(&store, root.id)
        .await
        .unwrap();
    let identity = canonical(&entry, "reserved", "reserved@example.com");
    kernel
        .bind_external_identity(&actor, identity.clone())
        .await
        .unwrap();
    let binding = store
        .find_external_identity_binding(&identity)
        .await
        .unwrap()
        .unwrap();
    let service = AuthenticationMethodsService::new(store.clone());
    let available = vec![AUTHORITY.to_owned()];
    service
        .unlink(&actor, binding.identity_id, &available)
        .await
        .unwrap();
    let retained = store
        .find_external_identity_binding(&identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.user_id, root.id);
    assert_eq!(retained.state, "unlinked");
    assert!(retained.unlinked_at.is_some());
    assert!(
        AuthRepository::find_user_for_verified_external_identity(&store, &identity)
            .await
            .unwrap()
            .is_none()
    );
    let before: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let error = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity: identity.clone(),
            registration_eligible: true,
        })
        .await
        .err()
        .unwrap();
    assert!(
        error.to_string().contains("external_identity_unlinked"),
        "{error}"
    );
    // The repository's registration transaction also rejects a stale "unknown" result.
    use control_plane::ports::{
        CreateExternalRegisteredMemberInput, ExternalRegistrationRepository,
    };
    let direct = store
        .create_external_registered_member(&CreateExternalRegisteredMemberInput {
            identity: identity.clone(),
            email: "reserved@example.com".into(),
            account: None,
            name: None,
            nickname: None,
            avatar_url: None,
            audit: domain::AuditLogRecord {
                id: Uuid::now_v7(),
                workspace_id: Some(actor.current_workspace_id),
                actor_user_id: None,
                target_type: "user".into(),
                target_id: None,
                event_code: "member.external_registered".into(),
                payload: serde_json::json!({}),
                created_at: time::OffsetDateTime::now_utc(),
            },
        })
        .await
        .unwrap_err();
    assert!(direct.to_string().contains("external_identity_unlinked"));
    let after: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(before, after);
    let (other, _) = external_member(&store, &entry, "other-owner").await;
    let error = kernel
        .bind_external_identity(&other, identity.clone())
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("external_identity_already_bound"));
    kernel
        .bind_external_identity(&actor, identity.clone())
        .await
        .unwrap();
    let restored = store
        .find_external_identity_binding(&identity)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.identity_id, binding.identity_id);
    assert_eq!(restored.state, "bound");
    assert!(restored.unlinked_at.is_none());
    let login = kernel
        .complete_external_login(CompleteExternalLoginCommand {
            login_entry_id: entry.id,
            identity,
            registration_eligible: false,
        })
        .await
        .unwrap();
    assert_eq!(login.actor.user_id, root.id);
    let audits: Vec<serde_json::Value> = sqlx::query_scalar(
        "select payload from audit_logs where target_id=$1 order by created_at,id",
    )
    .bind(binding.identity_id)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        audits.len(),
        3,
        "bind, unlink and explicit restore each commit an audit"
    );
    for payload in &audits {
        assert_eq!(payload["new_owner_user_id"], root.id.to_string());
        assert_eq!(payload["authority_namespace"], AUTHORITY);
        assert_eq!(payload["subject_value"], "reserved");
        assert_eq!(payload["issuer"], "https://github.com");
        assert!(payload.get("old_state").is_some() && payload.get("new_state").is_some());
    }
    assert_eq!(audits[2]["old_state"], "unlinked");
}

#[tokio::test]
async fn methods_view_preserves_dto_order_and_unlink_counterexamples() {
    const OTHER_AUTHORITY: &str = "managed:fixture/alternate:github-alternate";
    let (store, _, _) = seed_store().await;
    let mut a = external_entry(&store).await;
    let mut b = a.clone();
    b.id = Uuid::now_v7();
    b.connection_id = Uuid::now_v7();
    b.auth_type = "github-alternate".into();
    store.upsert_login_entry(&b).await.unwrap();
    let (actor, ia) = external_member(&store, &a, "view-a").await;
    let ib = canonical(&b, "view-b", "view-b@example.com")
        .with_authority_namespace(OTHER_AUTHORITY.into())
        .unwrap();
    let retired = canonical(&a, "view-retired", "view-retired@example.com");
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    );
    for identity in [ib.clone(), retired.clone()] {
        kernel
            .bind_external_identity(&actor, identity)
            .await
            .unwrap();
    }
    let mut ids = Vec::new();
    for identity in [&ia, &ib, &retired] {
        ids.push(
            store
                .find_external_identity_binding(identity)
                .await
                .unwrap()
                .unwrap()
                .identity_id,
        );
    }
    let service = AuthenticationMethodsService::new(store.clone());
    let all = vec![AUTHORITY.to_owned(), OTHER_AUTHORITY.to_owned()];
    // Two subjects sharing one authority are two usable methods, not one.
    let shared = service
        .read(actor.user_id, &[AUTHORITY.to_owned()])
        .await
        .unwrap();
    assert!(!shared.password_usable);
    assert_eq!(shared.bindings.len(), 3);
    assert!(shared.bindings.iter().all(|binding| binding.can_unlink));
    service
        .initialize_password(&actor, ids[0], PASSWORD_HASH, &all)
        .await
        .unwrap();
    service.unlink(&actor, ids[2], &all).await.unwrap();

    // Explicit expected p_i and can_unlink_i for [a, b, retired]. In particular,
    // an unavailable target may be removed when another live method remains;
    // a retained unlinked identity must never count as that alternative.
    let cases = [
        (
            false,
            true,
            true,
            all.clone(),
            [true, true, true],
            [true, true, false],
        ),
        (
            false,
            true,
            true,
            vec![AUTHORITY.to_owned()],
            [true, false, true],
            [false, true, false],
        ),
        (false, true, true, vec![], [false; 3], [false; 3]),
        (
            false,
            true,
            false,
            all.clone(),
            [true, false, true],
            [false, true, false],
        ),
        (
            false,
            false,
            true,
            all.clone(),
            [false, true, false],
            [true, false, false],
        ),
        (false, false, false, all.clone(), [false; 3], [false; 3]),
        (
            false,
            true,
            true,
            vec![AUTHORITY.to_owned(), AUTHORITY.to_owned()],
            [true, false, true],
            [false, true, false],
        ),
        (true, false, false, vec![], [false; 3], [true, true, false]),
    ];
    for (password, a_enabled, b_enabled, available, providers, unlinkable) in cases {
        sqlx::query("update users set password_hash=$2 where id=$1")
            .bind(actor.user_id)
            .bind(if password { PASSWORD_HASH } else { "!" })
            .execute(store.pool())
            .await
            .unwrap();
        a.enabled = a_enabled;
        b.enabled = b_enabled;
        store.upsert_login_entry(&a).await.unwrap();
        store.upsert_login_entry(&b).await.unwrap();
        let snapshot = store
            .authentication_methods_snapshot(actor.user_id)
            .await
            .unwrap();
        let mut expected = Vec::new();
        for binding in snapshot.bindings {
            let index = ids
                .iter()
                .position(|id| *id == binding.identity_id)
                .unwrap();
            let mut dto = serde_json::to_value(&binding).unwrap();
            let fields = dto.as_object_mut().unwrap();
            fields.remove("user_id");
            fields.remove("entry_enabled");
            fields.insert(
                "provider_available".into(),
                serde_json::json!(providers[index]),
            );
            fields.insert("can_unlink".into(), serde_json::json!(unlinkable[index]));
            let reason = if index == 2 {
                Some("external_identity_unlinked")
            } else if !unlinkable[index] {
                Some("last_usable_authentication_method")
            } else {
                None
            };
            fields.insert("unlink_blocked_reason".into(), serde_json::json!(reason));
            expected.push(dto);
        }
        assert_eq!(expected.len(), 3);
        let view = service.read(actor.user_id, &available).await.unwrap();
        assert_eq!(
            serde_json::to_value(view).unwrap(),
            serde_json::json!({
                "password_configured": password,
                "password_usable": password,
                "bindings": expected,
            }),
            "password={password}, entries=({a_enabled},{b_enabled}), authorities={available:?}"
        );
    }
}

#[tokio::test]
async fn parallel_unlinks_keep_one_live_method_and_ignore_sentinel_contact_and_inactive_provider() {
    let (store, _, _) = seed_store().await;
    let a = external_entry(&store).await;
    let mut b = a.clone();
    b.id = Uuid::now_v7();
    b.connection_id = Uuid::now_v7();
    b.auth_type = "github-alternate".into();
    store.upsert_login_entry(&b).await.unwrap();
    let (actor, ia) = external_member(&store, &a, "parallel-a").await;
    let ib = canonical(&b, "parallel-b", "parallel-b@example.com")
        .with_authority_namespace("managed:fixture/alternate:github-alternate".into())
        .unwrap();
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    );
    kernel
        .bind_external_identity(&actor, ib.clone())
        .await
        .unwrap();
    // Profile edits may add a local contact identity without configuring a password.
    sqlx::query("insert into user_auth_identities (id,user_id,connection_id,subject_type,subject_value) values ($1,$2,$3,'email','contact-only@example.com')")
        .bind(Uuid::now_v7()).bind(actor.user_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).execute(store.pool()).await.unwrap();
    let one = store
        .find_external_identity_binding(&ia)
        .await
        .unwrap()
        .unwrap();
    let two = store
        .find_external_identity_binding(&ib)
        .await
        .unwrap()
        .unwrap();
    let service = AuthenticationMethodsService::new(store.clone());
    let available = vec![
        AUTHORITY.to_owned(),
        "managed:fixture/alternate:github-alternate".to_owned(),
    ];
    let offline = service.read(actor.user_id, &[]).await.unwrap();
    assert!(!offline.password_configured && !offline.password_usable);
    assert!(offline.bindings.iter().all(|binding| !binding.can_unlink));
    b.enabled = false;
    store.upsert_login_entry(&b).await.unwrap();
    let error = service
        .unlink(&actor, one.identity_id, &available)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("last_usable_authentication_method"));
    b.enabled = true;
    store.upsert_login_entry(&b).await.unwrap();
    let before = service.read(actor.user_id, &available).await.unwrap();
    assert_eq!(before.bindings.len(), 2);
    assert!(before
        .bindings
        .iter()
        .all(|binding| binding.provider_available
            && binding.can_unlink
            && binding.unlink_blocked_reason.is_none()));
    let (first, second) = tokio::join!(
        service.unlink(&actor, one.identity_id, &available),
        service.unlink(&actor, two.identity_id, &available)
    );
    assert_eq!(
        usize::from(first.is_ok()) + usize::from(second.is_ok()),
        1,
        "only one concurrent unlink may commit"
    );
    let bound: i64 = sqlx::query_scalar("select count(*) from user_auth_identities where user_id=$1 and connection_id<>$2 and state='bound'")
        .bind(actor.user_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).fetch_one(store.pool()).await.unwrap();
    assert_eq!(bound, 1);
    let after = service.read(actor.user_id, &available).await.unwrap();
    assert_eq!(after.bindings.len(), 2);
    for binding in &after.bindings {
        assert!(binding.provider_available && !binding.can_unlink);
        assert_eq!(
            binding.unlink_blocked_reason.as_deref(),
            Some(if binding.state == "bound" {
                "last_usable_authentication_method"
            } else {
                "external_identity_unlinked"
            })
        );
    }
    let error = first.err().or_else(|| second.err()).unwrap();
    assert!(error
        .to_string()
        .contains("last_usable_authentication_method"));
}

#[tokio::test]
async fn initial_password_requires_live_owned_binding_creates_account_and_authenticates() {
    let (store, _, root) = seed_store().await;
    let mut entry = external_entry(&store).await;
    let (actor, identity) = external_member(&store, &entry, "first-password").await;
    let binding = store
        .find_external_identity_binding(&identity)
        .await
        .unwrap()
        .unwrap();
    let service = AuthenticationMethodsService::new(store.clone());
    let available = vec![AUTHORITY.to_owned()];
    let root_actor = AuthRepository::load_actor_context_for_user(&store, root.id)
        .await
        .unwrap();
    let error = service
        .initialize_password(&root_actor, binding.identity_id, PASSWORD_HASH, &available)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("password_initialization_identity_invalid"));
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    );
    let backup = canonical(&entry, "password-backup", "backup@example.com");
    kernel.bind_external_identity(&actor, backup).await.unwrap();
    service
        .unlink(&actor, binding.identity_id, &available)
        .await
        .unwrap();
    let error = service
        .initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &available)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("password_initialization_identity_invalid"),
        "an unlinked proof identity must be rejected"
    );
    kernel
        .bind_external_identity(&actor, identity.clone())
        .await
        .unwrap();
    let error = service
        .initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &[])
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("authentication_provider_unavailable"));
    entry.enabled = false;
    store.upsert_login_entry(&entry).await.unwrap();
    assert!(service
        .initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &available)
        .await
        .is_err());
    entry.enabled = true;
    store.upsert_login_entry(&entry).await.unwrap();
    let (first, second) = tokio::join!(
        service.initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &available),
        service.initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &available)
    );
    assert_eq!(
        usize::from(first.is_ok()) + usize::from(second.is_ok()),
        1,
        "first credential installation is atomic"
    );
    let error = first.err().or_else(|| second.err()).unwrap();
    assert!(
        error.to_string().contains("password_already_configured")
            || error.to_string().contains("authentication_methods_changed")
    );
    let user = AuthRepository::find_user_by_id(&store, actor.user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.session_version, 2);
    let methods = service.read(actor.user_id, &available).await.unwrap();
    assert!(methods.password_configured && methods.password_usable);
    let local: Vec<(String,String)> = sqlx::query_as("select subject_type,subject_value from user_auth_identities where user_id=$1 and connection_id=$2")
        .bind(actor.user_id).bind(domain::PASSWORD_LOCAL_CONNECTION_ID).fetch_all(store.pool()).await.unwrap();
    assert_eq!(local, vec![("account".into(), user.account.clone())]);
    let kernel = AuthKernel::new(
        store.clone(),
        SessionIssuer::new(
            MemorySessionStore::new(format!("auth-methods-{}", Uuid::now_v7())),
            7,
        ),
    );
    let login = kernel
        .login(LoginCommand {
            login_entry_id: domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
            identifier: user.account,
            password: "password".into(),
        })
        .await
        .unwrap();
    assert_eq!(login.actor.user_id, actor.user_id);
    assert_eq!(login.session.session_version, 2);
    let error = service
        .initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &available)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("password_already_configured"));
    service
        .unlink(&actor, binding.identity_id, &available)
        .await
        .unwrap();
    let audits: i64 = sqlx::query_scalar("select count(*) from audit_logs where target_id=$1 and event_code='user.password_initialized'")
        .bind(actor.user_id).fetch_one(store.pool()).await.unwrap();
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn lifecycle_audit_failure_rolls_back_identity_password_and_version() {
    use control_plane::ports::PasswordAvailability;
    let (store, workspace, _) = seed_store().await;
    let entry = external_entry(&store).await;
    let (actor, identity) = external_member(&store, &entry, "atomic-audit").await;
    let binding = store
        .find_external_identity_binding(&identity)
        .await
        .unwrap()
        .unwrap();
    let mut audit = domain::AuditLogRecord {
        id: Uuid::now_v7(),
        workspace_id: Some(workspace.id),
        actor_user_id: Some(actor.user_id),
        target_type: "user".into(),
        target_id: Some(actor.user_id),
        event_code: "user.password_initialized".into(),
        payload: serde_json::json!({}),
        created_at: time::OffsetDateTime::now_utc(),
    };
    AuthRepository::append_audit_log(&store, &audit)
        .await
        .unwrap();
    let available = vec![AUTHORITY.to_owned()];
    let snapshot = store
        .authentication_methods_snapshot(actor.user_id)
        .await
        .unwrap();
    let result = store
        .initialize_password_atomic(
            actor.user_id,
            binding.identity_id,
            PASSWORD_HASH,
            &available,
            PasswordAvailability {
                expected_hash: &snapshot.password_hash,
                hash_usable: false,
            },
            &audit,
        )
        .await;
    assert!(
        result.is_err(),
        "duplicate audit id must reject all credential writes"
    );
    let user = AuthRepository::find_user_by_id(&store, actor.user_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(user.password_hash, snapshot.password_hash);
    assert_eq!(user.session_version, 1);
    assert!(
        !store
            .authentication_methods_snapshot(actor.user_id)
            .await
            .unwrap()
            .local_account_identity
    );
    // A valid password makes unlink possible, but its audit still participates atomically.
    let service = AuthenticationMethodsService::new(store.clone());
    service
        .initialize_password(&actor, binding.identity_id, PASSWORD_HASH, &available)
        .await
        .unwrap();
    audit.event_code = "user.external_identity_unlinked".into();
    let result = store
        .unlink_external_identity_atomic(
            actor.user_id,
            binding.identity_id,
            &available,
            PasswordAvailability {
                expected_hash: PASSWORD_HASH,
                hash_usable: true,
            },
            &audit,
        )
        .await;
    assert!(result.is_err());
    assert_eq!(
        store
            .find_external_identity_binding(&identity)
            .await
            .unwrap()
            .unwrap()
            .state,
        "bound"
    );
    let fresh = canonical(&entry, "bind-audit-rollback", "bind-rollback@example.com");
    let result =
        AuthRepository::bind_verified_external_identity(&store, actor.user_id, &fresh, &audit)
            .await;
    assert!(result.is_err(), "binding and audit must commit together");
    assert!(store
        .find_external_identity_binding(&fresh)
        .await
        .unwrap()
        .is_none());
}
