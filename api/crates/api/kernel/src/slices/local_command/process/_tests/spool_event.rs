#[test]
fn positioned_event_reads_preserve_an_arbitrary_existing_http_cursor() {
    super::event_support::positioned_event_reads_preserve_an_arbitrary_existing_http_cursor();
}

#[test]
fn failed_event_decode_retains_file_ownership_position_and_original_bytes() {
    super::event_support::failed_event_decode_retains_file_ownership_position_and_original_bytes();
}
