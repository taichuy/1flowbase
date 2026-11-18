use crate::CommunicationBindingInput;
use serde_json::{json, Value};

fn input() -> Value {
    json!({
        "application_id":"00000000-0000-4000-8000-000000000001",
        "model":null,"reasoning_effort":null,"file_table_id":null,
        "mcp_instance_ids":["channel-history"],"system_prompt":"Keep the configured prompt",
        "commands":[
            {"kind":"registered","command_id":"host/help","enabled":false},
            {"kind":"event","command_id":"user/paipan","name":"paipan","description":"Chart",
             "enabled":true,"contract_id":"custom.command.chart","input_mapping":{"arguments":"/arguments"}}
        ]
    })
}

#[test]
fn one_collection_roundtrips_registered_switches_and_custom_event_targets() {
    let wire = input();
    let binding: CommunicationBindingInput = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(binding.commands.len(), 2);
    assert!(!binding.command_settings()[0].enabled);
    assert_eq!(
        binding.event_commands()[0].contract_id,
        "custom.command.chart"
    );
    assert_eq!(serde_json::to_value(binding).unwrap(), wire);
}

#[test]
fn retired_fields_and_conflicting_command_variants_are_rejected() {
    for field in ["command_settings", "event_commands"] {
        let mut wire = input();
        wire[field] = json!([]);
        assert!(serde_json::from_value::<CommunicationBindingInput>(wire).is_err());
    }
    let mut wire = input();
    wire["commands"][0]["contract_id"] = json!("custom.forged");
    assert!(serde_json::from_value::<CommunicationBindingInput>(wire).is_err());
    let mut wire = input();
    wire["commands"][1]["kind"] = json!("registered");
    assert!(serde_json::from_value::<CommunicationBindingInput>(wire).is_err());
}
