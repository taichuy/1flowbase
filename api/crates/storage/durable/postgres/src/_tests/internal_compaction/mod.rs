use control_plane_contracts::{
    application_public_runtime::{
        ApplicationPublishedRunControlRepository, CancelPublishedFlowRunInput,
    },
    ports::{
        AppendRuntimeEventInput, CommitInternalCompactionInput, InternalCompactionHead,
        ObserveInternalCompactionInput, OrchestrationRuntimeRepository,
    },
};
use serde_json::{json, Value};
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

struct Fixture {
    store: PgControlPlaneStore,
    run: Uuid,
    application_id: Uuid,
    scope_id: Uuid,
}

impl Fixture {
    async fn new() -> Self {
        // Every fixture uses a fresh schema and the complete official migrations.
        let (pool, run) =
            crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
        let (application_id, scope_id) =
            sqlx::query_as("select application_id, scope_id from flow_runs where id=$1")
                .bind(run)
                .fetch_one(&pool)
                .await
                .unwrap();
        Self {
            store: PgControlPlaneStore::new(pool),
            run,
            application_id,
            scope_id,
        }
    }

    fn source(&self, hashes: &[&str]) -> ObserveInternalCompactionInput {
        ObserveInternalCompactionInput {
            scope_id: self.scope_id,
            application_id: self.application_id,
            session_key: "conversation/node".into(),
            source: json!({"messages": hashes, "original": "before\0after"}),
            source_hashes: hashes.iter().map(|value| (*value).into()).collect(),
            prefix_hash: "system-and-tools".into(),
        }
    }

    fn candidate(
        &self,
        hashes: &[&str],
        previous: Option<Uuid>,
        checkpoint: Value,
    ) -> CommitInternalCompactionInput {
        CommitInternalCompactionInput {
            scope_id: self.scope_id,
            application_id: self.application_id,
            flow_run_id: Some(self.run),
            session_key: "conversation/node".into(),
            expected_checkpoint_id: previous,
            prefix_hash: "system-and-tools".into(),
            covered_source_hashes: hashes.iter().map(|value| (*value).into()).collect(),
            checkpoint,
        }
    }

    async fn state(&self) -> (Vec<String>, String, Uuid, Option<Uuid>) {
        sqlx::query_as(
            "select source_hashes,prefix_hash,source_content_id,checkpoint_content_id \
             from runtime_internal_compaction_heads \
             where scope_id=$1 and application_id=$2 and session_key='conversation/node'",
        )
        .bind(self.scope_id)
        .bind(self.application_id)
        .fetch_one(self.store.pool())
        .await
        .unwrap()
    }

    async fn content(&self, id: Uuid) -> Value {
        sqlx::query_scalar(
            "select runtime_original_json(content,raw_json_payloads,'content') \
             from runtime_canonical_contents where id=$1",
        )
        .bind(id)
        .fetch_one(self.store.pool())
        .await
        .unwrap()
    }

    async fn archived(&self, kind: &str) -> Vec<Value> {
        sqlx::query_scalar(
            "select runtime_original_json(c.content,c.raw_json_payloads,'content') \
             from runtime_internal_compaction_contents a \
             join runtime_canonical_contents c on c.id=a.content_id \
             where a.scope_id=$1 and a.application_id=$2 and a.content_kind=$3",
        )
        .bind(self.scope_id)
        .bind(self.application_id)
        .bind(kind)
        .fetch_all(self.store.pool())
        .await
        .unwrap()
    }
}

