use super::*;

async fn seed() -> (PgControlPlaneStore, Uuid, Uuid) {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool);
    let request = Uuid::now_v7();
    store
        .append_client_trajectory_archive(&AppendClientTrajectoryArchiveInput {
            request_id: request,
            part_id: Uuid::now_v7(),
            transport: ClientTrajectoryTransport::Http,
            frames: vec![ClientTrajectoryArchiveFrame {
                sequence: 0,
                kind: ClientTrajectoryFrameKind::Request,
                observed_at: AT.into(),
                bytes: b"{\"model\":\"fixed\",\"input\":[]}".to_vec(),
            }],
        })
        .await
        .unwrap();
    (store, flow, request)
}
async fn head(store: &PgControlPlaneStore, request: Uuid) -> (Option<Uuid>, String) {
    sqlx::query_as(
        "select flow_run_id,ctid::text from client_trajectory_archive_heads where request_id=$1",
    )
    .bind(request)
    .fetch_one(store.pool())
    .await
    .unwrap()
}
fn input(flow: Uuid, request: Uuid, fact: ClientTrajectoryFact) -> AppendClientTrajectoryInput {
    AppendClientTrajectoryInput {
        flow_run_id: flow,
        node_run_id: None,
        request_id: request,
        observed_at: AT.into(),
        fact,
    }
}
fn root(flow: Uuid, request: Uuid) -> ClientTrajectoryFact {
    ClientTrajectoryFact::Step {
        step: Box::new(step(flow, None, request, request, "submitted", "request")),
    }
}

#[tokio::test]
async fn archive_head_binds_once_without_rewriting_already_bound_tuple() {
    let (store, flow, request) = seed().await;
    let unbound = head(&store, request).await;
    assert_eq!(unbound.0, None);
    begin(&store, flow, None, request).await;
    let bound = head(&store, request).await;
    assert_eq!(bound.0, Some(flow));
    assert_ne!(bound.1, unbound.1, "first binding updates the head");
    append(&store, flow, None, request, root(flow, request)).await;
    for _ in 0..8 {
        section(
            &store,
            flow,
            None,
            request,
            request,
            "timing",
            json!({"observed_at":AT}),
        )
        .await;
        assert_eq!(
            head(&store, request).await,
            bound,
            "bound appends must not create replacement tuples"
        );
    }
    let result = store
        .client_trajectory_section(flow, None, request, "timing", None, 32)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        result.items.len(),
        8,
        "identical values remain distinct occurrences"
    );
    assert_eq!(
        result
            .items
            .iter()
            .map(|item| item.sequence)
            .collect::<Vec<_>>(),
        (3..=10).collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn bound_archive_head_mutex_is_held_and_does_not_exclude_fk_reader() {
    let (store, flow, request) = seed().await;
    begin(&store, flow, None, request).await;
    let before = head(&store, request).await;
    let mut holder = store.pool().begin().await.unwrap();
    let holder_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    sqlx::query("select request_id from client_trajectory_archive_heads where request_id=$1 for no key update")
        .bind(request).fetch_one(&mut *holder).await.unwrap();
    let mut reader = store.pool().begin().await.unwrap();
    sqlx::query(
        "select request_id from client_trajectory_archive_heads where request_id=$1 for key share",
    )
    .bind(request)
    .fetch_one(&mut *reader)
    .await
    .unwrap();
    let writer_store = store.clone();
    let writer = tokio::spawn(async move {
        writer_store
            .append_client_trajectory(&input(flow, request, root(flow, request)))
            .await
    });
    let blocked = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if writer.is_finished() {
                return false;
            }
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))",
            )
            .bind(holder_pid)
            .fetch_one(store.pool())
            .await
            .unwrap();
            if waiting {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    // Release the writer blocker even when the witness assertion failed.
    holder.rollback().await.unwrap();
    let finished = tokio::time::timeout(std::time::Duration::from_secs(10), writer).await;
    reader.rollback().await.unwrap();
    assert!(
        blocked.unwrap(),
        "binding must still wait on the already-bound head mutex"
    );
    finished
        .expect("NO KEY UPDATE must coexist with a KEY SHARE reader")
        .unwrap()
        .unwrap();
    assert_eq!(head(&store, request).await, before);
}

