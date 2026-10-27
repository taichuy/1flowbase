use super::resolve_move_placement;
use domain::frontstage::FrontstageNavigationPlacement::{Sidebar, Topbar};
use uuid::Uuid;

#[test]
fn root_destination_promotes_sidebar_nodes_without_an_anchor() {
    assert_eq!(
        resolve_move_placement(Sidebar, None, Some(Topbar), None).unwrap(),
        Topbar
    );
    assert_eq!(
        resolve_move_placement(Sidebar, None, Some(Topbar), Some(Topbar)).unwrap(),
        Topbar
    );
}

#[test]
fn explicit_destination_rejects_conflicting_anchor_or_parent() {
    assert!(resolve_move_placement(Sidebar, None, Some(Topbar), Some(Sidebar)).is_err());
    assert!(resolve_move_placement(Topbar, None, Some(Sidebar), Some(Topbar)).is_err());
    assert!(resolve_move_placement(Sidebar, Some(Uuid::nil()), Some(Topbar), None).is_err());
}

#[test]
fn nested_moves_demote_topbar_nodes_and_unspecified_root_moves_preserve_semantics() {
    assert_eq!(
        resolve_move_placement(Topbar, Some(Uuid::nil()), None, None).unwrap(),
        Sidebar
    );
    assert_eq!(
        resolve_move_placement(Sidebar, None, None, Some(Topbar)).unwrap(),
        Topbar
    );
    assert_eq!(
        resolve_move_placement(Sidebar, None, None, None).unwrap(),
        Sidebar
    );
    assert_eq!(
        resolve_move_placement(Topbar, None, Some(Sidebar), None).unwrap(),
        Sidebar
    );
}
