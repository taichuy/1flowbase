use crate::_tests::support::MemoryAuthRepository;
use crate::profile::{ProfileService, UpdateMeCommand, UpdateMeMetaCommand};
use domain::{BoundRole, RoleScopeKind, UserRecord, UserStatus};
use uuid::Uuid;

fn test_user() -> UserRecord {
    UserRecord {
        id: Uuid::now_v7(),
        account: "root".to_string(),
        email: "root@example.com".to_string(),
        phone: Some("13800000000".to_string()),
        password_hash: "password-hash".to_string(),
        name: "Root".to_string(),
        nickname: "Root".to_string(),
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        introduction: "before".to_string(),
        preferred_locale: None,
        meta: serde_json::json!({}),
        default_display_role: Some("root".to_string()),
        email_login_enabled: true,
        phone_login_enabled: true,
        status: UserStatus::Active,
        session_version: 1,
        roles: vec![BoundRole {
            code: "root".to_string(),
            name: "Root".to_string(),
            scope_kind: RoleScopeKind::System,
            workspace_id: None,
        }],
    }
}

#[tokio::test]
async fn update_me_updates_only_profile_fields() {
    let repository = MemoryAuthRepository::new(test_user());
    let service = ProfileService::new(repository.clone());
    let existing_user = repository.user();

    let profile = service
        .update_me(UpdateMeCommand {
            actor_user_id: existing_user.id,
            tenant_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            name: "Root Next".to_string(),
            nickname: "Captain Root".to_string(),
            email: "root-next@example.com".to_string(),
            phone: Some("13900000000".to_string()),
            avatar_url: Some("https://example.com/next-avatar.png".to_string()),
            introduction: "updated intro".to_string(),
            preferred_locale: None,
        })
        .await
        .unwrap();

    let stored_user = repository.user();
    assert_eq!(stored_user.name, "Root Next");
    assert_eq!(stored_user.nickname, "Captain Root");
    assert_eq!(stored_user.email, "root-next@example.com");
    assert_eq!(stored_user.phone.as_deref(), Some("13900000000"));
    assert_eq!(
        stored_user.avatar_url.as_deref(),
        Some("https://example.com/next-avatar.png")
    );
    assert_eq!(stored_user.introduction, "updated intro");
    assert_eq!(stored_user.account, existing_user.account);
    assert_eq!(stored_user.status, existing_user.status);
    assert_eq!(stored_user.session_version, existing_user.session_version);
    assert_eq!(profile.user.email, "root-next@example.com");
    assert_eq!(profile.user.nickname, "Captain Root");
    assert_eq!(profile.actor.effective_display_role, "root");
}

#[tokio::test]
async fn update_me_persists_preferred_locale() {
    let repository = MemoryAuthRepository::new(test_user());
    let service = ProfileService::new(repository.clone());

    let profile = service
        .update_me(UpdateMeCommand {
            actor_user_id: repository.user().id,
            tenant_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            name: "Root".into(),
            nickname: "Root".into(),
            email: "root@example.com".into(),
            phone: None,
            avatar_url: None,
            introduction: "intro".into(),
            preferred_locale: Some("zh_Hans".into()),
        })
        .await
        .unwrap();

    assert_eq!(profile.user.preferred_locale.as_deref(), Some("zh_Hans"));
}

#[tokio::test]
async fn update_me_meta_merges_nested_preferences_without_replacing_siblings() {
    let mut user = test_user();
    user.meta = serde_json::json!({
        "ui": {
            "data_tables": {
                "applications.logs.runs": {
                    "visibleColumnKeys": ["title"],
                    "columnWidths": {
                        "title": 320
                    }
                }
            }
        }
    });
    let repository = MemoryAuthRepository::new(user);
    let service = ProfileService::new(repository.clone());

    let profile = service
        .update_me_meta(UpdateMeMetaCommand {
            actor_user_id: repository.user().id,
            tenant_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            meta_patch: serde_json::json!({
                "ui": {
                    "data_tables": {
                        "applications.logs.runs": {
                            "columnWidths": {
                                "status": 180
                            }
                        }
                    }
                }
            }),
        })
        .await
        .unwrap();

    assert_eq!(
        profile.user.meta["ui"]["data_tables"]["applications.logs.runs"]["visibleColumnKeys"],
        serde_json::json!(["title"])
    );
    assert_eq!(
        profile.user.meta["ui"]["data_tables"]["applications.logs.runs"]["columnWidths"],
        serde_json::json!({
            "title": 320,
            "status": 180
        })
    );
}
