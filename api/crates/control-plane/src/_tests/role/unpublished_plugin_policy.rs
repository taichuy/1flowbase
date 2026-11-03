use super::*;

#[tokio::test]
async fn stopped_plugin_policy_remains_durable_without_blocking_role_editor() {
    let repository = MemoryRoleRepository::default();
    let service = RoleService::new(repository.clone());
    editable_role(&service, &repository, "policy-editor").await;
    let stopped =
        domain::RoleConsoleGroupPolicy::full(domain::ConsolePolicyGroup::plugin("ssh").unwrap());
    repository
        .replace_role_console_policy(&ReplaceRoleConsolePolicyInput {
            actor_user_id: repository.root_user_id(),
            workspace_id: Uuid::nil(),
            role_code: "policy-editor".into(),
            groups: vec![stopped.clone()],
        })
        .await
        .unwrap();
    let inventory = console_policy_inventory();
    let projected = service
        .get_console_policy(repository.root_user_id(), "policy-editor", &inventory)
        .await
        .unwrap();
    assert_eq!(projected.groups().len(), 3);
    assert!(!projected
        .groups()
        .iter()
        .any(|group| group.group() == stopped.group()));
    let saved = service
        .replace_console_policy(
            ReplaceRoleConsolePolicyCommand {
                actor_user_id: repository.root_user_id(),
                role_code: "policy-editor".into(),
                groups: vec![],
            },
            &inventory,
        )
        .await
        .unwrap();
    assert_eq!(saved.groups().len(), 3);
    let durable = repository
        .get_role_console_policy(Uuid::nil(), "policy-editor")
        .await
        .unwrap();
    assert!(durable.groups().contains(&stopped));
    // Once the plugin's group is registered again its prior selection is available unchanged.
    assert!(durable
        .groups()
        .iter()
        .find(|group| group.group() == stopped.group())
        .unwrap()
        .enabled());
}
