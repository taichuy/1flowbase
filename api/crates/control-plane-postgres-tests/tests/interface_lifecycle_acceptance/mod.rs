//! Root #1998 AC-007. Real service + PostgreSQL transaction, with controlled Kernel
//! registration and a failing protocol writer. No production plugin-loader or network
//! exactly-once claim; test execution is deferred to the candidate-bound CI partitions.
mod contribution_authority;
mod fixture;
use fixture::*;
use interface_runtime::*;
use serde_json::{json, Value};
use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::sync::Notify;

fn assert_completion(
    receipt: &InterfaceInvocationReceipt,
    seen: &Mutex<Vec<InterfaceInvocationTerminal>>,
    terminal: InterfaceInvocationTerminal,
) {
    assert_eq!(receipt.terminal(), terminal);
    assert_eq!(*seen.lock().unwrap(), [terminal]);
    assert_eq!(receipt.observer_records().len(), 1);
    assert_eq!(
        receipt.observer_records()[0].point(),
        InterfaceExtensionPoint::Completion
    );
    assert_eq!(
        receipt.observer_records()[0].status(),
        InterfaceObserverStatus::Executed
    );
}
async fn committed_fact(fixture: &Fixture, code: &str) -> Value {
    let row: (uuid::Uuid, uuid::Uuid, String, String) =
        sqlx::query_as("select id, scope_id, title, status from model_definitions where code=$1")
            .bind(code)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(row.1, fixture.actor.current_workspace_id);
    assert_eq!(row.2, "Root 1998 model");
    assert_eq!(row.3, "published");
    let fact: Value = sqlx::query_scalar("select jsonb_build_object('event_id', o.event_id, 'transaction_id', o.transaction_id, 'contract_id', o.contract_id, 'contract_version', o.contract_version, 'payload', convert_from(o.canonical_payload, 'UTF8')::jsonb, 'status', o.status, 'graph', o.graph_fingerprint, 'delivery_status', d.status, 'attempt_count', d.attempt_count, 'delivered_at', d.delivered_at, 'subscriber', d.subscriber_id) from lifecycle_outbox o join lifecycle_outbox_deliveries d using(event_id)")
        .fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(fact["contract_id"], "model_definition.committed");
    assert_eq!(fact["contract_version"], "v1");
    assert_eq!(fact["graph"], GRAPH);
    assert_eq!(
        fact["payload"]["payload"],
        json!({"model_definition_id":row.0, "scope_kind":"workspace", "scope_id":row.1})
    );
    assert_eq!(fact["payload"]["fact_id"], fact["event_id"]);
    assert_eq!(fact["payload"]["transaction_id"], fact["transaction_id"]);
    assert_eq!(fact["status"], "pending");
    assert_eq!(fact["delivery_status"], "pending");
    assert_eq!(fact["subscriber"], "fixture.subscriber");
    assert_eq!(fact["attempt_count"], 0);
    assert_eq!(fact["delivered_at"], Value::Null);
    fact
}
struct BrokenConnection;
impl Write for BrokenConnection {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "fixture client disconnected",
        ))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[tokio::test]
