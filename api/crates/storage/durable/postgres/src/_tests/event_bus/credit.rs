use super::*;

#[tokio::test]
async fn credit_handoff_is_atomic_snapshot_and_does_not_replay_after_poll() {
    let (bus, _, workspace) = fixture().await;
    let (id, expected) = source(&bus, workspace, false).await;
    assert_eq!(bus.handoff_credit_events(50).await.unwrap(), 1);
    let row = sqlx::query("select p.payload,p.credit_event_id,h.origin,h.cutover_id,c.published_at from host_event_bus_pending p join credit_event_handoffs h on h.event_id=p.credit_event_id join credit_event_outbox c on c.event_id=h.event_id")
        .fetch_one(&bus.pool).await.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&row.get::<String, _>("payload")).unwrap(),
        expected
    );
    assert_eq!(row.get::<Uuid, _>("credit_event_id"), id);
    assert_eq!(row.get::<String, _>("origin"), "live");
    assert!(
        row.get::<Option<OffsetDateTime>, _>("published_at")
            .is_some()
    );
    let entries = bus.list_ephemeral_entries().await.unwrap();
    assert_eq!(
        entries[0].value_size_bytes,
        serde_json::to_vec(&expected).unwrap().len() as u64
    );
    assert_eq!(
        bus.reveal_ephemeral_entry(&entries[0].entry_ref, EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .unwrap()
            .value,
        Some(expected.clone())
    );
    assert_eq!(bus.poll("credit.events").await.unwrap(), Some(expected));
    assert_eq!(bus.handoff_credit_events(50).await.unwrap(), 0);
    assert_eq!(bus.import_legacy_credit_events(&[id, id]).await.unwrap(), 0);
    assert_eq!(count(&bus, "pending").await, 0);
    assert_eq!(count(&bus, "receipts").await, 1);
    // Idempotency survives poll AND ordinary source deletion.
    sqlx::query("delete from credit_event_outbox where event_id=$1")
        .bind(id)
        .execute(&bus.pool)
        .await
        .unwrap();
    assert_eq!(bus.import_legacy_credit_events(&[id]).await.unwrap(), 0);
}

#[tokio::test]
async fn legacy_is_not_scanned_and_exact_import_preserves_old_timestamp_and_receipt() {
    let (bus, _, workspace) = fixture().await;
    let (legacy, expected) = source(&bus, workspace, true).await;
    let (other, _) = source(&bus, workspace, true).await;
    assert_eq!(bus.handoff_credit_events(50).await.unwrap(), 0);
    assert_eq!(count(&bus, "pending").await, 0);
    assert!(
        bus.import_legacy_credit_events(&[legacy, Uuid::now_v7()])
            .await
            .is_err()
    );
    assert_eq!(count(&bus, "receipts").await, 0);
    assert_eq!(
        bus.import_legacy_credit_events(&[legacy, legacy])
            .await
            .unwrap(),
        1
    );
    let timestamp: OffsetDateTime =
        sqlx::query_scalar("select published_at from credit_event_outbox where event_id=$1")
            .bind(legacy)
            .fetch_one(&bus.pool)
            .await
            .unwrap();
    assert_eq!(
        timestamp,
        OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap()
    );
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "select event_id from credit_event_handoffs where origin='legacy_snapshot'",
    )
    .fetch_all(&bus.pool)
    .await
    .unwrap();
    assert_eq!(ids, vec![legacy]);
    assert!(!ids.contains(&other));
    assert_eq!(bus.poll("credit.events").await.unwrap(), Some(expected));
    assert_eq!(bus.import_legacy_credit_events(&[legacy]).await.unwrap(), 0);
    assert_eq!(count(&bus, "pending").await, 0);
}

