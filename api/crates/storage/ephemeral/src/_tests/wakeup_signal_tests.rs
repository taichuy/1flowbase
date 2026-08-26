use storage_ephemeral::{MemoryWakeupSignalBus, WakeupSignalBus};

#[tokio::test]
async fn memory_wakeup_signal_bus_delivers_one_signal() {
    let bus = MemoryWakeupSignalBus::new();

    bus.publish("flow-run:1").await.unwrap();

    assert_eq!(bus.poll().await.unwrap(), Some("flow-run:1".to_string()));
}
