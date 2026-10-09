use super::*;

#[tokio::test]
async fn same_waiting_workload_releases_confirmed_ring_bytes_and_capacity_with_writers_pinned() {
    // These sizes describe the controlled test workload, not admission limits.
    const RUNS: usize = 8;
    const EVENTS_PER_RUN: usize = 32;
    const BODY_BYTES: usize = 4096;
    let legacy = LocalRuntimeEventStream::new();
    let confirmed = LocalRuntimeEventStream::new();
    let mut legacy_probes = Vec::new();
    let mut confirmed_probes = Vec::new();
    let mut legacy_writers = Vec::new();
    let mut confirmed_writers = Vec::new();

    for _ in 0..RUNS {
        let run_id = Uuid::now_v7();
        for stream in [&legacy, &confirmed] {
            stream
                .open_run(run_id, RuntimeEventStreamPolicy::debug_default())
                .await
                .unwrap();
            for _ in 0..EVENTS_PER_RUN {
                stream
                    .append(
                        run_id,
                        blob_event(BODY_BYTES, RuntimeEventDurability::DurableRequired),
                    )
                    .await
                    .unwrap();
            }
            stream
                .append_terminal_if_missing_and_close(
                    run_id,
                    RuntimeEventPayload {
                        event_type: "waiting_callback".into(),
                        source: RuntimeEventSource::Runtime,
                        durability: RuntimeEventDurability::DurableRequired,
                        persist_required: true,
                        trace_visible: true,
                        payload: json!({"type": "waiting_callback"}),
                    },
                )
                .await
                .unwrap();
        }
        legacy_probes.push(legacy.run_probe_for_tests(run_id).unwrap());
        confirmed_probes.push(confirmed.run_probe_for_tests(run_id).unwrap());
        legacy_writers.push(legacy.terminal_writer(run_id).await.unwrap());
        confirmed_writers.push((run_id, confirmed.terminal_writer(run_id).await.unwrap()));
    }
    let legacy_before: usize = legacy_probes.iter().map(|probe| probe().1).sum();
    let confirmed_before: usize = confirmed_probes.iter().map(|probe| probe().1).sum();
    let legacy_capacity: usize = legacy_probes.iter().map(|probe| probe().2).sum();
    let confirmed_capacity: usize = confirmed_probes.iter().map(|probe| probe().2).sum();
    assert!(legacy_before >= RUNS * EVENTS_PER_RUN * BODY_BYTES);
    assert_eq!(confirmed_before, legacy_before);
    assert_eq!(confirmed_capacity, legacy_capacity);

    let aged = OffsetDateTime::now_utc() - TimeDuration::minutes(6);
    for (run_id, writer) in &confirmed_writers {
        // The actual persister proof is covered separately; here isolate ownership
        // and the default hot lifetime under an identical retained workload.
        writer.set_durable_replay_boundary(1).unwrap();
        writer
            .confirm_terminal_persisted((EVENTS_PER_RUN + 1) as i64)
            .await
            .unwrap();
        legacy
            .set_run_timestamps_for_tests(*run_id, aged, Some(aged))
            .unwrap();
        confirmed
            .set_run_timestamps_for_tests(*run_id, aged, Some(aged))
            .unwrap();
        confirmed
            .set_confirmation_timestamp_for_tests(*run_id, aged)
            .unwrap();
        wait_for_idle_reclamation(&confirmed, *run_id).await;
    }
    let legacy_after: usize = legacy_probes.iter().map(|probe| probe().1).sum();
    let confirmed_after: usize = confirmed_probes.iter().map(|probe| probe().1).sum();
    let confirmed_capacity_after: usize = confirmed_probes.iter().map(|probe| probe().2).sum();
    assert_eq!(legacy_after, legacy_before);
    assert_eq!(confirmed_after, 0);
    assert_eq!(confirmed_capacity_after, 0);
    assert!(confirmed_probes.iter().all(|probe| probe().0 == 1));
    for ((run_id, writer), old_writer) in confirmed_writers.iter().zip(&legacy_writers) {
        assert_eq!(writer.closure(), old_writer.closure());
        assert!(confirmed.replay(*run_id, None, 1).await.is_err());
        assert!(legacy.replay(*run_id, None, 1).await.is_ok());
    }
    eprintln!(
        "replay_gc_memory_comparison={}",
        json!({
            "runs": RUNS, "events_per_run": EVENTS_PER_RUN, "body_bytes": BODY_BYTES,
            "legacy_ring_bytes_before": legacy_before, "confirmed_ring_bytes_before": confirmed_before,
            "legacy_ring_bytes_after": legacy_after, "confirmed_ring_bytes_after": confirmed_after,
            "legacy_ring_capacity_before": legacy_capacity, "confirmed_ring_capacity_after": confirmed_capacity_after,
            "pinned_terminal_writers": confirmed_writers.len(),
            "rss_pss": "not measured; ring ownership release is not allocator page-return evidence"
        })
    );
}