#[tokio::test]
async fn source_confirmation_failure_rolls_back_spool_receipt_and_counter() {
    let (bus, _, workspace) = fixture().await;
    let (id, _) = source(&bus, workspace, false).await;
    sqlx::query("create function reject_credit_confirmation() returns trigger language plpgsql as $$ begin raise exception 'fixture confirmation failure'; end $$")
        .execute(&bus.pool).await.unwrap();
    sqlx::query("create trigger reject_credit_confirmation before update on credit_event_outbox for each row execute function reject_credit_confirmation()")
        .execute(&bus.pool).await.unwrap();
    assert!(bus.handoff_credit_events(50).await.is_err());
    assert_eq!(count(&bus, "pending").await, 0);
    assert_eq!(count(&bus, "receipts").await, 0);
    let published: Option<OffsetDateTime> =
        sqlx::query_scalar("select published_at from credit_event_outbox where event_id=$1")
            .bind(id)
            .fetch_one(&bus.pool)
            .await
            .unwrap();
    assert!(published.is_none());
    let counter: i64 = sqlx::query_scalar("select last_sequence from host_event_bus_counter")
        .fetch_one(&bus.pool)
        .await
        .unwrap();
    assert_eq!(counter, 0);
    sqlx::query("drop trigger reject_credit_confirmation on credit_event_outbox")
        .execute(&bus.pool)
        .await
        .unwrap();
    assert_eq!(bus.handoff_credit_events(50).await.unwrap(), 1);
    let reopened = PgEventBus::new(bus.pool.clone());
    assert!(reopened.poll("credit.events").await.unwrap().is_some());
}

