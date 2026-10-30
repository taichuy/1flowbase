use super::*;
#[tokio::test]
async fn socket_exit_and_mutation_cancel_the_ephemeral_call_and_release_registration() {
    let generation = streams()
        .lock()
        .unwrap()
        .generation
        .clone()
        .unwrap_or_default();
    let installation = uuid::Uuid::now_v7();
    let (lease, mut cancellation) =
        register(installation, epoch(installation), &generation).unwrap();
    assert!(!*cancellation.borrow());
    drop(lease);
    cancellation.changed().await.unwrap();
    assert!(*cancellation.borrow());
    assert!(streams()
        .lock()
        .unwrap()
        .installations
        .get(&installation)
        .unwrap()
        .sockets
        .is_empty());
    let (lease, mut cancellation) =
        register(installation, epoch(installation), &generation).unwrap();
    revoke_installation(installation);
    cancellation.changed().await.unwrap();
    assert!(*cancellation.borrow());
    assert!(register(installation, epoch(installation) - 1, &generation).is_err());
    drop(lease);
}

#[test]
fn client_frames_reject_schema_forgery_unknown_fields_and_oversize_transport() {
    let schema = jsonschema::validator_for(&json!({"type":"object","required":["data"],"properties":{"data":{"type":"string"}},"additionalProperties":false})).unwrap();
    assert!(matches!(
        parse_client_frame(r#"{"type":"input","input":{"data":"x"}}"#, &schema),
        Ok(ClientFrame::Input(_))
    ));
    assert!(matches!(
        parse_client_frame(r#"{"type":"ack","id":0}"#, &schema),
        Ok(ClientFrame::Ack(0))
    ));
    for text in [
        r#"{"type":"input","input":{"data":42}}"#,
        r#"{"type":"input","input":{"data":"x","actor":"forged"}}"#,
        r#"{"type":"ack","id":-1}"#,
        r#"{"type":"ack","id":0,"extra":true}"#,
        r#"{"type":"unknown","id":0}"#,
        "invalid",
    ] {
        assert!(parse_client_frame(text, &schema).is_err(), "{text}");
    }
    assert!(parse_client_frame(&"x".repeat(FRAME_BYTES + 1), &schema).is_err());
}

/// The production session root is separate from both the HTTP body and the completion task.
#[tokio::test]
async fn generation_managed_socket_session_retains_old_owner_until_session_drop() {
    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let owner = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .publish_complete_catalog(&state)
        .unwrap();
    let probe = Arc::downgrade(&owner);
    let state_probe = Arc::downgrade(&state);
    let publisher = crate::managed_publication::ManagedGenerationPublisher::new(
        &state,
        crate::_tests::support::test_config(),
        vec![],
        crate::console_router(state.clone(), true),
    )
    .unwrap();
    let operation: plugin_framework::ManagedServiceOperation = serde_json::from_value(json!({
        "interface_id":"fixture.stream","contribution_id":"fixture.stream","method":"GET",
        "path":"/api/console/managed-services/fixture/stream","summary":"Stream","description":"Stream fixture",
        "input_schema":{"type":"object"},"output_schema":{"type":"object"}
    })).unwrap();
    let (input, _) = mpsc::channel(1);
    let (_, events) = mpsc::channel(1);
    let (_, terminal) = oneshot::channel();
    let (cancel_sender, cancel) = watch::channel(false);
    let session = SocketSession {
        state: state.clone(),
        generation: owner.clone(),
        operation: Arc::new(operation),
        headers: HeaderMap::new(),
        session: String::new(),
        input,
        events,
        terminal,
        cancel,
        lease: SocketLease {
            installation: uuid::Uuid::now_v7(),
            id: uuid::Uuid::now_v7(),
            cancel: cancel_sender,
        },
    };
    drop(owner);
    drop(state);
    publisher.publish(publisher.prepare(vec![], BTreeMap::new()).await.unwrap());
    assert!(probe.upgrade().is_some());
    assert_eq!(
        session.generation.generation_id(),
        session
            .state
            .extension_boot_snapshot
            .as_ref()
            .unwrap()
            .generation_id()
    );
    assert_ne!(
        session.generation.generation_id(),
        publisher
            .current_generation()
            .unwrap()
            .owner
            .generation_id()
    );
    drop(session);
    assert!(probe.upgrade().is_none());
    assert!(state_probe.upgrade().is_none());
}