async fn root_1998_ac_007_commit_and_completion_do_not_ack_or_reverse_failed_delivery() {
    let fixture = Fixture::new().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let snapshot = registry(
        CreateHandler {
            store: fixture.store.clone(),
            hold_after_commit: None,
        },
        seen.clone(),
    );
    let kernel = InterfaceInvocationKernel::new(Arc::new(CoreAuthorization));
    let outcome = kernel
        .invoke::<CreateInput, CreateOutput, CreateError>(
            snapshot,
            fixture.envelope("root_1998_commit"),
        )
        .await
        .unwrap();
    assert_completion(
        outcome.receipt(),
        &seen,
        InterfaceInvocationTerminal::Completed,
    );
    assert_eq!(outcome.value().0.code, "root_1998_commit");
    assert_eq!(outcome.value().0.template_code, "general");
    let before_delivery = committed_fact(&fixture, "root_1998_commit").await;
    let frozen_receipt = outcome.receipt().clone();
    // Actual serde protocol encoding to an I/O writer; only the transport is controlled.
    // Neither the writer nor the Completion observer has an outbox acknowledgement port.
    let error = serde_json::to_writer(BrokenConnection, &outcome.value().0).unwrap_err();
    assert_eq!(error.io_error_kind(), Some(io::ErrorKind::BrokenPipe));
    assert_eq!(outcome.receipt(), &frozen_receipt);
    assert_eq!(
        committed_fact(&fixture, "root_1998_commit").await,
        before_delivery
    );
}
#[tokio::test]
async fn root_1998_ac_007_commit_rejection_rolls_back_model_schema_and_required_fact() {
    let fixture = Fixture::new().await;
    // Formal migrations may seed builtin metadata: preserve it exactly rather than
    // assuming the database starts with zero model/field/grant rows.
    let before_counts: (i64, i64, i64, i64, i64) = sqlx::query_as("select (select count(*) from model_definitions where code='root_1998_rollback'), (select count(*) from model_fields), (select count(*) from lifecycle_outbox), (select count(*) from lifecycle_outbox_deliveries), (select count(*) from scope_data_model_grants)")
        .fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(before_counts.0, 0);
    // The session search_path is the isolated PostgresTestSchema. Fail only our model's
    // required fact, at COMMIT after model/DDL/field/change-log/outbox/delivery inserts.
    // Earlier statement failures intentionally retain existing broken-metadata recovery;
    // this fixture does not modify or claim to cover that separate behavior.
    sqlx::raw_sql(r#"
        CREATE FUNCTION root_1998_reject_commit() RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF EXISTS (SELECT 1 FROM model_definitions WHERE code='root_1998_rollback'
                AND id=(convert_from(NEW.canonical_payload,'UTF8')::jsonb #>> '{payload,model_definition_id}')::uuid) THEN
                IF NOT EXISTS (SELECT 1 FROM lifecycle_outbox_deliveries WHERE event_id=NEW.event_id) THEN
                    RAISE EXCEPTION 'root_1998_missing_delivery_prerequisite';
                END IF;
                RAISE EXCEPTION 'root_1998_commit_rejected' USING ERRCODE='23514';
            END IF;
            RETURN NEW;
        END $$;
        CREATE CONSTRAINT TRIGGER root_1998_commit_failure AFTER INSERT ON lifecycle_outbox
            DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION root_1998_reject_commit();
    "#).execute(fixture.store.pool()).await.unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let snapshot = registry(
        CreateHandler {
            store: fixture.store.clone(),
            hold_after_commit: None,
        },
        seen.clone(),
    );
    let failure = InterfaceInvocationKernel::new(Arc::new(CoreAuthorization))
        .invoke::<CreateInput, CreateOutput, CreateError>(
            snapshot,
            fixture.envelope("root_1998_rollback"),
        )
        .await
        .unwrap_err();
    assert_completion(
        failure.receipt(),
        &seen,
        InterfaceInvocationTerminal::Failed,
    );
    match failure.into_error() {
        InterfaceInvocationError::TargetFailed(error) => {
            assert_eq!(error.classification(), "model_create_failed");
            let service_error = error.into_source::<CreateError>().unwrap();
            assert!(
                service_error.0.contains("root_1998_commit_rejected"),
                "{}",
                service_error.0
            );
        }
        error => panic!("expected actual service commit error: {error}"),
    }
    let counts: (i64,i64,i64,i64,i64) = sqlx::query_as("select (select count(*) from model_definitions where code='root_1998_rollback'), (select count(*) from model_fields), (select count(*) from lifecycle_outbox), (select count(*) from lifecycle_outbox_deliveries), (select count(*) from scope_data_model_grants)")
        .fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(counts, before_counts);
    let physical: bool = sqlx::query_scalar("select to_regclass('root_1998_rollback') is not null")
        .fetch_one(fixture.store.pool())
        .await
        .unwrap();
    assert!(!physical, "transactional DDL must roll back too");
}
#[tokio::test]
async fn root_1998_ac_007_cancelled_invocation_does_not_mean_business_rollback() {
    let fixture = Fixture::new().await;
    let seen = Arc::new(Mutex::new(Vec::new()));
    let committed = Arc::new(Notify::new());
    let snapshot = registry(
        CreateHandler {
            store: fixture.store.clone(),
            hold_after_commit: Some(committed.clone()),
        },
        seen.clone(),
    );
    let envelope = fixture.envelope("root_1998_cancelled");
    let cancellation = envelope.controls().cancellation().clone();
    let kernel = InterfaceInvocationKernel::new(Arc::new(CoreAuthorization));
    let (failure, ()) = tokio::time::timeout(Duration::from_secs(10), async {
        tokio::join!(
            kernel.invoke::<CreateInput, CreateOutput, CreateError>(snapshot, envelope),
            async {
                committed.notified().await;
                cancellation.cancel();
            }
        )
    })
    .await
    .expect("service commit/cancellation barrier must finish");
    let failure = failure.unwrap_err();
    assert!(matches!(
        failure.error(),
        InterfaceInvocationError::Cancelled
    ));
    assert_completion(
        failure.receipt(),
        &seen,
        InterfaceInvocationTerminal::Cancelled,
    );
    committed_fact(&fixture, "root_1998_cancelled").await;
}
