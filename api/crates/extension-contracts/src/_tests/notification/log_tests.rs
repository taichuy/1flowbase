use super::*;
use crate::NotificationLogDescriptor;

fn mapped() -> NotificationTypeDescriptor {
    let mut descriptor = descriptor();
    descriptor.input_schema["properties"]["heading"] = json!({"type":"string"});
    descriptor.log = Some(
        serde_json::from_value(json!({
            "title":"/heading","content":"/text",
            "extra":{"accepted":{"source":"result","pointer":"/accepted"}}
        }))
        .unwrap(),
    );
    descriptor
}

#[test]
fn notification_log_preserves_body_and_defaults_title_by_unicode_characters() {
    let descriptor = mapped();
    descriptor.validate().unwrap();
    let mapping = descriptor.log.unwrap();
    let content = "汉😀".repeat(300);
    for input in [
        json!({"text":content}),
        json!({"text":content,"heading":"  "}),
    ] {
        let snapshot = mapping.project_input(&input).unwrap();
        assert_eq!(snapshot.content, content);
        assert_eq!(snapshot.title.chars().count(), 255);
        assert_eq!(
            snapshot.title,
            content.chars().take(255).collect::<String>()
        );
        assert_eq!(snapshot.extra, json!({}));
    }
    assert_eq!(
        mapping
            .project_input(&json!({"text":content,"heading":"Explicit heading"}))
            .unwrap()
            .title,
        "Explicit heading"
    );
    assert_eq!(
        mapping.project_result(&json!({"accepted":true,"private":"not mapped"})),
        json!({"accepted":true})
    );
    assert_eq!(mapping.project_result(&json!({})), json!({}));
}

#[test]
fn notification_log_supports_escaped_pointers_and_only_declared_phase_fields() {
    let mut descriptor = mapped();
    descriptor.input_schema["properties"]["a/b~c"] = json!({"type":"object","properties":{"values":{"type":"array","items":{"type":"integer"}}}});
    descriptor.log.as_mut().unwrap().extra.insert(
        "attachment".into(),
        serde_json::from_value(json!({"source":"input","pointer":"/a~1b~0c"})).unwrap(),
    );
    descriptor.validate().unwrap();
    let mapping = descriptor.log.unwrap();
    let snapshot = mapping
        .project_input(&json!({"text":"hello","a/b~c":{"values":[1,2]},"accepted":false}))
        .unwrap();
    assert_eq!(snapshot.extra, json!({"attachment":{"values":[1,2]}}));
    assert_eq!(
        mapping.project_result(&json!({"accepted":true})),
        json!({"accepted":true})
    );
}

#[test]
fn notification_log_rejects_invalid_paths_private_fields_and_runtime_envelopes() {
    for path in [
        "",
        "text",
        "/text~2",
        "/text~",
        "/missing",
        "/config/token",
        "/managed_secrets/token",
    ] {
        let mut descriptor = mapped();
        descriptor.log.as_mut().unwrap().content = path.into();
        assert!(descriptor.validate().is_err(), "{path}");
    }
    let mut descriptor = mapped();
    descriptor.input_schema["properties"]["text"]["writeOnly"] = json!(true);
    assert!(descriptor.validate().is_err());
    let mut descriptor = mapped();
    descriptor.output_schema["properties"]["accepted"]["writeOnly"] = json!(true);
    assert!(descriptor.validate().is_err());
    for source in ["config", "managed_secrets", "runtime", "patch"] {
        assert!(serde_json::from_value::<NotificationLogDescriptor>(json!({
            "content":"/text","extra":{"secret":{"source":source,"pointer":"/token"}}
        }))
        .is_err());
    }
    let mapping = mapped().log.unwrap();
    for input in [
        json!({}),
        json!({"text":42}),
        json!({"text":"safe","heading":true}),
    ] {
        assert!(mapping.project_input(&input).is_err());
    }
}

#[test]
fn notification_log_is_additive_and_rejects_misspelled_declarations() {
    let old = descriptor();
    let raw = serde_json::to_value(&old).unwrap();
    assert!(raw.get("log").is_none());
    assert!(
        serde_json::from_value::<NotificationTypeDescriptor>(raw.clone())
            .unwrap()
            .log
            .is_none()
    );
    let mut invalid = raw;
    invalid["log"] = json!({"content":"/text","contents":"/text"});
    assert!(serde_json::from_value::<NotificationTypeDescriptor>(invalid).is_err());
}
