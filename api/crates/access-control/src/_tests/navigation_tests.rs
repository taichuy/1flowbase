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
    assert!(item_ids.contains(&"frontstage"));
    assert!(item_ids.contains(&"embedded-apps"));
    assert!(item_ids.contains(&"templates"));
    assert!(item_ids.contains(&"settings"));
    assert!(item_ids.contains(&"docs"));
    assert!(item_ids.contains(&"api-key-authentication"));
    assert!(item_ids.contains(&"auth-center"));
    assert!(item_ids.contains(&"system-runtime"));
    assert!(item_ids.contains(&"host-infrastructure"));
    assert!(item_ids.contains(&"memory-observation"));
    assert!(item_ids.contains(&"files"));
    assert!(item_ids.contains(&"data-models"));
    assert!(item_ids.contains(&"model-providers"));
    assert!(item_ids.contains(&"mcp-management"));
    assert!(item_ids.contains(&"members"));
    assert!(item_ids.contains(&"roles"));
}

#[test]
fn user_view_actor_sees_authenticated_routes_and_user_management_items() {
    let actor = scoped_actor(&["user.view.all"]);

    let navigation = accessible_console_navigation(&actor);

    let item_ids = item_ids(&navigation);
    assert!(item_ids.contains(&"frontstage"));
    assert!(item_ids.contains(&"settings"));
    assert!(item_ids.contains(&"api-key-authentication"));
    assert!(item_ids.contains(&"auth-center"));
    assert!(item_ids.contains(&"members"));
    assert!(!item_ids.contains(&"docs"));
    assert!(!item_ids.contains(&"roles"));
    assert!(!item_ids.contains(&"templates"));

    let route_ids = route_ids(&navigation);
    assert!(!route_ids.contains(&"docs"));
    assert!(!route_ids.contains(&"roles"));
    assert!(!route_ids.contains(&"templates"));
}

#[test]
fn route_page_actor_sees_workbench_and_templates() {
    let actor = scoped_actor(&["route_page.view.all"]);

    let navigation = accessible_console_navigation(&actor);

    let item_ids = item_ids(&navigation);
    assert!(item_ids.contains(&"home"));
    assert!(item_ids.contains(&"templates"));
    assert!(item_ids.contains(&"frontstage"));
    assert!(item_ids.contains(&"settings"));
    assert!(item_ids.contains(&"api-key-authentication"));
    assert!(!item_ids.contains(&"docs"));
    assert!(!item_ids.contains(&"roles"));
}