#[tokio::test]
async fn append_and_stale_observation_preserve_suffix_and_restart_checkpoint() {
    let fixture = Fixture::new().await;
    let original = fixture.source(&["a", "b"]);
    assert_eq!(
        fixture
            .store
            .observe_internal_compaction(&original)
            .await
            .unwrap(),
        InternalCompactionHead::default()
    );
    let latest = fixture.source(&["a", "b", "c"]);
    fixture
        .store
        .observe_internal_compaction(&latest)
        .await
        .unwrap();
    fixture
        .store
        .observe_internal_compaction(&original)
        .await
        .unwrap();
    let checkpoint = json!({"summary":"exact\0summary", "covered_source_hashes":["a","b"]});
    let id = fixture
        .store
        .commit_internal_compaction(&fixture.candidate(&["a", "b"], None, checkpoint.clone()))
        .await
        .unwrap()
        .unwrap();
    let state = fixture.state().await;
    assert_eq!(state.0, vec!["a", "b", "c"]);
    assert_eq!(fixture.content(state.2).await, latest.source);
    assert_eq!(state.3, Some(id));
    assert_eq!(fixture.archived("source").await.len(), 2);

    let recreated = PgControlPlaneStore::new(fixture.store.pool().clone());
    let read = recreated
        .observe_internal_compaction(&latest)
        .await
        .unwrap();
    assert_eq!(read.checkpoint_id, Some(id));
    assert_eq!(read.checkpoint, Some(checkpoint));
    let mut isolated_session = latest.clone();
    isolated_session.session_key = "another-conversation/node".into();
    assert_eq!(
        recreated
            .observe_internal_compaction(&isolated_session)
            .await
            .unwrap(),
        InternalCompactionHead::default()
    );
}

#[tokio::test]
async fn covered_mutation_and_prefix_change_reject_inflight_candidate_and_keep_old_head() {
    let fixture = Fixture::new().await;
    fixture
        .store
        .observe_internal_compaction(&fixture.source(&["a", "b"]))
        .await
        .unwrap();
    let original_checkpoint = json!({"summary":"previous"});
    let previous = fixture
        .store
        .commit_internal_compaction(&fixture.candidate(&["a"], None, original_checkpoint.clone()))
        .await
        .unwrap();
    let candidate = fixture.candidate(&["a", "b"], previous, json!({"summary":"inflight"}));
    for hashes in [vec!["changed", "b"], vec!["a"]] {
        fixture
            .store
            .observe_internal_compaction(&fixture.source(&hashes))
            .await
            .unwrap();
        assert_eq!(
            fixture
                .store
                .commit_internal_compaction(&candidate)
                .await
                .unwrap(),
            None
        );
        assert_eq!(fixture.state().await.3, previous);
    }
    let mut changed_prefix = fixture.source(&["a", "b"]);
    changed_prefix.prefix_hash = "changed-system-or-tools".into();
    fixture
        .store
        .observe_internal_compaction(&changed_prefix)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .store
            .commit_internal_compaction(&candidate)
            .await
            .unwrap(),
        None
    );
    let read = fixture
        .store
        .observe_internal_compaction(&changed_prefix)
        .await
        .unwrap();
    assert_eq!(read.checkpoint_id, previous);
    assert_eq!(read.checkpoint, Some(original_checkpoint));
    assert!(fixture
        .archived("checkpoint")
        .await
        .contains(&candidate.checkpoint));
}

#[tokio::test]
async fn changed_prefix_on_shorter_source_replaces_latest_observation() {
    let fixture = Fixture::new().await;
    fixture
        .store
        .observe_internal_compaction(&fixture.source(&["a", "b", "c"]))
        .await
        .unwrap();
    let candidate = fixture.candidate(&["a", "b"], None, json!({"summary":"old-prefix"}));
    let mut changed = fixture.source(&["a"]);
    changed.prefix_hash = "new-tools".into();
    fixture
        .store
        .observe_internal_compaction(&changed)
        .await
        .unwrap();
    let state = fixture.state().await;
    assert_eq!(state.0, vec!["a"]);
    assert_eq!(state.1, "new-tools");
    assert_eq!(fixture.content(state.2).await, changed.source);
    assert_eq!(
        fixture
            .store
            .commit_internal_compaction(&candidate)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn concurrent_checkpoint_cas_publishes_exactly_one_candidate() {
    let fixture = Fixture::new().await;
    let source = fixture.source(&["a", "b"]);
    fixture
        .store
        .observe_internal_compaction(&source)
        .await
        .unwrap();
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(2));
    let mut tasks = Vec::new();
    for label in ["one", "two"] {
        let store = fixture.store.clone();
        let candidate = fixture.candidate(&["a"], None, json!({"summary": label}));
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let result = store.commit_internal_compaction(&candidate).await.unwrap();
            (result, candidate.checkpoint)
        }));
    }
    let mut outcomes = Vec::new();
    for task in tasks {
        outcomes.push(task.await.unwrap());
    }
    let winners = outcomes
        .iter()
        .filter(|(id, _)| id.is_some())
        .collect::<Vec<_>>();
    assert_eq!(winners.len(), 1);
    let read = fixture
        .store
        .observe_internal_compaction(&source)
        .await
        .unwrap();
    assert_eq!(read.checkpoint_id, winners[0].0);
    assert_eq!(read.checkpoint.as_ref(), Some(&winners[0].1));
    assert_eq!(fixture.archived("checkpoint").await.len(), 2);
}

