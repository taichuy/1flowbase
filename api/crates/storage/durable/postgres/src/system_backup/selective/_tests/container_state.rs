use super::*;
use control_plane_contracts::ports::{
    CommitContainerTransitionInput, ContainerCommitAdmission, ContainerControlFailureFact,
    ContainerNodeFact, ContainerObservationInput, ContainerRoundFact,
    OrchestrationRuntimeRepository,
};
use serde_json::json;

mod identity;

const TABLES: [&str; 5] = [
    "flow_container_checkpoints",
    "flow_container_executions",
    "flow_container_rounds",
    "flow_container_control_failures",
    "flow_container_observations",
];

async fn container_rows(db: &PgPool) -> BTreeMap<String, Vec<serde_json::Value>> {
    let mut rows = BTreeMap::new();
    for table in TABLES {
        let query = format!("select to_jsonb(t) from {table} t order by to_jsonb(t)::text");
        rows.insert(
            table.to_owned(),
            sqlx::query_scalar(sqlx::AssertSqlSafe(query.as_str()))
                .fetch_all(db)
                .await
                .unwrap(),
        );
    }
    rows
}

#[tokio::test]
async fn selective_container_execution_data_roundtrips_and_configuration_excludes_it() {
    // Reuse the formal migration fixture and the real atomic state writer.
    let (db, run) = crate::_tests::provider_protocol_capsule_store_tests::seeded_flow_run().await;
    let store = crate::PgControlPlaneStore::new(db.clone());
    let execution = Uuid::now_v7();
    let mut transition = CommitContainerTransitionInput {
        flow_run_id: run,
        resume_claim_owner: None,
        execution_identity: domain::NodeExecutionIdentity {
            execution_id: execution,
            node_id: "loop".into(),
            parent_execution_id: None,
            round_path: vec![],
        },
        checkpoint: json!({"version":1,"phase":"started"}),
        admission: ContainerCommitAdmission::AdmitWork {
            effective_deadline: None,
        },
        node: Some(ContainerNodeFact::Started {
            node_type: "loop".into(),
            node_alias: "Backup container".into(),
            input_payload: json!({}),
        }),
        rounds: vec![],
        control_failure: None,
        observation: None,
    };
    store
        .commit_container_transition(&transition)
        .await
        .unwrap();
    transition.admission = ContainerCommitAdmission::ClosureOrObservation;
    transition.checkpoint =
        json!({"version":1,"phase":"condition_failed","text":"raw\u{0000}value"});
    transition.node = Some(ContainerNodeFact::Updated {
        status: domain::NodeRunStatus::Failed,
        output_payload: json!({"result":["value"]}),
        error_payload: Some(json!({"code":"condition_value_invalid"})),
        metrics_payload: json!({}),
        debug_payload: json!({}),
        terminal: true,
    });
    transition.rounds = vec![ContainerRoundFact {
        execution_id: Uuid::now_v7(),
        container_execution_id: execution,
        round_index: 0,
        deadline_unix_ms: 2_000_000_000_000,
        status: "succeeded".into(),
        error_payload: None,
        output_payload: Some(json!({"text":"value\u{0000}suffix"})),
        variables: Some(json!({"total":1})),
    }];
    transition.control_failure = Some(ContainerControlFailureFact {
        after_round_index: 0,
        error_payload: json!({"code":"condition_value_invalid"}),
    });
    transition.observation = Some(ContainerObservationInput {
        observation_key: "terminal".into(),
        trace_payload: json!({"text":"trace\u{0000}value"}),
    });
    sqlx::query("alter table flow_container_observations alter column sequence restart with 10001")
        .execute(&db)
        .await
        .unwrap();
    store
        .commit_container_transition(&transition)
        .await
        .unwrap();
    let original = container_rows(&db).await;
    assert!(original.values().all(|rows| rows.len() == 1));
    let repo = PgSelectiveBackupRepository::new(db.clone());
    let categories = repo.catalog().await.unwrap();
    let applications = categories
        .iter()
        .find(|entry| entry.feature_id == "system.applications")
        .unwrap();
    for table in TABLES {
        assert!(applications.data_tables.contains(&table.to_owned()));
        assert!(!applications.structure_tables.contains(&table.to_owned()));
        assert!(!inventory::excluded(table));
    }

    let configuration = capture(&repo, select("applications", true, false)).await;
    let data = capture(&repo, select("applications", false, true)).await;
    for (bytes, includes_data) in [(&configuration, false), (&data, true)] {
        let scratch = archive::unpack(reader(bytes.clone())).await.unwrap();
        let header = archive::header(&mut scratch.reader().await.unwrap())
            .await
            .unwrap();
        for table in TABLES {
            let entry = header.tables.iter().find(|entry| entry.name == table);
            assert_eq!(entry.is_some(), includes_data, "{table}");
            if let Some(entry) = entry {
                assert_eq!(entry.feature_id, "system.applications");
                assert!(entry.data);
            }
        }
    }
    // Deleting the execution exercises both composite-FK children on restore;
    // checkpoint and round heads have their own run FKs.
    for table in [
        "flow_container_executions",
        "flow_container_checkpoints",
        "flow_container_rounds",
    ] {
        let query = format!("delete from {table} where flow_run_id=$1");
        sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
            .bind(run)
            .execute(&db)
            .await
            .unwrap();
    }
    let empty = container_rows(&db).await;
    assert!(empty.values().all(Vec::is_empty));
    // A destination allocator may lag far behind the identities in an archive.
    sqlx::query("alter table flow_container_observations alter column sequence restart with 1")
        .execute(&db)
        .await
        .unwrap();
    repo.restore(reader(configuration), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(container_rows(&db).await, empty);
    let preview = repo
        .preflight(reader(data.clone()), "source", "target")
        .await
        .unwrap();
    assert!(preview.failures.is_empty(), "{:?}", preview.failures);
    for table in TABLES {
        assert!(preview.selected_tables.contains(&table.to_owned()));
    }
    let pending = repo
        .prepare_restore(reader(data.clone()), "source", "target", true)
        .await
        .unwrap();
    pending.rollback().await.unwrap();
    assert_eq!(container_rows(&db).await, empty);
    repo.restore(reader(data.clone()), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(container_rows(&db).await, original);
    // Updating an existing PK must restore the original identity, too.
    sqlx::query(
        "alter table flow_container_observations alter column sequence set generated by default",
    )
    .execute(&db)
    .await
    .unwrap();
    sqlx::query("update flow_container_observations set sequence=7")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query(
        "alter table flow_container_observations alter column sequence set generated always",
    )
    .execute(&db)
    .await
    .unwrap();
    // Preserve observation identity sequences, timestamps and lossless JSON on replay.
    repo.restore(reader(data), "source", "target", true)
        .await
        .unwrap();
    assert_eq!(container_rows(&db).await, original);
    // Append an observation through the real writer without re-closing a terminal node.
    transition.node = None;
    transition.rounds.clear();
    transition.control_failure = None;
    transition.observation.as_mut().unwrap().observation_key = "after-restore".into();
    store
        .commit_container_transition(&transition)
        .await
        .unwrap();
    let next: i64 = sqlx::query_scalar(
        "select sequence from flow_container_observations where observation_key='after-restore'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert!(
        next > 10001,
        "restored observations must not collide with new facts"
    );
}
