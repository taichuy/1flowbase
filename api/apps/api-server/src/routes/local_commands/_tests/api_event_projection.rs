use super::*;
#[test]
fn local_command_before_keeps_actual_command_and_completed_spools_have_real_output_codec() {
    let op = ApiOperationContext {
        interface_id: "local_commands.execute".into(),
        method: "POST".into(),
        path: "/api/console/local-commands/execute".into(),
    };
    let command = "printf 'public text'".repeat(100);
    assert_eq!(
        LocalCommandBody {
            command: command.clone()
        }
        .project_document_input(&op)
        .unwrap(),
        json!({"body":{"command":command}})
    );
    assert_eq!(
        LocalCommandExecution::document_output_unavailable(&op),
        None
    );
}
