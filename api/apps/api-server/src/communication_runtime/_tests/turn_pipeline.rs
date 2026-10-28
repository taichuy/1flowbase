//! Real host + PostgreSQL + SDK notification worker + local model-provider processes.
//! File barriers are emitted inside actual model invocations, after real MCP tool results.
use super::pipeline::{api, execution_diagnostics};
use super::*;
use crate::_tests::{application, support};
use axum::http::StatusCode;
use control_plane::ports::{
    CommitCommunicationReceiveInput, CommunicationReceiveMessage, NotificationLogListInput,
    OrchestrationRuntimeRepository, SaveNotificationAuthorizationInput,
};
use domain::{
    CommunicationConversationScope, CommunicationReplyTurn, CommunicationSendStatus,
    CommunicationTurnState, FlowRunStatus, NodeRunStatus,
};
use futures_util::FutureExt;
use std::{future::Future, panic::AssertUnwindSafe};

const NEW_REPLY: &str =
    "The previous conversation has stopped generating. A new conversation has started.";
const TOOL_IDS: [&str; 3] = ["turn_list", "turn_get", "turn_call"];

async fn eventually<T, F, Fut>(description: &str, mut probe: F) -> T
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Option<T>>,
{
    tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Some(value) = probe().await {
                return value;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for {description}"))
}

struct ProviderControl {
    root: PathBuf,
}
impl ProviderControl {
    fn new() -> Self {
        let root = crate::api_workspace_root()
            .unwrap()
            .parent()
            .unwrap()
            .join("tmp/test-governance/communications-session-commands-20261028")
            .join(format!("turn-provider-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("block-final-1"), "block").unwrap();
        Self { root }
    }
    async fn final_ready(&self, round: usize) -> Value {
        let path = self.root.join(format!("final-{round}.ready.json"));
        eventually("actual provider final-round barrier", || async {
            std::fs::read(&path)
                .ok()
                .map(|bytes| serde_json::from_slice(&bytes).unwrap())
        })
        .await
    }
    fn release(&self, round: usize) {
        std::fs::write(self.root.join(format!("release-final-{round}")), "release").unwrap();
    }
    fn release_all(&self) {
        std::fs::write(self.root.join("release-all"), "release").unwrap();
    }
    fn completed(&self, round: usize) -> bool {
        self.root.join(format!("final-{round}.completed")).exists()
    }
    fn invocations(&self) -> Vec<Value> {
        std::fs::read_to_string(self.root.join("invocations.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
}
impl Drop for ProviderControl {
    fn drop(&mut self) {
        let _ = std::fs::write(self.root.join("release-all"), "release");
    }
}

struct Workers {
    stop: watch::Sender<bool>,
    tasks: Vec<tokio::task::JoinHandle<()>>,
}
impl Workers {
    fn new() -> Self {
        Self {
            stop: watch::channel(false).0,
            tasks: Vec::new(),
        }
    }
    fn admit(&mut self, f: &TurnFixture) {
        self.tasks.push(tokio::spawn(super::super::commands::run(
            f.fixture.state.clone(),
            f.key(),
            self.stop.subscribe(),
        )));
    }
    fn execute(&mut self, f: &TurnFixture) {
        self.tasks.push(tokio::spawn(super::super::turns::run(
            f.fixture.state.clone(),
            f.key(),
            self.stop.subscribe(),
        )));
    }
    async fn stop(&mut self) -> bool {
        self.stop.send_replace(true);
        let mut clean = true;
        for mut task in self.tasks.drain(..) {
            match tokio::time::timeout(Duration::from_secs(10), &mut task).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => clean = false,
                Err(_) => {
                    clean = false;
                    task.abort();
                    let _ = task.await;
                }
            }
        }
        clean
    }
}
impl Drop for Workers {
    fn drop(&mut self) {
        self.stop.send_replace(true);
        for task in &self.tasks {
            task.abort();
        }
    }
}

struct TurnFixture {
    fixture: Fixture,
    application_id: Uuid,
    cookie: String,
    control: ProviderControl,
    scope: CommunicationConversationScope,
}
impl TurnFixture {
    async fn new(expected_sends: &[(&str, &str)]) -> Self {
        let expected_sends = expected_sends
            .iter()
            .map(|(receipt, text)| {
                json!({
                    "sender_id":"alice","conversation_id":"external-room",
                    "provider_message_id":receipt,"text":text,
                })
            })
            .collect::<Vec<_>>();
        let fixture = Fixture::with_expected_sends(Some(json!(expected_sends))).await;
        fixture.receive().await;
        // Retain Bob's history for the real delegated-scope check, but isolate this run's
        // execution lane through the normal explicit admission exclusion operation.
        let pending = fixture
            .state
            .notification_service
            .scan_pending_admissions(fixture.actor.current_workspace_id, fixture.channel, 100)
            .await
            .unwrap();
        let mut scope = None;
        for input in pending {
            let conversation_scope = CommunicationConversationScope {
                workspace_id: fixture.actor.current_workspace_id,
                instance_id: fixture.channel,
                external_identity: input.conversation.external_identity,
                conversation_id: input.conversation.id,
            };
            if conversation_scope.external_identity == "bob" {
                assert!(fixture
                    .state
                    .notification_service
                    .exclude_input(&conversation_scope, input.input_id)
                    .await
                    .unwrap());
            } else {
                assert_eq!(conversation_scope.external_identity, "alice");
                scope = Some(conversation_scope);
            }
        }
        let (cookie, csrf) =
            support::login_and_capture_cookie(&fixture.app, "root", "change-me").await;
        let (status, granted) = api(&fixture.app, &cookie, &csrf, "POST",
            &format!("/api/console/settings/billing/credits/{}/grant", fixture.actor.user_id),
            json!({"amount":"100","reason":"communication turn fixture","source_type":"test", "source_id":"communication-turn", "idempotency_key":"communication-turn"}),
        ).await;
        assert_eq!(status, StatusCode::OK, "{granted}");
        let control = ProviderControl::new();
        let provider = application::create_turn_communication_provider_instance(
            &fixture.app,
            &cookie,
            &csrf,
            &control.root,
        )
        .await;
        let application =
            application::seed_agent_flow_application(&fixture.app, &cookie, &csrf, &provider).await;
        let mut mapping =
            control_plane::application_public_api::mapping::ApplicationApiMappingConfig::default_native();
        // The fixture's fixed model is intentional. Both authenticated projections must
        // expose the resulting /model unavailability while retaining the saved toggles.
        mapping.input.model_target = None;
        let (status, published) = api(
            &fixture.app,
            &cookie,
            &csrf,
            "POST",
            &format!("/api/console/applications/{application}/api-publications"),
            json!({"mapping":mapping,"api_enabled":true}),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED, "{published}");
        let (status, saved) = api(&fixture.app, &cookie, &csrf, "PUT",
            &format!("/api/console/communications/instances/{}/binding", fixture.channel),
            json!({"application_id":application,"mcp_instance_ids":[fixture.mcp],
                "system_prompt":"Use this sender's scoped history. Never replay prior user inputs automatically.",
                "model":null,"reasoning_effort":null,"file_table_id":null,
                "command_settings":[{"command_id":"host/help","enabled":false}]}),
        ).await;
        assert_eq!(status, StatusCode::OK, "{saved}");
        Self {
            fixture,
            application_id: application.parse().unwrap(),
            cookie,
            control,
            scope: scope.unwrap(),
        }
    }
    fn key(&self) -> (Uuid, Uuid) {
        (self.scope.workspace_id, self.scope.instance_id)
    }

    async fn receive(&self, receipt: &str, text: &str) -> Uuid {
        let state = &self.fixture.state;
        let auth = state
            .store
            .resolve_notification_authorization(
                self.key().0,
                self.key().1,
                &state.provider_secret_master_key,
            )
            .await
            .unwrap();
        state
            .notification_service
            .commit_receive(CommitCommunicationReceiveInput {
                authorization: SaveNotificationAuthorizationInput {
                    workspace_id: self.key().0,
                    instance_id: self.key().1,
                    expected_revision: auth.revision,
                    authorization: auth.authorization.unwrap_or_else(|| {
                        serde_json::from_value(json!({"status":"authorized"})).unwrap()
                    }),
                    managed_secrets: auth.managed_secrets,
                    master_key: state.provider_secret_master_key.clone(),
                },
                candidate_cursor: Some(json!("accepted-cursor")),
                messages: vec![CommunicationReceiveMessage {
                    provider_message_id: receipt.into(),
                    conversation_id: "external-room".into(),
                    sender_id: "alice".into(),
                    sender_display_name: None,
                    received_at_ms: 1000,
                    text: Some(text.into()),
                    attachments: vec![],
                }],
            })
            .await
            .unwrap()
            .expect("synthetic input must pass the real receive CAS");
        sqlx::query_scalar("SELECT id FROM communication_inputs WHERE instance_id=$1 AND external_identity='alice' AND external_message_key=$2")
            .bind(self.key().1).bind(receipt).fetch_one(state.store.pool()).await.unwrap()
    }

    async fn admitted(&self, receipt: &str) -> (Uuid, i64) {
        eventually("ordinary input admission", || async {
            sqlx::query_as("SELECT turn_id,turn_sequence FROM communication_inputs WHERE instance_id=$1 AND external_identity='alice' AND external_message_key=$2 AND admission_status='admitted'")
                .bind(self.key().1).bind(receipt).fetch_optional(self.fixture.state.store.pool()).await.unwrap()
        }).await
    }
    async fn turn(&self, id: Uuid) -> (CommunicationReplyTurn, Option<Value>) {
        let (record, checkpoint): (Value, Option<Value>) = sqlx::query_as(
            "SELECT to_jsonb(t),checkpoint_json FROM communication_reply_turns t WHERE id=$1",
        )
        .bind(id)
        .fetch_one(self.fixture.state.store.pool())
        .await
        .unwrap();
        (serde_json::from_value(record).unwrap(), checkpoint)
    }
    async fn sent(&self, id: Uuid) -> CommunicationReplyTurn {
        eventually("one durable turn send", || async {
            let (turn, _) = self.turn(id).await;
            if turn.state == CommunicationTurnState::Failed
                || matches!(
                    turn.send_status,
                    CommunicationSendStatus::Failed | CommunicationSendStatus::Unknown
                )
            {
                let diagnostic = match turn.run_id {
                    Some(run) => {
                        execution_diagnostics(&self.fixture, &self.application_id.to_string(), run)
                            .await
                    }
                    None => json!("no run"),
                };
                panic!("turn failed: {turn:?}; {diagnostic}");
            }
            (turn.send_status == CommunicationSendStatus::Succeeded).then_some(turn)
        })
        .await
    }
    async fn outbound_for(&self, turn: &CommunicationReplyTurn) -> Vec<String> {
        sqlx::query_scalar("SELECT content FROM communication_messages WHERE input_id=$1 AND direction='outbound' AND type='text' ORDER BY id")
            .bind(turn.anchor_input_id).fetch_all(self.fixture.state.store.pool()).await.unwrap()
    }
    async fn assert_successful_deliveries(&self, count: usize) {
        let logs = self
            .fixture
            .state
            .notification_service
            .logs(
                &self.fixture.actor,
                NotificationLogListInput {
                    instance_id: Some(self.key().1),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(
            logs.items.len(),
            count,
            "no hidden stale/provisional send attempts"
        );
        for log in logs.items {
            assert_eq!(log.status, "success");
            assert_eq!(log.actor_user_id, self.fixture.actor.user_id);
            assert!(log.content.is_none());
        }
        // Each success passed the real SDK worker's exact sender/chat/receipt/text list.
    }
    async fn assert_command_routes(&self) {
        let (status, binding) = api(
            &self.fixture.app,
            &self.cookie,
            "",
            "GET",
            &format!(
                "/api/console/communications/instances/{}/binding",
                self.key().1
            ),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{binding}");
        assert_eq!(
            binding["data"]["binding"]["command_settings"],
            json!([{"command_id":"host/help","enabled":false}])
        );
        let (status, options) = api(
            &self.fixture.app,
            &self.cookie,
            "",
            "GET",
            &format!(
                "/api/console/communications/options?application_id={}&instance_id={}",
                self.application_id,
                self.key().1
            ),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{options}");
        for payload in [&binding, &options] {
            let commands = payload["data"]["commands"].as_array().unwrap();
            let command = |id| {
                commands
                    .iter()
                    .find(|entry| entry["command_id"] == id)
                    .unwrap()
            };
            assert_eq!(command("host/help")["enabled"], false);
            assert_eq!(command("host/new")["enabled"], true);
            assert_eq!(command("host/new")["available"], true);
            assert_eq!(command("host/model")["available"], false);
            assert_eq!(
                command("host/model")["unavailable_reason"],
                "model_selection_unavailable"
            );
        }
        assert_eq!(binding["data"]["commands"], options["data"]["commands"]);
    }
}

fn assert_provider_inputs(captured: &Value, expected: &[&str]) {
    assert_eq!(captured["user_messages"], json!(expected));
    assert_eq!(captured["session"]["user_id"], "alice");
    assert_eq!(
        captured["session"]["external_conversation_key"],
        "external-room"
    );
    let tools = captured["tool_results"].as_array().unwrap();
    assert_eq!(tools.len(), 3);
    for (tool, id) in tools.iter().zip(TOOL_IDS) {
        assert_eq!(tool["tool_call_id"], id);
        assert_eq!(tool["is_error"], false);
    }
    assert!(tools[2]["content"]
        .as_str()
        .unwrap()
        .contains("Alice private input"));
    assert!(!tools[2]["content"]
        .as_str()
        .unwrap()
        .contains("Bob private input"));
}

#[tokio::test]
async fn communication_turn_merges_before_and_during_generation_without_repeating_completed_tools()
{
    let f = TurnFixture::new(&[
        ("alice-input", "Final for alice"),
        ("alice-next", "Final for alice"),
    ])
    .await;
    let mut workers = Workers::new();
    let outcome = AssertUnwindSafe(async {
        f.assert_command_routes().await;
        f.receive("alice-before", "Before generation input").await;
        workers.admit(&f);
        let (turn_id, first_sequence) = f.admitted("alice-input").await;
        assert_eq!(first_sequence, 1);
        assert_eq!(f.admitted("alice-before").await, (turn_id, 2));
        assert!(f.turn(turn_id).await.0.run_id.is_none());
        workers.execute(&f);

        let blocked = f.control.final_ready(1).await;
        assert_provider_inputs(&blocked, &["Alice private input", "Before generation input"]);
        assert!(!f.control.completed(1), "provider must still be inside its barrier");
        let (before, checkpoint) = f.turn(turn_id).await;
        assert_eq!((before.received_watermark, before.consumed_watermark), (2, 2));
        assert!(f.outbound_for(&before).await.is_empty());
        let run_id = before.run_id.unwrap();
        let llm_receipt: Uuid = serde_json::from_value(checkpoint.unwrap()["node_receipts"]["node-llm"].clone()).unwrap();
        let running = f.fixture.state.store.get_application_run_detail(f.application_id, run_id).await.unwrap().unwrap();
        let llm = running.node_runs.iter().find(|node| node.id == llm_receipt).unwrap();
        assert_eq!(llm.status, NodeRunStatus::Running);
        assert!(llm.finished_at.is_none());

        f.receive("alice-during", "During generation input").await;
        assert_eq!(f.admitted("alice-during").await, (turn_id, 3));
        let (pending, _) = f.turn(turn_id).await;
        assert_eq!(pending.run_id, Some(run_id));
        assert_eq!((pending.received_watermark, pending.consumed_watermark), (3, 2));
        assert!(f.outbound_for(&pending).await.is_empty());
        f.control.release(1);

        let complete = f.sent(turn_id).await;
        assert_eq!(complete.state, CommunicationTurnState::Sealed);
        assert_eq!(complete.run_id, Some(run_id));
        assert_eq!((complete.received_watermark, complete.consumed_watermark), (3, 3));
        assert_eq!(f.outbound_for(&complete).await, vec!["Final for alice"]);
        let final_request = f.control.final_ready(2).await;
        assert_provider_inputs(&final_request, &["Alice private input", "Before generation input", "During generation input"]);
        assert_eq!(final_request["session"], blocked["session"]);
        let invocations = f.control.invocations();
        assert_eq!(invocations.iter().map(|v| v["stage"].as_str().unwrap()).collect::<Vec<_>>(), vec!["turn_list", "turn_get", "turn_call", "final", "final"]);

        let detail = f.fixture.state.store.get_application_run_detail(f.application_id, run_id).await.unwrap().unwrap();
        assert_eq!(detail.flow_run.status, FlowRunStatus::Succeeded);
        assert_eq!(detail.flow_run.input_payload["sys"]["provider_session_identity"], complete.session_id.to_string());
        assert_eq!(detail.flow_run.output_payload["answer"], "Final for alice");
        let llms = detail.node_runs.iter().filter(|node| node.node_type == "llm").collect::<Vec<_>>();
        assert_eq!(llms.len(), 1, "continuation reuses the durable LLM receipt");
        assert_eq!(llms[0].id, llm_receipt);
        assert_eq!(llms[0].status, NodeRunStatus::Succeeded);
        assert_eq!(llms[0].metrics_payload["internal_tool_call_count"], 3);
        let tools = llms[0].debug_payload["runtime_internal_tool_events"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        for (event, id) in tools.iter().zip(TOOL_IDS) {
            assert_eq!(event["tool_call_id"], id);
            assert_eq!(event["execution_status"], "succeeded");
        }
        // This fixture has established the application's own run. Stitched traces contain
        // prior runs only; the canonical repository reader hydrates this run's payloads.
        let runtime_events = f.fixture.state.store.list_runtime_events(run_id, 0).await.unwrap();
        let completed_tools = runtime_events.iter().filter(|event| event.event_type == "assistant_tool_call_finished").map(|event| {
            assert_eq!(event.flow_run_id, run_id);
            assert_eq!(event.node_run_id, Some(llm_receipt));
            event.payload.pointer("/tool_call/id").and_then(Value::as_str).unwrap()
        }).collect::<Vec<_>>();
        assert_eq!(completed_tools, TOOL_IDS, "each completed tool has exactly one canonical runtime receipt");
        assert_eq!(runtime_events.iter().filter(|event| event.event_type == "runtime_stream_opened").count(), 1, "same-turn continuation reuses one event generation");
        // Every member resolves the canonical run through its scoped turn; only the
        // anchor input carries the legacy unique run_id association.
        let memberships: Vec<(Uuid, i64, Uuid, String)> = sqlx::query_as("SELECT m.turn_id,m.turn_sequence,rt.run_id,m.processing_status FROM communication_inputs m JOIN communication_reply_turns rt ON rt.id=m.turn_id AND rt.scope_id=m.scope_id AND rt.instance_id=m.instance_id AND rt.external_identity=m.external_identity AND rt.conversation_id=m.conversation_id WHERE m.instance_id=$1 AND m.external_identity='alice' ORDER BY m.id")
            .bind(f.key().1).fetch_all(f.fixture.state.store.pool()).await.unwrap();
        assert_eq!(memberships, vec![(turn_id,1,run_id,"succeeded".into()),(turn_id,2,run_id,"succeeded".into()),(turn_id,3,run_id,"succeeded".into())]);
        f.assert_successful_deliveries(1).await;

        f.receive("alice-next", "Next turn only").await;
        let (next_id, sequence) = f.admitted("alice-next").await;
        assert_ne!(next_id, turn_id);
        assert_eq!(sequence, 1);
        let next = f.sent(next_id).await;
        assert_eq!(next.session_id, complete.session_id);
        assert_ne!(next.run_id, complete.run_id);
        let next_request = f.control.final_ready(3).await;
        assert_provider_inputs(&next_request, &["Next turn only"]);
        assert_eq!(next_request["session"], blocked["session"]);
        let next_run = f.fixture.state.store.get_flow_run(f.application_id, next.run_id.unwrap()).await.unwrap().unwrap();
        assert_eq!(next_run.input_payload["sys"]["provider_session_identity"], complete.session_id.to_string());
        assert_eq!(f.outbound_for(&next).await, vec!["Final for alice"]);
        f.assert_successful_deliveries(2).await;
    }).catch_unwind().await;
    f.control.release_all();
    let workers_stopped = workers.stop().await;
    let host_stopped = f.fixture.runtime.host.stop().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(workers_stopped, "all fixture workers joined");
    host_stopped.unwrap();
}

#[tokio::test]
async fn communication_new_cancels_an_actual_blocked_provider_before_ack_and_never_sends_old_reply()
{
    // An old alice-input send is deliberately absent from the real SDK allow-list.
    let f = TurnFixture::new(&[
        ("alice-new", NEW_REPLY),
        ("alice-after-new", "Final for alice"),
    ])
    .await;
    let mut workers = Workers::new();
    let outcome = AssertUnwindSafe(async {
        workers.admit(&f);
        let (old_id, _) = f.admitted("alice-input").await;
        workers.execute(&f);
        let blocked = f.control.final_ready(1).await;
        assert_provider_inputs(&blocked, &["Alice private input"]);
        assert!(!f.control.completed(1));
        let old = f.turn(old_id).await.0;
        let old_run = old.run_id.unwrap();
        assert!(f.outbound_for(&old).await.is_empty());
        #[cfg(target_os = "linux")]
        let provider_process = {
            let path = PathBuf::from(format!("/proc/{}", blocked["pid"].as_u64().unwrap()));
            assert!(
                path.exists(),
                "the model provider process is actually blocked"
            );
            path
        };

        let command_input = f.receive("alice-new", "/new").await;
        let command = eventually("new-session acknowledgement", || async {
            let command = f
                .fixture
                .state
                .notification_service
                .command_for_input(&f.scope, command_input)
                .await
                .unwrap()?;
            if matches!(
                command.send_status,
                CommunicationSendStatus::Failed | CommunicationSendStatus::Unknown
            ) {
                panic!("new command failed: {command:?}");
            }
            (command.send_status == CommunicationSendStatus::Succeeded).then_some(command)
        })
        .await;
        assert_eq!(command.reply.as_ref().unwrap().text, NEW_REPLY);
        assert_eq!(command.failure_code, None);
        let active = f
            .fixture
            .state
            .notification_service
            .active_session(&f.scope)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(active.id, old.session_id);
        assert_eq!(command.response_session_id, Some(active.id));
        assert_eq!(active.conversation_id, old.conversation_id);
        assert_eq!(
            f.turn(old_id).await.0.state,
            CommunicationTurnState::Invalidated
        );
        let cancelled = f
            .fixture
            .state
            .store
            .get_flow_run(f.application_id, old_run)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            cancelled.status,
            FlowRunStatus::Cancelled,
            "ack follows joined cancellation"
        );
        #[cfg(target_os = "linux")]
        eventually(
            "cancelled provider process exit while its release is still closed",
            || async { (!provider_process.exists()).then_some(()) },
        )
        .await;
        assert!(
            !f.control.completed(1),
            "cancelled provider must never produce its candidate"
        );
        assert!(f.outbound_for(&old).await.is_empty());
        f.assert_successful_deliveries(1).await;

        // Opening the obsolete gate cannot revive the cancelled turn. A new ordinary
        // request must run under the rotated identity and use its own exact destination.
        f.control.release(1);
        f.receive("alice-after-new", "Fresh session only").await;
        let (fresh_id, sequence) = f.admitted("alice-after-new").await;
        assert_eq!(sequence, 1);
        let fresh = f.sent(fresh_id).await;
        assert_eq!(fresh.session_id, active.id);
        assert_ne!(fresh.run_id, Some(old_run));
        let request = f.control.final_ready(2).await;
        assert_provider_inputs(&request, &["Fresh session only"]);
        assert_ne!(
            request["session"]["session_id"],
            blocked["session"]["session_id"]
        );
        assert_eq!(
            request["session"]["conversation_id"],
            blocked["session"]["conversation_id"]
        );
        let run = f
            .fixture
            .state
            .store
            .get_flow_run(f.application_id, fresh.run_id.unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            run.input_payload["sys"]["provider_session_identity"],
            active.id.to_string()
        );
        assert!(f.outbound_for(&old).await.is_empty());
        assert_eq!(f.outbound_for(&fresh).await, vec!["Final for alice"]);
        assert!(!f.control.completed(1));
        f.assert_successful_deliveries(2).await;
    })
    .catch_unwind()
    .await;
    f.control.release_all();
    let workers_stopped = workers.stop().await;
    let host_stopped = f.fixture.runtime.host.stop().await;
    if let Err(panic) = outcome {
        std::panic::resume_unwind(panic);
    }
    assert!(workers_stopped, "all fixture workers joined");
    host_stopped.unwrap();
}
