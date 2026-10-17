use super::*;

#[tokio::test]
async fn generic_roundtrip_inspection_and_reopen_preserve_pending_fifo() {
    let (bus, _, _) = fixture().await;
    let values = [
        json!(null),
        json!([1, true, "正文\n"]),
        json!({"a":{"b":[1,2]},"empty":{}}),
    ];
    for value in &values {
        bus.publish("first", value.clone()).await.unwrap();
    }
    bus.publish("second", json!({"retained":true}))
        .await
        .unwrap();
    let reopened = PgEventBus::new(bus.pool.clone());
    assert_eq!(
        reopened.ephemeral_inspection_capabilities().storage_medium,
        Some("postgresql-durable")
    );
    let entries = reopened.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), 4);
    assert!(
        entries
            .iter()
            .all(|entry| entry.sensitive && entry.status == "buffered")
    );
    for entry in &entries {
        let metadata = reopened
            .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Metadata)
            .await
            .unwrap()
            .unwrap();
        assert!(metadata.value.is_none());
        assert_eq!(metadata.full_value_size_bytes, entry.value_size_bytes);
        assert_eq!(entry.metadata["storage_medium"], "postgresql-durable");
    }
    let first = reopened
        .reveal_ephemeral_entry("1", EphemeralValueRevealMode::Full)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.value, Some(json!(null)));
    assert_eq!(count(&bus, "pending").await, 4);
    for value in values {
        assert_eq!(reopened.poll("first").await.unwrap(), Some(value));
    }
    assert_eq!(reopened.poll("first").await.unwrap(), None);
    assert_eq!(reopened.poll("absent").await.unwrap(), None);
    assert_eq!(
        reopened.poll("second").await.unwrap(),
        Some(json!({"retained":true}))
    );
    assert!(
        reopened
            .reveal_ephemeral_entry("1", EphemeralValueRevealMode::Full)
            .await
            .unwrap()
            .is_none()
    );
    bus.publish("first", json!("after drain")).await.unwrap();
    assert_eq!(
        bus.list_ephemeral_entries().await.unwrap()[0].entry_ref,
        "5"
    );
    assert!(
        bus.publish("large", json!("x".repeat(2 * 1024 * 1024)))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn publication_counter_and_poll_head_wait_for_transaction_commit() {
    let (bus, _, _) = fixture().await;
    let mut tx = bus.pool.begin().await.unwrap();
    PgEventBus::lock_queue(&mut tx).await.unwrap();
    let sequence = PgEventBus::next_sequence(&mut tx).await.unwrap();
    sqlx::query("insert into host_event_bus_pending(sequence,topic,payload,value_size_bytes) values ($1,'ordered','1',1)")
        .bind(sequence).execute(&mut *tx).await.unwrap();
    let producer = bus.clone();
    let mut publish = tokio::spawn(async move { producer.publish("ordered", json!(2)).await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut publish)
            .await
            .is_err()
    );
    assert!(bus.list_ephemeral_entries().await.unwrap().is_empty());
    tx.commit().await.unwrap();
    publish.await.unwrap().unwrap();

    let mut tx = bus.pool.begin().await.unwrap();
    PgEventBus::lock_queue(&mut tx).await.unwrap();
    let first: String = sqlx::query_scalar("delete from host_event_bus_pending where sequence=(select min(sequence) from host_event_bus_pending where topic='ordered') returning payload")
        .fetch_one(&mut *tx).await.unwrap();
    assert_eq!(serde_json::from_str::<Value>(&first).unwrap(), json!(1));
    let consumer = bus.clone();
    let mut take = tokio::spawn(async move { consumer.poll("ordered").await });
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut take)
            .await
            .is_err()
    );
    // Rolling back must put the original head back, not let the second consumer skip it.
    tx.rollback().await.unwrap();
    assert_eq!(take.await.unwrap().unwrap(), Some(json!(1)));
    assert_eq!(bus.poll("ordered").await.unwrap(), Some(json!(2)));
}

#[tokio::test]
async fn multiple_producers_share_unique_sequence_and_two_polls_do_not_duplicate() {
    let (bus, _, _) = fixture().await;
    let mut producers = Vec::new();
    for id in 0..8 {
        let bus = bus.clone();
        producers.push(tokio::spawn(async move {
            for offset in 0..4 {
                bus.publish(&format!("topic-{id}"), json!([id, offset]))
                    .await
                    .unwrap();
            }
        }));
    }
    for producer in producers {
        producer.await.unwrap();
    }
    let entries = bus.list_ephemeral_entries().await.unwrap();
    let refs: std::collections::BTreeSet<_> = entries
        .iter()
        .map(|e| e.entry_ref.parse::<i64>().unwrap())
        .collect();
    assert_eq!(refs, (1..=32).collect());
    for id in 0..8 {
        for offset in 0..4 {
            assert_eq!(
                bus.poll(&format!("topic-{id}")).await.unwrap(),
                Some(json!([id, offset]))
            );
        }
    }
    bus.publish("race", json!(1)).await.unwrap();
    bus.publish("race", json!(2)).await.unwrap();
    let (a, b) = tokio::join!(bus.poll("race"), bus.poll("race"));
    let mut values = vec![
        a.unwrap().unwrap().as_i64().unwrap(),
        b.unwrap().unwrap().as_i64().unwrap(),
    ];
    values.sort();
    assert_eq!(values, vec![1, 2]);
    assert_eq!(bus.poll("race").await.unwrap(), None);
}

#[tokio::test]
async fn generic_snapshot_text_roundtrips_every_supported_json_shape_without_jsonb_narrowing() {
    let (bus, _, _) = fixture().await;
    let values = vec![
        json!({"nul\0key":["before\0after","literal\\u0000",null],
            "nul\\u0000key":"distinct", "nested":{"a\0b":"c\0d"}}),
        Value::Null,
        json!(false),
        json!("scalar\0text"),
        json!([
            i64::MIN,
            u64::MAX,
            0.1,
            1.25,
            1e100,
            f64::MIN_POSITIVE,
            f64::MAX,
            null
        ]),
        json!({"ordinary":[true,false,{"key":"\\"}]}),
    ];
    for value in &values {
        bus.publish("lossless\0topic", value.clone()).await.unwrap();
    }
    let reopened = PgEventBus::new(bus.pool.clone());
    let entries = reopened.list_ephemeral_entries().await.unwrap();
    assert_eq!(entries.len(), values.len());
    for (index, value) in values.iter().enumerate() {
        let entry = entries
            .iter()
            .find(|entry| entry.entry_ref == (index + 1).to_string())
            .unwrap();
        let text: String =
            sqlx::query_scalar("select payload from host_event_bus_pending where sequence=$1")
                .bind((index + 1) as i64)
                .fetch_one(&bus.pool)
                .await
                .unwrap();
        assert_eq!(text, serde_json::to_string(value).unwrap());
        assert!(!text.contains('\0'));
        assert_eq!(entry.value_size_bytes, text.len() as u64);
        assert_eq!(
            reopened
                .reveal_ephemeral_entry(&entry.entry_ref, EphemeralValueRevealMode::Full)
                .await
                .unwrap()
                .unwrap()
                .value,
            Some(value.clone())
        );
    }
    for value in values {
        assert_eq!(reopened.poll("lossless\0topic").await.unwrap(), Some(value));
    }
}