#[tokio::test]
async fn head_binding_does_not_rebind_foreign_scope_or_create_missing_head() {
    let (store, flow, request) = seed().await;
    begin(&store, flow, None, request).await;
    // Another real run in the same formally isolated fixture, using its formal domain keys.
    let foreign = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by from flow_runs where id=$2")
        .bind(foreign).bind(flow).execute(store.pool()).await.unwrap();
    sqlx::query("update client_trajectory_archive_heads set flow_run_id=$2 where request_id=$1")
        .bind(request)
        .bind(foreign)
        .execute(store.pool())
        .await
        .unwrap();
    let before = head(&store, request).await;
    append(&store, flow, None, request, root(flow, request)).await;
    assert_eq!(
        head(&store, request).await,
        before,
        "same omission policy as the original UPDATE predicate"
    );
    let missing = Uuid::now_v7();
    begin(&store, flow, None, missing).await;
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from client_trajectory_archive_heads where request_id=$1)",
    )
    .bind(missing)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert!(
        !exists,
        "observational append must not invent an archive head"
    );
}

#[tokio::test]
async fn every_section_failure_preserves_successful_facts_raw_access_and_sequence_reuse() {
    let names = [
        "overview",
        "parameters",
        "result",
        "schema",
        "timing",
        "usage",
    ];
    for failed_section in names {
        let (store, flow, request) = seed().await;
        begin(&store, flow, None, request).await;
        let mut root = step(flow, None, request, request, "submitted", "request");
        root.available_sections = names
            .iter()
            .map(|name| (*name).to_owned())
            .chain(["raw".into()])
            .collect();
        append(
            &store,
            flow,
            None,
            request,
            ClientTrajectoryFact::Step {
                step: Box::new(root),
            },
        )
        .await;
        let before = head(&store, request).await;
        // Fault only one later INSERT; the fixture/database remains usable for the suffix.
        sqlx::query(&format!("alter table client_trajectory_sections add constraint section_failure_fixture check (section <> '{failed_section}')"))
            .execute(store.pool()).await.unwrap();
        let mut failed = 0;
        let mut expected_sequence = 3;
        for name in names {
            let value = json!({"section":name,"exact":"北京\0🌍","optional":null,"enabled":true});
            let result = store
                .append_client_trajectory(&input(
                    flow,
                    request,
                    ClientTrajectoryFact::Section {
                        step_id: request,
                        section: name.into(),
                        value: value.clone(),
                    },
                ))
                .await;
            if name == failed_section {
                assert!(result.is_err());
                failed += 1;
            } else {
                result.unwrap();
                let section = store
                    .client_trajectory_section(flow, None, request, name, None, 32)
                    .await
                    .unwrap()
                    .unwrap();
                assert_eq!(section.items.len(), 1);
                assert_eq!(section.items[0].value, value);
                assert_eq!(section.items[0].sequence, expected_sequence);
                expected_sequence += 1;
            }
        }
        assert_eq!(failed, 1);
        append(
            &store,
            flow,
            None,
            request,
            ClientTrajectoryFact::Integrity {
                status: "incomplete".into(),
                dropped_count: 0,
                persist_failed_count: failed,
            },
        )
        .await;
        let page = store
            .client_trajectory_page(flow, None, None, 100)
            .await
            .unwrap();
        assert_eq!(page.integrity, "incomplete");
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, request);
        assert!(store
            .client_trajectory_section(flow, None, request, "raw", None, 32)
            .await
            .unwrap()
            .is_some());
        assert_eq!(
            store
                .read_client_trajectory_archive(request, 0, 32)
                .await
                .unwrap()[0]
                .bytes,
            b"{\"model\":\"fixed\",\"input\":[]}"
        );
        let high: i64 = sqlx::query_scalar(
            "select runtime_event_sequence_high_water from flow_runs where id=$1",
        )
        .bind(flow)
        .fetch_one(store.pool())
        .await
        .unwrap();
        assert_eq!(
            high, 8,
            "failed fact reuses its slot; terminal integrity consumes one"
        );
        assert_eq!(head(&store, request).await, before);
    }
}