#[tokio::test]
async fn failed_candidate_storage_and_pointer_publication_keep_previous_state() {
    let fixture = Fixture::new().await;
    fixture
        .store
        .observe_internal_compaction(&fixture.source(&["a", "b"]))
        .await
        .unwrap();
    let previous = fixture
        .store
        .commit_internal_compaction(&fixture.candidate(&["a"], None, json!({"summary":"previous"})))
        .await
        .unwrap();
    let old_state = fixture.state().await;
    sqlx::raw_sql(
        "create function reject_compaction_content() returns trigger language plpgsql as $$ \
         begin if NEW.content->>'fail_store'='true' then raise exception 'controlled candidate storage failure'; \
         end if; return NEW; end $$; \
         create trigger reject_compaction_content before insert on runtime_canonical_contents \
         for each row execute function reject_compaction_content();",
    ).execute(fixture.store.pool()).await.unwrap();
    let failed_storage = fixture.candidate(&["a", "b"], previous, json!({"fail_store":true}));
    assert!(fixture
        .store
        .commit_internal_compaction(&failed_storage)
        .await
        .is_err());
    assert_eq!(fixture.state().await, old_state);
    assert_eq!(fixture.archived("checkpoint").await.len(), 1);

    sqlx::raw_sql(
        "create function reject_compaction_publication() returns trigger language plpgsql as $$ \
         begin raise exception 'controlled candidate publication failure'; end $$; \
         create trigger reject_compaction_publication before update of checkpoint_content_id \
         on runtime_internal_compaction_heads for each row execute function reject_compaction_publication();",
    ).execute(fixture.store.pool()).await.unwrap();
    let failed_publication = fixture.candidate(
        &["a", "b"],
        previous,
        json!({"summary":"durable candidate"}),
    );
    assert!(fixture
        .store
        .commit_internal_compaction(&failed_publication)
        .await
        .is_err());
    assert_eq!(fixture.state().await, old_state);
    assert!(fixture
        .archived("checkpoint")
        .await
        .contains(&failed_publication.checkpoint));
    assert_eq!(
        fixture.content(previous.unwrap()).await,
        json!({"summary":"previous"})
    );
}

