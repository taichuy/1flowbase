use serde::Deserialize;

#[derive(Deserialize)]
struct Inventory {
    count: usize,
    consumers: Vec<Consumer>,
}

#[derive(Deserialize)]
struct Consumer {
    module: String,
    msgid: String,
    path: String,
}

#[test]
fn ac_010_dynamic_backend_consumer_inventory_is_exact_and_english_owned() {
    let inventory: Inventory = serde_json::from_str(include_str!(
        "fixtures/dynamic_backend_consumers.d3-p6.json"
    ))
    .unwrap();
    assert_eq!(inventory.count, 18);
    assert_eq!(inventory.consumers.len(), inventory.count);
    assert!(inventory.consumers.iter().all(|consumer| {
        consumer.module.starts_with("@taichuy/platform/")
            && consumer.path.starts_with("api/")
            && !consumer
                .msgid
                .chars()
                .any(|character| matches!(character, '\u{4e00}'..='\u{9fff}'))
    }));
}
