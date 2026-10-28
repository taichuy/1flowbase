use crate::*;
use serde_json::json;

#[test]
fn communication_command_reply_wire_is_bounded_and_has_no_destination_authority() {
    let outcome = ManagedEventOutcome::CommandReply {
        reply: CommunicationCommandReply {
            text: "reply".into(),
        },
    };
    let value = serde_json::to_value(&outcome).unwrap();
    assert_eq!(
        value,
        json!({"outcome":"command_reply","reply":{"text":"reply"}})
    );
    assert_eq!(
        serde_json::from_value::<ManagedEventOutcome>(value.clone()).unwrap(),
        outcome
    );
    for field in [
        "workspace_id",
        "actor_id",
        "recipient",
        "installation_id",
        "channel_id",
    ] {
        let mut forged = value.clone();
        forged["reply"][field] = json!("forged");
        assert!(serde_json::from_value::<ManagedEventOutcome>(forged).is_err());
    }
    for text in [String::new(), " ".into(), "x".repeat(32_769)] {
        assert!(CommunicationCommandReply { text }.validate().is_err());
    }
    assert!(CommunicationCommandReply {
        text: "x".repeat(32_768)
    }
    .validate()
    .is_ok());
}
