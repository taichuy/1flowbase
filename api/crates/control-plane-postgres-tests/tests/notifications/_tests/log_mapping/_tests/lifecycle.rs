use super::*;
use control_plane_contracts::ports::{
    with_lifecycle_workflow_ancestors, CommunicationFactContent, CommunicationLifecycleFact,
    CommunicationLifecycleObserver, CommunicationSendResultKind,
};

#[derive(Default)]
struct Facts(Mutex<Vec<CommunicationLifecycleFact>>);
impl CommunicationLifecycleObserver for Facts {
    fn observed(&self, fact: CommunicationLifecycleFact) {
        self.0.lock().unwrap().push(fact);
    }
}

#[tokio::test]
async fn communication_send_result_actual_provider_outcomes_preserve_reply_and_safe_content() {
    let mut descriptor = descriptor(true);
    descriptor.receive = Some(serde_json::from_value(json!({
        "contribution_id":"acme.log-fixture.receive", "default_system_prompt":"Answer", "mcp_tools":[
            {"name":"history", "description":"Read messages", "input_schema":{"type":"object"}, "operation":"history_messages"},
            {"name":"files", "description":"Read files", "input_schema":{"type":"object"}, "operation":"history_files"},
            {"name":"read", "description":"Read a file", "input_schema":{"type":"object"}, "operation":"read_file"}
        ]
    })).unwrap());
    descriptor.validate().unwrap();
    let mut fixture = Fixture::new(descriptor).await;
    let facts = Arc::new(Facts::default());
    fixture.service = fixture
        .service
        .with_communication_lifecycle_observer(facts.clone());
    let target = extension_contracts::NotificationSendTarget {
        sender_id: "alice".into(),
        conversation_id: "provider-conversation".into(),
        provider_message_id: "original-inbound-key".into(),
    };
    let ancestor = Uuid::now_v7();
    for (outcome, expected) in [
        (
            json!({"status":"succeeded","result":{"receipt":"provider-accepted"}}),
            CommunicationSendResultKind::Accepted,
        ),
        (
            json!({"status":"failed","failure_code":"provider_rejected"}),
            CommunicationSendResultKind::Failed,
        ),
        (
            json!({"status":"unknown","failure_code":"provider_result_unknown"}),
            CommunicationSendResultKind::Unknown,
        ),
    ] {
        fixture.reply(json!({"type":"send", "outcome":outcome}));
        let response = with_lifecycle_workflow_ancestors(
            vec![ancestor],
            fixture.service.send_to(
                &fixture.actor,
                NotificationSource::Workflow,
                fixture.instance_id,
                json!({"body":"Actual reply", "skipped":"unselected-private-input"}),
                target.clone(),
            ),
        )
        .await;
        assert_eq!(
            response.is_ok(),
            expected == CommunicationSendResultKind::Accepted
        );
        let fact = {
            let observed = facts.0.lock().unwrap();
            let CommunicationLifecycleFact::SendResult(fact) = observed.last().unwrap() else {
                panic!("result fact")
            };
            fact.clone()
        };
        assert_eq!(fact.result_kind, expected);
        assert_eq!(fact.target, Some(target.clone()));
        assert_eq!(
            *fixture.provider.target.lock().unwrap(),
            Some(target.clone()),
            "the real provider receives the same immutable reply anchor"
        );
        assert_eq!(fact.workspace_id, fixture.actor.current_workspace_id);
        assert_eq!(fact.instance_id, fixture.instance_id);
        assert_eq!(fact.send_log_id, Some(fixture.latest().await.id));
        assert_eq!(fact.source, Some(NotificationSource::Workflow));
        assert_eq!(fact.workflow_ancestor_ids, vec![ancestor]);
        assert!(
            matches!(&fact.content, Some(CommunicationFactContent::Text { content, .. }) if content == "Actual reply")
        );
        let encoded = serde_json::to_string(&fact).unwrap();
        for secret in [
            CONFIG_SECRET,
            MANAGED_SECRET,
            NEW_SECRET,
            "unselected-private-input",
            "managed_secrets",
            "config_json",
        ] {
            assert!(!encoded.contains(secret), "{secret}");
        }
    }
    assert_eq!(facts.0.lock().unwrap().len(), 3);
    // Generic callers use the same owner even without an immutable reply target.
    fixture.reply(managed_success(json!({"receipt":"generic"}), json!({})));
    fixture.send(json!({"body":"Generic send"})).await.unwrap();
    assert_eq!(facts.0.lock().unwrap().len(), 4);
    let before = fixture.provider.calls.load(Ordering::SeqCst);
    fixture
        .send(json!({"body":CONFIG_SECRET}))
        .await
        .unwrap_err();
    fixture
        .send(json!({"invalid":"no body"}))
        .await
        .unwrap_err();
    let state = fixture
        .store
        .resolve_notification_authorization(
            fixture.actor.current_workspace_id,
            fixture.instance_id,
            MASTER_KEY,
        )
        .await
        .unwrap();
    fixture
        .store
        .save_notification_authorization(SaveNotificationAuthorizationInput {
            workspace_id: fixture.actor.current_workspace_id,
            instance_id: fixture.instance_id,
            expected_revision: state.revision,
            authorization: serde_json::from_value(json!({"status":"unauthorized"})).unwrap(),
            managed_secrets: BTreeMap::new(),
            master_key: MASTER_KEY.into(),
        })
        .await
        .unwrap()
        .unwrap();
    fixture
        .send(json!({"body":"Before provider"}))
        .await
        .unwrap_err();
    assert_eq!(fixture.provider.calls.load(Ordering::SeqCst), before);
    assert_eq!(
        facts.0.lock().unwrap().len(),
        4,
        "preprocessing failures are not provider results"
    );
}

#[tokio::test]
async fn communication_send_legacy_transport_failure_and_unsafe_result_are_unknown() {
    let mut fixture = Fixture::new(descriptor(false)).await;
    let facts = Arc::new(Facts::default());
    fixture.service = fixture
        .service
        .with_communication_lifecycle_observer(facts.clone());
    fixture.send(json!({"body":"Legacy text"})).await.unwrap();
    *fixture.provider.reply.lock().unwrap() = Err("private transport error".into());
    fixture
        .send(json!({"body":"Uncertain submission"}))
        .await
        .unwrap_err();
    fixture.reply(json!({"receipt":CONFIG_SECRET}));
    fixture
        .send(json!({"body":"Unsafe result"}))
        .await
        .unwrap_err();
    let observed = facts.0.lock().unwrap();
    let kinds = observed
        .iter()
        .map(|fact| match fact {
            CommunicationLifecycleFact::SendResult(fact) => fact.result_kind,
            _ => panic!("send"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            CommunicationSendResultKind::Accepted,
            CommunicationSendResultKind::Unknown,
            CommunicationSendResultKind::Unknown
        ]
    );
    for fact in observed.iter() {
        let CommunicationLifecycleFact::SendResult(fact) = fact else {
            unreachable!()
        };
        let encoded = serde_json::to_string(fact).unwrap();
        assert!(!encoded.contains(CONFIG_SECRET));
        assert!(!encoded.contains("private transport error"));
        assert!(fact.target.is_none());
    }
}
