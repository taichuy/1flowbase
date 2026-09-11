use super::*;
#[test]
fn issue_2032_formal_output_is_durable_but_diagnostics_remain_ephemeral() {
    let event = provider_output_item_done(
        "llm",
        Uuid::now_v7(),
        0,
        json!({"id":"item-1","type":"message","role":"assistant","phase":"final_answer","content":[]}),
    );
    assert!(
        event.persist_required,
        "log retention must not depend on a provider cursor or socket"
    );
    assert_eq!(event.durability, RuntimeEventDurability::DurableRequired);
    let diagnostic = provider_native_event(
        "llm",
        Uuid::now_v7(),
        "responses".into(),
        json!({"type":"response.output_item.done"}),
    );
    assert!(!diagnostic.persist_required);
    assert_eq!(diagnostic.durability, RuntimeEventDurability::Ephemeral);
}
