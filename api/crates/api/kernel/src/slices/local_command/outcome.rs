use super::process;
use control_plane::local_command::LocalCommandOutcome;
impl LocalCommandOutcome for process::CompletedCommand {
    fn audit_payload(&self) -> serde_json::Value {
        serde_json::to_value(&self.metadata).expect("command metadata has no fallible serializer")
    }
}
