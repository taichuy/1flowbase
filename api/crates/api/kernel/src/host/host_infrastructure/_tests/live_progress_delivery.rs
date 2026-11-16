use super::*;

#[test]
fn live_block_progress_requires_lossless_delivery_without_durable_logging() {
    for event in [
        "provider_content_block_start",
        "provider_content_block_done",
        "provider_tool_call_delta",
    ] {
        assert!(requires_delivery(event, RuntimeEventDurability::Ephemeral));
    }
    assert!(!requires_delivery(
        "provider_native_event",
        RuntimeEventDurability::Ephemeral
    ));
}