#[tokio::test]
async fn cancellation_and_all_terminal_statuses_refuse_publication_but_retain_candidate() {
    let fixture = Fixture::new().await;
    let source = fixture.source(&["a", "b"]);
    fixture
        .store
        .observe_internal_compaction(&source)
        .await
        .unwrap();
    let previous = fixture
        .store
        .commit_internal_compaction(&fixture.candidate(&["a"], None, json!({"summary":"previous"})))
        .await
        .unwrap();
    let old_state = fixture.state().await;
    // Exercise the real terminal owner, whose flow_runs UPDATE serializes with
    // compaction's FOR UPDATE lock before the checkpoint pointer is considered.
    fixture
        .store
        .cancel_published_flow_run(&CancelPublishedFlowRunInput {
            flow_run_id: fixture.run,
            from_status: domain::FlowRunStatus::Running,
            output_payload: json!({}),
            error_payload: None,
            flow_run_event_payload: json!({"reason":"cancel-before-publication"}),
            terminal_event_payload: json!({"reason":"cancel-before-publication"}),
            finished_at: time::OffsetDateTime::now_utc(),
        })
        .await
        .unwrap();
    let status: String = sqlx::query_scalar("select status from flow_runs where id=$1")
        .bind(fixture.run)
        .fetch_one(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(status, "cancelled");
    for status in [
        domain::FlowRunStatus::Cancelled,
        domain::FlowRunStatus::Failed,
        domain::FlowRunStatus::Succeeded,
        domain::FlowRunStatus::Incomplete,
    ] {
        assert!(status.is_terminal());
        if status != domain::FlowRunStatus::Cancelled {
            // Isolated fixture setup for the other terminal-owner outcomes.
            sqlx::query("update flow_runs set status=$2 where id=$1")
                .bind(fixture.run)
                .bind(status.as_str())
                .execute(fixture.store.pool())
                .await
                .unwrap();
        }
        let candidate =
            fixture.candidate(&["a", "b"], previous, json!({"summary":status.as_str()}));
        assert_eq!(
            fixture
                .store
                .commit_internal_compaction(&candidate)
                .await
                .unwrap(),
            None
        );
        assert_eq!(fixture.state().await, old_state);
        assert!(fixture
            .archived("checkpoint")
            .await
            .contains(&candidate.checkpoint));
    }
    let head = fixture
        .store
        .observe_internal_compaction(&source)
        .await
        .unwrap();
    assert_eq!(head.checkpoint_id, previous);
    assert_eq!(head.checkpoint, Some(json!({"summary":"previous"})));
}

#[tokio::test]
async fn empty_coverage_and_missing_run_preserve_head_and_none_keeps_storage_only_semantics() {
    let fixture = Fixture::new().await;
    fixture
        .store
        .observe_internal_compaction(&fixture.source(&["a", "b"]))
        .await
        .unwrap();
    let mut generic_candidate = fixture.candidate(&["a"], None, json!({"summary":"storage-only"}));
    generic_candidate.flow_run_id = None;
    let previous = fixture
        .store
        .commit_internal_compaction(&generic_candidate)
        .await
        .unwrap();
    assert!(previous.is_some());
    let old_state = fixture.state().await;
    let empty_coverage = fixture.candidate(&[], previous, json!({"summary":"empty-coverage"}));
    assert_eq!(
        fixture
            .store
            .commit_internal_compaction(&empty_coverage)
            .await
            .unwrap(),
        None
    );
    assert_eq!(fixture.state().await, old_state);
    let mut missing_run =
        fixture.candidate(&["a", "b"], previous, json!({"summary":"missing-run"}));
    missing_run.flow_run_id = Some(Uuid::now_v7());
    assert_eq!(
        fixture
            .store
            .commit_internal_compaction(&missing_run)
            .await
            .unwrap(),
        None
    );
    assert_eq!(fixture.state().await, old_state);
    let archived = fixture.archived("checkpoint").await;
    assert!(archived.contains(&empty_coverage.checkpoint));
    assert!(archived.contains(&missing_run.checkpoint));
}

#[tokio::test]
async fn original_archive_survives_last_observation_owner_reclamation() {
    let fixture = Fixture::new().await;
    let mut original = fixture.source(&["a"]);
    original.source = json!({"body":"exact\0source"});
    let event = fixture
        .store
        .append_runtime_event(&AppendRuntimeEventInput {
            flow_run_id: fixture.run,
            node_run_id: None,
            span_id: None,
            parent_span_id: None,
            event_type: "provider_semantic_step".into(),
            layer: domain::RuntimeEventLayer::ProviderRaw,
            source: domain::RuntimeEventSource::ProviderPlugin,
            trust_level: domain::RuntimeTrustLevel::ExternalOpaque,
            item_id: None,
            ledger_ref: None,
            payload: json!({"body":original.source,"step_key":"one","source":"ai_native"}),
            visibility: domain::RuntimeEventVisibility::User,
            durability: domain::RuntimeEventDurability::Durable,
        })
        .await
        .unwrap();
    let body_id: Uuid =
        sqlx::query_scalar("select observation_body_content_id from runtime_events where id=$1")
            .bind(event.id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    fixture
        .store
        .observe_internal_compaction(&original)
        .await
        .unwrap();
    assert_eq!(fixture.state().await.2, body_id);
    // Advance the source too: the archived original remains owned after both live
    // pointers (event and session source) have stopped referencing it.
    fixture
        .store
        .observe_internal_compaction(&fixture.source(&["a", "b"]))
        .await
        .unwrap();
    sqlx::query("delete from runtime_events where id=$1")
        .bind(event.id)
        .execute(fixture.store.pool())
        .await
        .unwrap();
    assert_eq!(fixture.content(body_id).await, original.source);
    assert_eq!(fixture.archived("source").await.len(), 2);
}
