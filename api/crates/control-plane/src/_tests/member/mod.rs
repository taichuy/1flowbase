use crate::_tests::support::{memory_actor_context, MemoryMemberRepository};
use crate::member::{CreateMemberCommand, MemberService};

fn members_policy(operation_ids: &[&str]) -> domain::RoleConsolePolicy {
    let group = domain::ConsolePolicyGroup::settings_feature("system.members")
        .expect("members feature id must be valid");
    domain::RoleConsolePolicy::new(
        uuid::Uuid::now_v7(),
        vec![domain::RoleConsoleGroupPolicy::custom(
            group,
            operation_ids
                .iter()
                .map(|operation_id| {
                    domain::ConsoleOperationPolicy::simple(
                        domain::ConsoleOperationId::try_from(*operation_id)
                            .expect("members operation id must be valid"),
                        true,
                    )
                })
                .collect(),
        )],
    )
}

fn create_member_command(actor_user_id: uuid::Uuid, account: &str) -> CreateMemberCommand {
    CreateMemberCommand {
        actor_user_id,
        account: account.to_string(),
        email: format!("{account}@example.com"),
        phone: Some("13800000000".to_string()),
        password_hash: "hash".to_string(),
        name: account.to_string(),
        nickname: account.to_string(),
        introduction: String::new(),
        email_login_enabled: true,
        phone_login_enabled: false,
    }
}

#[tokio::test]
async fn ac_011_members_policy_only_allows_list_and_create_without_legacy_feature_grant() {
    let repository = MemoryMemberRepository::default();
    let actor = memory_actor_context(false, &[]);
    repository.set_actor_context(actor.clone()).await;
    repository
        .set_console_policies(vec![members_policy(&["members.list", "members.create"])])
        .await;
    let service = MemberService::new(repository.clone());

    assert!(service.list_members(actor.user_id).await.is_ok());
    service
        .create_member(create_member_command(actor.user_id, "policy-member"))
        .await
        .unwrap();
}

#[tokio::test]
async fn ac_011_members_legacy_feature_grant_does_not_authorize_list_or_create() {
    let repository = MemoryMemberRepository::default();
    let actor = memory_actor_context(
        false,
        &[access_control::SYSTEM_MEMBERS_SETTINGS_FEATURE_PERMISSION],
    );
    repository.set_actor_context(actor.clone()).await;
    let service = MemberService::new(repository);

    assert!(service.list_members(actor.user_id).await.is_err());
    assert!(service
        .create_member(create_member_command(actor.user_id, "legacy-member"))
        .await
        .is_err());
}

#[tokio::test]
async fn create_member_assigns_default_member_role_and_records_audit() {
    let repository = MemoryMemberRepository::default();
    let service = MemberService::new(repository.clone());

    service
        .create_member(CreateMemberCommand {
            actor_user_id: repository.root_user_id(),
            account: "member-1".into(),
            email: "member-1@example.com".into(),
            phone: Some("13800000000".into()),
            password_hash: "hash".into(),
            name: "Member 1".into(),
            nickname: "Member 1".into(),
            introduction: String::new(),
            email_login_enabled: true,
            phone_login_enabled: false,
        })
        .await
        .unwrap();

    assert_eq!(repository.created_members().len(), 1);
    assert_eq!(repository.created_members()[0].role_codes, vec!["member"]);
    assert_eq!(repository.audit_events(), vec!["member.created"]);
}

#[tokio::test]
async fn create_member_assigns_current_default_member_role_and_records_audit() {
    let repository = MemoryMemberRepository::with_default_role("qa");
    let service = MemberService::new(repository.clone());

    let created_member = service
        .create_member(CreateMemberCommand {
            actor_user_id: repository.root_user_id(),
            account: "qa-1".into(),
            email: "qa-1@example.com".into(),
            phone: Some("13800000001".into()),
            password_hash: "hash".into(),
            name: "QA 1".into(),
            nickname: "QA 1".into(),
            introduction: String::new(),
            email_login_enabled: true,
            phone_login_enabled: false,
        })
        .await
        .unwrap();

    assert_eq!(repository.created_members().len(), 1);
    assert_eq!(repository.created_members()[0].role_codes, vec!["qa"]);
    assert_eq!(created_member.default_display_role.as_deref(), Some("qa"));
    assert_eq!(repository.audit_events(), vec!["member.created"]);
}