#[tokio::test]
async fn grant_envelope_preserves_enqueue_time_account_and_full_original_wire_shape() {
    use control_plane_contracts::ports::{BillingRepository, CreditCommandInput};
    let (bus, store, workspace) = fixture().await;
    let user = Uuid::now_v7();
    sqlx::query("insert into users(id,account,email,password_hash,name,nickname,introduction,email_login_enabled,phone_login_enabled,status,session_version) values ($1,$2,$3,'hash','User','User','',true,false,'active',1)")
        .bind(user).bind(format!("eventbus-{user}")).bind(format!("{user}@example.test")).execute(&bus.pool).await.unwrap();
    let grant = store
        .execute_credit_command(&CreditCommandInput {
            workspace_id: workspace,
            user_id: user,
            amount: "10.25".into(),
            credit_unit: "USD".into(),
            command: "grant".into(),
            reason: "fixture".into(),
            source_type: Some("fixture".into()),
            source_id: Some("one".into()),
            idempotency_key: "eventbus:grant".into(),
            actor_user_id: Some(user),
            actor_plugin_id: None,
            metadata: json!({}),
        })
        .await
        .unwrap();
    // A current optional-zero grant creates its monetary ledger, not a legacy
    // transport source. Build that historical source explicitly from the real
    // committed ledger so this test continues to exercise recovery wire fidelity.
    let sources: i64 =
        sqlx::query_scalar("select count(*) from credit_event_outbox where workspace_id=$1")
            .bind(workspace)
            .fetch_one(&bus.pool)
            .await
            .unwrap();
    assert_eq!(sources, 0);
    let legacy_event_id = Uuid::now_v7();
    let legacy_payload = json!({
        "ledger_id":grant.id,"transaction_id":grant.transaction_id,"user_id":grant.user_id,
        "amount":grant.amount.parse::<rust_decimal::Decimal>().unwrap().normalize().to_string(),
        "balance_after":grant.balance_after,"credit_unit":grant.credit_unit,
    });
    sqlx::query("insert into credit_event_outbox(event_id,workspace_id,account_id,event_type,payload,created_at,published_at) values($1,$2,$3,'CreditGranted',$4,$5,null)")
        .bind(legacy_event_id).bind(workspace).bind(grant.account_id).bind(&legacy_payload).bind(grant.created_at)
        .execute(&bus.pool).await.unwrap();
    let row = sqlx::query("select event_id,workspace_id,account_id,event_type,payload,created_at from credit_event_outbox where workspace_id=$1")
        .bind(workspace).fetch_one(&bus.pool).await.unwrap();
    let id: Uuid = row.get("event_id");
    let account: Uuid = row.get("account_id");
    assert_eq!(id, legacy_event_id);
    assert_eq!(account, grant.account_id);
    assert_eq!(row.get::<Value, _>("payload"), legacy_payload);
    assert_eq!(row.get::<OffsetDateTime, _>("created_at"), grant.created_at);
    let expected = json!({
        "event_id":id,"event_type":"CreditGranted","workspace_id":workspace,
        "account_id":account,"occurred_at":row.get::<OffsetDateTime,_>("created_at"),
        "payload":row.get::<Value,_>("payload"),
    });
    assert_eq!(bus.handoff_credit_events(50).await.unwrap(), 1);
    // Source changes after handoff must not mutate the durable envelope snapshot.
    sqlx::query(r#"update credit_event_outbox set account_id=null,payload='{"mutated":true}'::jsonb,event_type='mutated' where event_id=$1"#)
        .bind(id)
        .execute(&bus.pool)
        .await
        .unwrap();
    assert_eq!(bus.poll("credit.events").await.unwrap(), Some(expected));
}

#[tokio::test]
async fn missing_credit_receipt_cannot_commit_an_orphan_queue_entry() {
    let (bus, _, _) = fixture().await;
    let mut tx = bus.pool.begin().await.unwrap();
    PgEventBus::lock_queue(&mut tx).await.unwrap();
    let sequence = PgEventBus::next_sequence(&mut tx).await.unwrap();
    sqlx::query("insert into host_event_bus_pending(sequence,topic,credit_event_id,payload,value_size_bytes) values ($1,'credit.events',$2,'{}',2)")
        .bind(sequence).bind(Uuid::now_v7()).execute(&mut *tx).await.unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(count(&bus, "pending").await, 0);
    let counter: i64 = sqlx::query_scalar("select last_sequence from host_event_bus_counter")
        .fetch_one(&bus.pool)
        .await
        .unwrap();
    assert_eq!(counter, 0);
}

#[tokio::test]
async fn queued_credit_snapshot_survives_the_original_workspace_cascade() {
    let (bus, _, root_workspace) = fixture().await;
    // Bootstrap seeds notification instances with a non-cascading workspace FK.
    // Create only this test's workspace, without unrelated bootstrap dependents.
    let workspace = Uuid::now_v7();
    assert_ne!(workspace, root_workspace);
    let inserted = sqlx::query("insert into workspaces(id,tenant_id,name) select $1,tenant_id,'EventBus cascade fixture' from workspaces where id=$2")
        .bind(workspace)
        .bind(root_workspace)
        .execute(&bus.pool)
        .await
        .unwrap();
    assert_eq!(inserted.rows_affected(), 1);
    let (id, expected) = source(&bus, workspace, false).await;
    assert_eq!(bus.handoff_credit_events(50).await.unwrap(), 1);
    let entry_ref = bus.list_ephemeral_entries().await.unwrap()[0]
        .entry_ref
        .clone();
    sqlx::query("delete from workspaces where id=$1")
        .bind(workspace)
        .execute(&bus.pool)
        .await
        .unwrap();
    let sources: i64 =
        sqlx::query_scalar("select count(*) from credit_event_outbox where event_id=$1")
            .bind(id)
            .fetch_one(&bus.pool)
            .await
            .unwrap();
    assert_eq!(sources, 0);
    assert_eq!(count(&bus, "receipts").await, 1);
    assert_eq!(count(&bus, "pending").await, 1);
    assert_eq!(
        bus.reveal_ephemeral_entry(&entry_ref, EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .unwrap()
            .value,
        Some(expected.clone())
    );
    assert_eq!(bus.poll("credit.events").await.unwrap(), Some(expected));
    assert_eq!(bus.import_legacy_credit_events(&[id, id]).await.unwrap(), 0);
}

#[tokio::test]
async fn precise_legacy_import_follows_observed_input_order_not_timestamp_or_uuid_order() {
    let (bus, _, workspace) = fixture().await;
    let (a, _) = source(&bus, workspace, true).await;
    let (b, _) = source(&bus, workspace, true).await;
    let (c, _) = source(&bus, workspace, true).await;
    let mut sorted = [a, b, c];
    sorted.sort();
    // Both created_at and UUID order are ascending; caller's observed order is reversed.
    for (index, id) in sorted.iter().enumerate() {
        sqlx::query("update credit_event_outbox set created_at=to_timestamp($2) where event_id=$1")
            .bind(id)
            .bind(1_700_000_000.0 + index as f64)
            .execute(&bus.pool)
            .await
            .unwrap();
    }
    let observed = [sorted[2], sorted[0], sorted[2], sorted[1], sorted[0]];
    assert_eq!(bus.import_legacy_credit_events(&observed).await.unwrap(), 3);
    for id in [sorted[2], sorted[0], sorted[1]] {
        let event = bus.poll("credit.events").await.unwrap().unwrap();
        assert_eq!(event["event_id"], json!(id));
    }
    assert_eq!(bus.import_legacy_credit_events(&observed).await.unwrap(), 0);
}
