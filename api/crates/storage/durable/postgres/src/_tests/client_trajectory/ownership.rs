use super::*;
use std::time::Duration;

fn input(
    flow: Uuid,
    node: Option<Uuid>,
    request: Uuid,
    fact: ClientTrajectoryFact,
) -> AppendClientTrajectoryInput {
    AppendClientTrajectoryInput {
        flow_run_id: flow,
        node_run_id: node,
        request_id: request,
        observed_at: AT.into(),
        fact,
    }
}

fn step_fact(flow: Uuid, node: Option<Uuid>, request: Uuid, id: Uuid) -> ClientTrajectoryFact {
    ClientTrajectoryFact::Step {
        step: Box::new(step(flow, node, request, id, "submitted", "request")),
    }
}

fn section_fact(id: Uuid, name: &str) -> ClientTrajectoryFact {
    ClientTrajectoryFact::Section {
        step_id: id,
        section: name.into(),
        value: json!({"observed_at":AT}),
    }
}

// The barrier is an observed PostgreSQL lock wait, not a timing assumption.
// The blocker PID belongs to this test's isolated-schema transaction.
async fn wait_for_flow_lock(pool: &sqlx::PgPool, blocker: i32) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                "select exists(select 1 from pg_stat_activity a where $1=any(pg_blocking_pids(a.pid)) and a.wait_event_type='Lock' and a.query like '%for no key update%')",
            ).bind(blocker).fetch_one(pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("append must reach the controlled flow-lock wait");
}

async fn locked_visibility(section_after_step: bool, single_statement_negative: bool) {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let request = Uuid::now_v7();
    let id = Uuid::now_v7();
    if section_after_step {
        begin(&store, flow, None, request).await;
    }
    let mut blocker = pool.begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("select id from flow_runs where id=$1 for no key update")
        .bind(flow)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    if section_after_step {
        // Use the formal semantic directory shape within the uncommitted lock owner.
        sqlx::query("insert into client_trajectory_steps(id,request_id,flow_run_id,event_sequence,metadata,observed_at,semantic_metadata_restored) values($1,$2,$3,2,$4,$5,true)")
            .bind(id).bind(request).bind(flow).bind(serde_json::to_value(step(flow,None,request,id,"submitted","request")).unwrap())
            .bind(AT).execute(&mut *blocker).await.unwrap();
        sqlx::query("update flow_runs set runtime_event_sequence_high_water=2 where id=$1")
            .bind(flow)
            .execute(&mut *blocker)
            .await
            .unwrap();
    } else {
        // Actual Integrity projection creates the capture only when T1 commits.
        let fact = input(
            flow,
            None,
            request,
            ClientTrajectoryFact::Integrity {
                status: "pending".into(),
                dropped_count: 0,
                persist_failed_count: 0,
            },
        );
        sqlx::query("insert into runtime_events(id,flow_run_id,sequence,event_type,layer,source,trust_level,payload,visibility,durability) values($1,$2,1,'client_protocol_trajectory','runtime_item','host','host_fact',$3,'internal','durable')")
            .bind(Uuid::now_v7()).bind(flow).bind(serde_json::to_value(fact).unwrap())
            .execute(&mut *blocker).await.unwrap();
    }
    let reader_pool = pool.clone();
    let waiter = tokio::spawn(async move {
        if single_statement_negative {
            // Deliberately wrong optimization: locking and owner reads share the
            // statement snapshot taken BEFORE the other writer's commit.
            let (capture_seen, step_seen): (bool, bool) = sqlx::query_as(
                r#"
                with locked as materialized (select id from flow_runs where id=$1 for no key update)
                select c.request_id is not null,s.id is not null from locked l
                left join client_trajectory_captures c on c.flow_run_id=l.id and c.request_id=$2
                left join client_trajectory_steps s on s.flow_run_id=l.id and s.id=$3
            "#,
            )
            .bind(flow)
            .bind(request)
            .bind(id)
            .fetch_one(&reader_pool)
            .await
            .unwrap();
            assert!(
                !(if section_after_step {
                    step_seen
                } else {
                    capture_seen
                }),
                "controlled single-statement negative must miss the newly committed owner"
            );
        } else {
            let fact = if section_after_step {
                section_fact(id, "overview")
            } else {
                step_fact(flow, None, request, id)
            };
            PgControlPlaneStore::new(reader_pool)
                .append_client_trajectory(&input(flow, None, request, fact))
                .await
                .unwrap();
        }
    });
    wait_for_flow_lock(&pool, blocker_pid).await;
    blocker.commit().await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), waiter)
        .await
        .unwrap()
        .unwrap();
    // Prove that the negative missed a real committed owner, rather than bad seeding.
    let committed: bool = sqlx::query_scalar(if section_after_step {
        "select exists(select 1 from client_trajectory_steps where id=$1)"
    } else {
        "select exists(select 1 from client_trajectory_captures where request_id=$1)"
    })
    .bind(if section_after_step { id } else { request })
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(committed);
    if !single_statement_negative {
        let page = store
            .client_trajectory_page(flow, None, None, 10)
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].id, id);
        if section_after_step {
            let section = store
                .client_trajectory_section(flow, None, id, "overview", None, 10)
                .await
                .unwrap()
                .unwrap();
            assert_eq!(section.items[0].value, json!({"observed_at":AT}));
            assert_eq!(section.items[0].sequence, 3);
        } else {
            assert_eq!(page.items[0].sequence, 2);
        }
    }
}

