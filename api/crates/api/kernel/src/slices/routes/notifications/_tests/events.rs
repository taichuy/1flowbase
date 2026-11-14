use super::*;
use control_plane::notifications::NotificationEventPublisher;

#[tokio::test]
async fn refresh_signals_coalesce_and_remain_workspace_recipient_scoped() {
    let hub = Arc::new(NotificationEventHub::default());
    let workspace = Uuid::now_v7();
    let user = Uuid::now_v7();
    let mut own = hub.subscribe(workspace, user);
    let other_user = hub.subscribe(workspace, Uuid::now_v7());
    let other_workspace = hub.subscribe(Uuid::now_v7(), user);
    for _ in 0..100 {
        hub.publish(workspace, &[user]);
    }
    assert!(own.receiver.has_changed().unwrap());
    own.receiver.changed().await.unwrap();
    assert!(!own.receiver.has_changed().unwrap());
    assert!(!other_user.receiver.has_changed().unwrap());
    assert!(!other_workspace.receiver.has_changed().unwrap());
    drop(own);
    assert!(!hub
        .channels
        .lock()
        .unwrap()
        .contains_key(&(workspace, user)));
    drop(other_user);
    drop(other_workspace);
    assert!(hub.channels.lock().unwrap().is_empty());
}

#[test]
fn dropping_one_browser_keeps_the_other_subscription_registered() {
    let hub = Arc::new(NotificationEventHub::default());
    let key = (Uuid::now_v7(), Uuid::now_v7());
    let first = hub.subscribe(key.0, key.1);
    let second = hub.subscribe(key.0, key.1);
    drop(first);
    assert!(hub.channels.lock().unwrap().contains_key(&key));
    hub.publish(key.0, &[key.1]);
    assert!(second.receiver.has_changed().unwrap());
    drop(second);
    assert!(hub.channels.lock().unwrap().is_empty());
}
