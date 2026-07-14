use crate::_tests::support::MemoryMemberRepository;
use crate::member::{CreateMemberCommand, MemberService};

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