#[tokio::test]
async fn deferred_commit_rejection_retains_earlier_facts_and_accepts_suffix() {
    let (store, flow, request) = seed().await;
    begin(&store, flow, None, request).await;
    append(&store, flow, None, request, root(flow, request)).await;
    section(
        &store,
        flow,
        None,
        request,
        request,
        "overview",
        json!({"exact":"committed 北京\0🌍"}),
    )
    .await;
    let before = head(&store, request).await;
    sqlx::query(r#"create function head_binding_reject_commit() returns trigger language plpgsql as $$
        begin if new.section='schema' then raise exception 'fixture deferred section rejection' using errcode='23514'; end if; return new; end $$"#)
        .execute(store.pool()).await.unwrap();
    sqlx::query("create constraint trigger head_binding_commit_failure after insert on client_trajectory_sections deferrable initially deferred for each row execute function head_binding_reject_commit()")
        .execute(store.pool()).await.unwrap();
    let error = store
        .append_client_trajectory(&input(
            flow,
            request,
            ClientTrajectoryFact::Section {
                step_id: request,
                section: "schema".into(),
                value: json!({"const":"commit rejected"}),
            },
        ))
        .await
        .unwrap_err();
    let db_error = error
        .downcast_ref::<sqlx::Error>()
        .unwrap()
        .as_database_error()
        .unwrap();
    assert_eq!(
        db_error.code().as_deref(),
        Some("23514"),
        "explicit deferred rejection is not a lost COMMIT acknowledgment"
    );
    section(
        &store,
        flow,
        None,
        request,
        request,
        "timing",
        json!({"observed_at":AT}),
    )
    .await;
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::Integrity {
            status: "incomplete".into(),
            dropped_count: 0,
            persist_failed_count: 1,
        },
    )
    .await;
    let page = store
        .client_trajectory_page(flow, None, None, 100)
        .await
        .unwrap();
    assert_eq!(page.integrity, "incomplete");
    assert_eq!(page.items.len(), 1);
    let overview = store
        .client_trajectory_section(flow, None, request, "overview", None, 32)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        overview.items[0].value,
        json!({"exact":"committed 北京\0🌍"})
    );
    assert_eq!(overview.items[0].sequence, 3);
    let timing = store
        .client_trajectory_section(flow, None, request, "timing", None, 32)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(timing.items[0].sequence, 4);
    assert!(store
        .client_trajectory_section(flow, None, request, "schema", None, 32)
        .await
        .unwrap()
        .unwrap()
        .items
        .is_empty());
    let high: i64 =
        sqlx::query_scalar("select runtime_event_sequence_high_water from flow_runs where id=$1")
            .bind(flow)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(high, 5);
    assert_eq!(head(&store, request).await, before);
}

#[tokio::test]
async fn interrupted_fact_preserves_committed_prefix_and_reuses_rolled_back_slot() {
    let (store, flow, request) = seed().await;
    begin(&store, flow, None, request).await;
    append(&store, flow, None, request, root(flow, request)).await;
    section(
        &store,
        flow,
        None,
        request,
        request,
        "overview",
        json!({"exact":"prefix 北京\0🌍"}),
    )
    .await;
    let before = head(&store, request).await;
    let mut holder = store.pool().begin().await.unwrap();
    let holder_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *holder)
        .await
        .unwrap();
    sqlx::query("select request_id from client_trajectory_archive_heads where request_id=$1 for no key update")
        .bind(request).fetch_one(&mut *holder).await.unwrap();
    let writer_store = store.clone();
    let writer = tokio::spawn(async move {
        writer_store
            .append_client_trajectory(&input(
                flow,
                request,
                ClientTrajectoryFact::Section {
                    step_id: request,
                    section: "parameters".into(),
                    value: json!({"exact":"interrupted body"}),
                },
            ))
            .await
    });
    let blocked = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            if writer.is_finished() {
                return false;
            }
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity where $1=any(pg_blocking_pids(pid)))",
            )
            .bind(holder_pid)
            .fetch_one(store.pool())
            .await
            .unwrap();
            if waiting {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    writer.abort();
    let interrupted = writer.await;
    holder.rollback().await.unwrap();
    assert!(
        blocked.unwrap(),
        "interrupt only while the fact is demonstrably before COMMIT"
    );
    assert!(interrupted.unwrap_err().is_cancelled());
    // This acquisition waits for the previous transaction's queued rollback before reuse.
    section(
        &store,
        flow,
        None,
        request,
        request,
        "timing",
        json!({"observed_at":AT}),
    )
    .await;
    let page = store
        .client_trajectory_page(flow, None, None, 100)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, request);
    assert_eq!(
        store
            .client_trajectory_section(flow, None, request, "overview", None, 32)
            .await
            .unwrap()
            .unwrap()
            .items[0]
            .sequence,
        3
    );
    assert_eq!(
        store
            .client_trajectory_section(flow, None, request, "timing", None, 32)
            .await
            .unwrap()
            .unwrap()
            .items[0]
            .sequence,
        4
    );
    assert!(store
        .client_trajectory_section(flow, None, request, "parameters", None, 32)
        .await
        .unwrap()
        .unwrap()
        .items
        .is_empty());
    assert!(store
        .client_trajectory_section(flow, None, request, "raw", None, 32)
        .await
        .unwrap()
        .is_some());
    assert_eq!(head(&store, request).await, before);
}
