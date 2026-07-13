use access_control::accessible_console_navigation;
use domain::ActorContext;

fn item_ids(navigation: &access_control::ConsoleNavigation) -> Vec<&str> {
    navigation
        .navigation_items
        .iter()
        .map(|item| item.item_id.as_str())
        .collect()
}

fn route_ids(navigation: &access_control::ConsoleNavigation) -> Vec<&str> {
    navigation
        .route_definitions
        .iter()
        .map(|route| route.route_id.as_str())
        .collect()
}

fn scoped_actor(permission_codes: &[&str]) -> ActorContext {
    ActorContext::scoped(
        Default::default(),
        Default::default(),
        "Member",
        permission_codes.iter().map(|code| (*code).to_string()),
    )
}

#[test]
fn root_console_navigation_sees_all_builtin_items() {
    let actor = ActorContext::root(Default::default(), Default::default(), "Root");

    let navigation = accessible_console_navigation(&actor);

    let item_ids = item_ids(&navigation);
    assert_eq!(navigation.route_definitions.len(), 17);
    assert_eq!(navigation.navigation_items.len(), 17);
    assert_eq!(navigation.permission_bindings.len(), 17);
    assert!(item_ids.contains(&"home"));
    assert!(item_ids.contains(&"embedded-apps"));
    assert!(item_ids.contains(&"templates"));
    assert!(item_ids.contains(&"settings"));
    assert!(item_ids.contains(&"settings.docs"));
    assert!(item_ids.contains(&"settings.api-key-authentication"));
    assert!(item_ids.contains(&"settings.auth-center"));
    assert!(item_ids.contains(&"settings.system-runtime"));
    assert!(item_ids.contains(&"settings.host-infrastructure"));
    assert!(item_ids.contains(&"settings.memory-observation"));
    assert!(item_ids.contains(&"settings.files"));
    assert!(item_ids.contains(&"settings.data-models"));
    assert!(item_ids.contains(&"settings.model-providers"));
    assert!(item_ids.contains(&"settings.mcp-management"));
    assert!(item_ids.contains(&"settings.members"));
    assert!(item_ids.contains(&"settings.roles"));
}

#[test]
fn settings_members_route_actor_sees_only_members_settings_entries() {
    let actor = scoped_actor(&["settings_feature.access.system.members"]);

    let navigation = accessible_console_navigation(&actor);

    let item_ids = item_ids(&navigation);
    assert!(item_ids.contains(&"home"));
    assert!(item_ids.contains(&"templates"));
    assert!(item_ids.contains(&"settings"));
    assert!(item_ids.contains(&"settings.members"));
    assert!(!item_ids.contains(&"settings.docs"));
    assert!(!item_ids.contains(&"settings.roles"));

    let route_ids = route_ids(&navigation);
    assert!(!route_ids.contains(&"settings.docs"));
    assert!(!route_ids.contains(&"settings.roles"));
}

#[test]
fn authenticated_actor_sees_workbench_and_templates_without_route_page_permission() {
    let actor = scoped_actor(&[]);

    let navigation = accessible_console_navigation(&actor);

    let item_ids = item_ids(&navigation);
    assert!(item_ids.contains(&"home"));
    assert!(item_ids.contains(&"templates"));
    assert!(!item_ids.contains(&"settings"));
    assert!(!item_ids.contains(&"settings.api-key-authentication"));
    assert!(!item_ids.contains(&"settings.docs"));
    assert!(!item_ids.contains(&"settings.roles"));
}
