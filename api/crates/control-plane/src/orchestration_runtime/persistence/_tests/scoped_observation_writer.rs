use super::*;

#[test]
fn scoped_write_ids_survive_partial_usage_links_and_keep_events_distinct() {
    let observation_id = Uuid::now_v7();
    let first = ScopedObservationWriter::new(&(), observation_id);
    let span = first.record_id("Span", None).unwrap();
    let first_attempt = first.record_id("Attempt", None).unwrap();
    let second_attempt = first.record_id("Attempt", None).unwrap();
    let first_usage = first
        .record_id("Usage", Some(first_attempt.to_string()))
        .unwrap();
    let second_usage = first
        .record_id("Usage", Some(second_attempt.to_string()))
        .unwrap();
    let first_event = first.record_id("RuntimeEvent", None).unwrap();
    let second_event = first.record_id("RuntimeEvent", None).unwrap();
    assert_ne!(
        first_event, second_event,
        "identical adjacent events remain distinct facts"
    );

    let resumed = ScopedObservationWriter::new(&(), observation_id);
    assert_eq!(resumed.record_id("Span", None).unwrap(), span);
    assert_eq!(resumed.record_id("Attempt", None).unwrap(), first_attempt);
    assert_eq!(resumed.record_id("Attempt", None).unwrap(), second_attempt);
    // The first usage is already linked, so the existing helper omits that write.
    assert_eq!(
        resumed
            .record_id("Usage", Some(second_attempt.to_string()))
            .unwrap(),
        second_usage
    );
    assert_eq!(
        resumed.record_id("RuntimeEvent", None).unwrap(),
        first_event
    );
    assert_eq!(
        resumed.record_id("RuntimeEvent", None).unwrap(),
        second_event
    );
    assert_ne!(first_usage, second_usage);
    let another_observation = ScopedObservationWriter::new(&(), Uuid::now_v7());
    assert_ne!(another_observation.record_id("Span", None).unwrap(), span);
}
