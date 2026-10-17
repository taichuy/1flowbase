// Run the application-owned fixture against the same production leaf adapter
// without compiling the unrelated API-server library and binaries in test profile.
#[path = "../../../apps/api-server/tests/event_bus_recovery.rs"]
mod cases;