#[tokio::test]
async fn client_trajectory_flow_lock_refreshes_snapshot_for_initial_capture() {
    locked_visibility(false, false).await;
}
#[tokio::test]
async fn client_trajectory_flow_lock_refreshes_snapshot_for_step_then_section() {
    locked_visibility(true, false).await;
}
#[tokio::test]
async fn client_trajectory_single_statement_lock_capture_snapshot_controlled_negative() {
    locked_visibility(false, true).await;
}
#[tokio::test]
async fn client_trajectory_single_statement_lock_step_snapshot_controlled_negative() {
    locked_visibility(true, true).await;
}

async fn snapshot(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar(r#"
        select jsonb_build_object(
            'sequences',(select jsonb_agg(jsonb_build_array(id,runtime_event_sequence_high_water) order by id) from flow_runs),
            'captures',(select jsonb_agg(to_jsonb(c) order by request_id) from client_trajectory_captures c),
            'steps',(select jsonb_agg(to_jsonb(s) order by id) from client_trajectory_steps s),
            'sections',(select jsonb_agg(to_jsonb(s) order by id) from client_trajectory_sections s),
            'events',(select jsonb_agg(to_jsonb(e) order by id) from runtime_events e),
            'heads',(select jsonb_agg(to_jsonb(h) order by request_id) from client_trajectory_archive_heads h))
    "#).fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn client_trajectory_ownership_error_order_and_failed_fact_atomicity() {
    let (pool, flow) = super::super::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = PgControlPlaneStore::new(pool.clone());
    let foreign_flow = Uuid::now_v7();
    sqlx::query("insert into flow_runs(id,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by) select $1,application_id,flow_id,flow_draft_id,compiled_plan_id,run_mode,status,created_by from flow_runs where id=$2")
        .bind(foreign_flow).bind(flow).execute(&pool).await.unwrap();
    let llm = Uuid::now_v7();
    let non_llm = Uuid::now_v7();
    let foreign_node = Uuid::now_v7();
    for (node, owner, kind) in [
        (llm, flow, "llm"),
        (non_llm, flow, "start"),
        (foreign_node, foreign_flow, "llm"),
    ] {
        sqlx::query("insert into node_runs(id,scope_id,flow_run_id,node_id,node_type,node_alias,status) select $1,scope_id,id,$1::text,$3,'fixture','succeeded' from flow_runs where id=$2")
            .bind(node).bind(owner).bind(kind).execute(&pool).await.unwrap();
    }
    let request = Uuid::now_v7();
    let other_request = Uuid::now_v7();
    let node_request = Uuid::now_v7();
    let foreign_request = Uuid::now_v7();
    begin(&store, flow, None, request).await;
    begin(&store, flow, None, other_request).await;
    begin(&store, flow, Some(llm), node_request).await;
    begin(&store, foreign_flow, None, foreign_request).await;
    let own_step = Uuid::now_v7();
    let other_step = Uuid::now_v7();
    let foreign_step = Uuid::now_v7();
    for (owner, capture, id) in [
        (flow, request, own_step),
        (flow, other_request, other_step),
        (foreign_flow, foreign_request, foreign_step),
    ] {
        append(
            &store,
            owner,
            None,
            capture,
            step_fact(owner, None, capture, id),
        )
        .await;
    }
    // A malformed historical directory can reference a valid request on a
    // different flow (the formal schema has independent owner FKs). This
    // isolates the Section flow predicate from its request predicate.
    let wrong_flow_step = Uuid::now_v7();
    sqlx::query("insert into client_trajectory_steps(id,request_id,flow_run_id,event_sequence,metadata,observed_at,semantic_metadata_restored) values($1,$2,$3,100,$4,$5,true)")
        .bind(wrong_flow_step).bind(request).bind(foreign_flow)
        .bind(serde_json::to_value(step(foreign_flow,None,request,wrong_flow_step,"submitted","request")).unwrap())
        .bind(AT).execute(&pool).await.unwrap();
    section(
        &store,
        flow,
        None,
        request,
        own_step,
        "overview",
        json!({"prefix":"committed"}),
    )
    .await;
    let missing = Uuid::now_v7();
    let cases = vec![
        // DTO identity validation still precedes even a missing flow.
        (
            input(
                missing,
                None,
                request,
                step_fact(flow, None, request, own_step),
            ),
            "client trajectory scope mismatch",
        ),
        (
            input(
                missing,
                Some(foreign_node),
                missing,
                section_fact(missing, "invalid"),
            ),
            "client trajectory flow missing",
        ),
        (
            input(
                flow,
                Some(foreign_node),
                missing,
                section_fact(missing, "invalid"),
            ),
            "client trajectory node scope mismatch",
        ),
        (
            input(
                flow,
                None,
                missing,
                ClientTrajectoryFact::NodeLink {
                    node_run_id: non_llm,
                },
            ),
            "client trajectory node scope mismatch",
        ),
        (
            input(
                flow,
                None,
                request,
                ClientTrajectoryFact::NodeLink {
                    node_run_id: foreign_node,
                },
            ),
            "client trajectory node scope mismatch",
        ),
        (
            input(
                flow,
                None,
                request,
                ClientTrajectoryFact::NodeLink {
                    node_run_id: missing,
                },
            ),
            "client trajectory node scope mismatch",
        ),
        (
            input(
                flow,
                None,
                missing,
                step_fact(flow, None, missing, own_step),
            ),
            "client trajectory capture missing",
        ),
        (
            input(flow, None, missing, section_fact(missing, "invalid")),
            "client trajectory capture missing",
        ),
        (
            input(
                flow,
                None,
                foreign_request,
                section_fact(missing, "invalid"),
            ),
            "client trajectory capture scope mismatch",
        ),
        // Both legitimate NULL node and non-NULL capture nodes remain distinct.
        (
            input(flow, Some(llm), request, section_fact(missing, "invalid")),
            "client trajectory capture scope mismatch",
        ),
        (
            input(flow, None, node_request, section_fact(missing, "invalid")),
            "client trajectory capture scope mismatch",
        ),
        (
            input(
                flow,
                None,
                request,
                step_fact(flow, None, request, other_step),
            ),
            "client trajectory step scope mismatch",
        ),
        (
            input(flow, None, request, section_fact(missing, "invalid")),
            "client trajectory section invalid",
        ),
        (
            input(flow, None, request, section_fact(missing, "overview")),
            "client trajectory section scope mismatch",
        ),
        (
            input(flow, None, request, section_fact(other_step, "overview")),
            "client trajectory section scope mismatch",
        ),
        (
            input(
                flow,
                None,
                request,
                section_fact(wrong_flow_step, "overview"),
            ),
            "client trajectory section scope mismatch",
        ),
        (
            input(flow, None, request, section_fact(foreign_step, "overview")),
            "client trajectory section scope mismatch",
        ),
    ];
    for (fact, expected) in cases {
        let before = snapshot(&pool).await;
        let error = store.append_client_trajectory(&fact).await.unwrap_err();
        assert_eq!(error.to_string(), expected, "fact: {fact:?}");
        assert_eq!(
            snapshot(&pool).await,
            before,
            "failed fact must leave sequence and rows unchanged: {expected}"
        );
    }
    // Legitimate suffix after all failures, including capture NULL-node handling.
    append(
        &store,
        flow,
        None,
        request,
        ClientTrajectoryFact::NodeLink { node_run_id: llm },
    )
    .await;
    section(
        &store,
        flow,
        None,
        request,
        own_step,
        "timing",
        json!({"observed_at":AT}),
    )
    .await;
    let prefix = store
        .client_trajectory_section(flow, None, own_step, "overview", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prefix.items.len(), 1);
    assert_eq!(prefix.items[0].value, json!({"prefix":"committed"}));
    let suffix = store
        .client_trajectory_section(flow, None, own_step, "timing", None, 10)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(suffix.items.len(), 1);
    assert_eq!(suffix.items[0].sequence, prefix.items[0].sequence + 2);
    assert_eq!(suffix.items[0].value, json!({"observed_at":AT}));
}
