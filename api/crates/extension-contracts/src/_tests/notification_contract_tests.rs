use crate::{ContributionId, NotificationTypeDescriptor, PluginFormSchema};
use serde_json::json;

fn descriptor() -> NotificationTypeDescriptor {
    NotificationTypeDescriptor {
        i18n: Default::default(),
        type_code: "acme/fixture/webhook".into(),
        display_name: "Webhook".into(),
        description: "Sends a provider payload".into(),
        config_schema: json!({"type":"object","properties":{"token":{"type":"string","minLength":1}},"required":["token"],"additionalProperties":false}),
        config_form: serde_json::from_value(json!({"schema_version":"1.0.0","fields":[{"key":"token","label":"Token","type":"string","control":"password"}]})).unwrap(),
        input_schema: json!({"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false}),
        input_form: serde_json::from_value(json!({"schema_version":"1.0.0","fields":[{"key":"text","label":"Text","type":"string"}]})).unwrap(),
        output_schema: json!({"type":"object","properties":{"accepted":{"type":"boolean"}},"required":["accepted"],"additionalProperties":false}),
        authorization: None,
        contribution_id: ContributionId::new("fixture.send").unwrap(),
    }
}

#[test]
fn notification_config_input_output_are_independent_and_errors_do_not_reveal_secrets() {
    let descriptor = descriptor();
    descriptor
        .validate_plugin_namespace("acme", "fixture")
        .unwrap();
    assert_eq!(
        descriptor.secret_config_fields().collect::<Vec<_>>(),
        ["token"]
    );
    descriptor
        .validate_config(&json!({"token":"credential"}))
        .unwrap();
    descriptor
        .validate_input(&json!({"text":"message"}))
        .unwrap();
    descriptor
        .validate_output(&json!({"accepted":true}))
        .unwrap();
    assert!(descriptor
        .validate_config(&json!({"text":"message"}))
        .is_err());
    assert!(descriptor
        .validate_input(&json!({"token":"credential"}))
        .is_err());
    let error = descriptor
        .validate_config(&json!({"token":0,"extra":"credential"}))
        .unwrap_err();
    assert!(!error.to_string().contains("credential"));
    assert!(descriptor
        .validate_output(&json!({"accepted":"true"}))
        .is_err());
}

#[test]
fn notification_descriptor_rejects_invalid_identity_schema_and_form() {
    let original = descriptor();
    for type_code in [
        "system",
        "other/fixture/webhook",
        "acme/fixture/",
        "acme/fixture/a/b",
    ] {
        let mut value = original.clone();
        value.type_code = type_code.into();
        assert!(value.validate_plugin_namespace("acme", "fixture").is_err());
    }
    for schema in [
        json!({"type":"unknown"}),
        json!({"$ref":"https://example.com/schema"}),
        json!({"type":"object","properties":{"x":{"$dynamicRef":"file:///secret"}}}),
    ] {
        let mut value = original.clone();
        value.output_schema = schema;
        assert!(value.validate().is_err());
    }
    let mut value = original.clone();
    value.config_form.fields[0].key = "missing".into();
    assert!(value.validate().is_err());
    let mut value = original.clone();
    value.input_form.fields[0].control = Some("password".into());
    assert!(value.validate().is_err());
    let mut value = original.clone();
    value
        .config_form
        .fields
        .push(value.config_form.fields[0].clone());
    assert!(value.validate().is_err());
    let mut value = original;
    value.config_form = PluginFormSchema {
        schema_version: "unknown".into(),
        title: None,
        description: None,
        fields: vec![],
    };
    assert!(value.validate().is_err());
}

#[test]
fn multi_select_validates_options_as_items_and_defaults_as_arrays() {
    let mut value = descriptor();
    value.input_schema = json!({"type":"object","properties":{"recipients":{"type":"array","items":{"type":"string","enum":["alice","bob"]}}}});
    value.input_form = serde_json::from_value(json!({"schema_version":"1.0.0","fields":[{"key":"recipients","label":"Recipients","type":"select","control":"multi-select","default_value":["alice"],"options":[{"label":"Alice","value":"alice"},{"label":"Bob","value":"bob"}]}]})).unwrap();
    value.validate().unwrap();
    value.input_form.fields[0].options[0].value = json!("mallory");
    assert!(value.validate().is_err());
    value.input_form.fields[0].options[0].value = json!("alice");
    value.input_form.fields[0].default_value = Some(json!("alice"));
    assert!(value.validate().is_err());
}

fn authorizable_descriptor() -> NotificationTypeDescriptor {
    let mut descriptor = descriptor();
    descriptor.authorization = Some(serde_json::from_value(json!({
        "actions":[{"code":"qr","label":"Scan QR code","kind":"qr_code"}],
        "managed_secret_keys":["qr_credential","bot_token","bound_user_id","context_token","cursor"]
    })).unwrap());
    descriptor
}

fn bilingual_i18n() -> serde_json::Value {
    json!({
        "zh_Hans": {
            "display_name":"回执", "description":"发送回执",
            "config_form":{"title":"配置", "description":"渠道配置", "fields":{
                "token":{"label":"令牌", "description":"渠道令牌", "placeholder":"输入令牌"}
            }},
            "input_form":{"title":"消息", "fields":{"text":{"label":"正文"}}},
            "authorization":{"actions":{"qr":{"label":"扫码授权"}}}
        },
        "en_US": {
            "display_name":"Receipt",
            "input_form":{"fields":{"text":{"label":"Message"}}},
            "authorization":{"actions":{"qr":{"label":"Scan QR code"}}}
        }
    })
}

#[test]
fn notification_i18n_roundtrips_without_changing_canonical_business_metadata() {
    let original = authorizable_descriptor();
    let mut descriptor = original.clone();
    descriptor.i18n = serde_json::from_value(bilingual_i18n()).unwrap();
    descriptor.validate().unwrap();
    let wire = serde_json::to_value(&descriptor).unwrap();
    assert_eq!(wire["i18n"], bilingual_i18n());
    assert_eq!(wire["i18n"]["en_US"].get("description"), None);
    let restored: NotificationTypeDescriptor = serde_json::from_value(wire).unwrap();
    assert_eq!(restored, descriptor);
    descriptor.i18n.clear();
    assert_eq!(descriptor, original);
    let legacy_wire = serde_json::to_value(&original).unwrap();
    assert!(legacy_wire.get("i18n").is_none());
    let legacy: NotificationTypeDescriptor = serde_json::from_value(legacy_wire).unwrap();
    assert!(legacy.i18n.is_empty());
    legacy.validate().unwrap();
}

#[test]
fn notification_i18n_partial_entries_default_empty_maps_and_omit_absent_strings() {
    let partial = json!({"en_US": {
        "config_form":{"title":"Configuration"},
        "input_form":{}, "authorization":{}
    }});
    let mut descriptor = authorizable_descriptor();
    descriptor.i18n = serde_json::from_value(partial).unwrap();
    descriptor.validate().unwrap();
    let translation = &descriptor.i18n["en_US"];
    assert!(translation.config_form.as_ref().unwrap().fields.is_empty());
    assert!(translation.input_form.as_ref().unwrap().fields.is_empty());
    assert!(translation
        .authorization
        .as_ref()
        .unwrap()
        .actions
        .is_empty());
    assert_eq!(
        serde_json::to_value(translation).unwrap(),
        json!({
            "config_form":{"title":"Configuration"}, "input_form":{}, "authorization":{}
        })
    );
    assert!(
        serde_json::from_value::<crate::NotificationAuthorizationActionLocale>(json!({})).is_err()
    );
}

#[test]
fn notification_i18n_rejects_business_overrides_at_every_level() {
    for (pointer, key, value) in [
        ("/zh_Hans", "config_schema", json!({"type":"object"})),
        ("/zh_Hans", "type_code", json!("system")),
        ("/zh_Hans/config_form", "schema_version", json!("2.0.0")),
        ("/zh_Hans/config_form/fields/token", "key", json!("other")),
        (
            "/zh_Hans/config_form/fields/token",
            "control",
            json!("text"),
        ),
        (
            "/zh_Hans/config_form/fields/token",
            "default_value",
            json!("secret"),
        ),
        (
            "/zh_Hans/config_form/fields/token",
            "value",
            json!("secret"),
        ),
        ("/zh_Hans/config_form/fields/token", "options", json!([])),
        (
            "/zh_Hans/authorization",
            "managed_secret_keys",
            json!(["token"]),
        ),
        (
            "/zh_Hans/authorization/actions/qr",
            "kind",
            json!("qr_code"),
        ),
        ("/zh_Hans/authorization/actions/qr", "code", json!("other")),
    ] {
        let mut translations = bilingual_i18n();
        translations.pointer_mut(pointer).unwrap()[key] = value;
        let mut wire = serde_json::to_value(authorizable_descriptor()).unwrap();
        wire["i18n"] = translations;
        assert!(
            serde_json::from_value::<NotificationTypeDescriptor>(wire).is_err(),
            "{pointer}/{key}"
        );
    }
}

#[test]
fn notification_i18n_rejects_unknown_locales_stale_references_and_blank_strings() {
    for locale in ["zh-CN", "en-US", "fr_FR", "zh_hans"] {
        let mut descriptor = authorizable_descriptor();
        descriptor.i18n.insert(locale.into(), Default::default());
        assert!(descriptor.validate().is_err(), "{locale}");
    }
    for pointer in [
        "/zh_Hans/display_name",
        "/zh_Hans/description",
        "/zh_Hans/config_form/title",
        "/zh_Hans/config_form/description",
        "/zh_Hans/config_form/fields/token/label",
        "/zh_Hans/config_form/fields/token/description",
        "/zh_Hans/config_form/fields/token/placeholder",
        "/zh_Hans/input_form/title",
        "/zh_Hans/input_form/fields/text/label",
        "/zh_Hans/authorization/actions/qr/label",
    ] {
        let mut translations = bilingual_i18n();
        *translations.pointer_mut(pointer).unwrap() = json!(" \n ");
        let mut descriptor = authorizable_descriptor();
        descriptor.i18n = serde_json::from_value(translations).unwrap();
        assert!(descriptor.validate().is_err(), "{pointer}");
    }
    for (pointer, stale) in [
        ("/zh_Hans/config_form/fields", "text"),
        ("/zh_Hans/input_form/fields", "token"),
        ("/zh_Hans/authorization/actions", "missing"),
    ] {
        let mut translations = bilingual_i18n();
        translations.pointer_mut(pointer).unwrap()[stale] = json!({"label":"Stale"});
        let mut descriptor = authorizable_descriptor();
        descriptor.i18n = serde_json::from_value(translations).unwrap();
        assert!(descriptor.validate().is_err(), "{pointer}/{stale}");
    }
    let mut descriptor = descriptor();
    descriptor.i18n = serde_json::from_value(bilingual_i18n()).unwrap();
    assert!(
        descriptor.validate().is_err(),
        "legacy type has no authorization actions"
    );
}

#[test]
fn notification_authorization_is_opt_in_and_rejects_user_owned_managed_keys() {
    let mut descriptor = authorizable_descriptor();
    descriptor.validate().unwrap();
    let mut legacy = serde_json::to_value(&descriptor).unwrap();
    legacy.as_object_mut().unwrap().remove("authorization");
    let legacy: NotificationTypeDescriptor = serde_json::from_value(legacy).unwrap();
    assert!(legacy.authorization.is_none());
    assert!(serde_json::to_value(legacy)
        .unwrap()
        .get("authorization")
        .is_none());
    descriptor.config_schema["additionalProperties"] = json!(true);
    assert!(descriptor
        .validate_config(&json!({"token":"safe","bot_token":"private"}))
        .is_err());
    descriptor.config_schema["properties"]["bot_token"] = json!({"type":"string"});
    assert!(descriptor.validate().is_err());
    for value in [
        json!({"actions":[],"managed_secret_keys":["bot_token"]}),
        json!({"actions":[{"code":"qr","label":"QR","kind":"qr_code"}],"managed_secret_keys":["bot_token","bot_token"]}),
        json!({"actions":[{"code":"qr","label":"QR","kind":"qr_code"},{"code":"qr","label":"QR","kind":"qr_code"}],"managed_secret_keys":["bot_token"]}),
    ] {
        let value: crate::NotificationAuthorizationDescriptor =
            serde_json::from_value(value).unwrap();
        assert!(value.validate().is_err());
    }
}

#[test]
fn notification_authorization_pending_qr_and_declared_patch_are_checked() {
    use crate::{NotificationRuntimeInput, NotificationRuntimeResult};
    let descriptor = authorizable_descriptor();
    let input: NotificationRuntimeInput = serde_json::from_value(json!({
        "operation":{"type":"authorize","operation":{"type":"begin","action":"qr"}}
    }))
    .unwrap();
    let response = json!({"type":"authorization","authorization":{
        "status":"pending","user_action":{"kind":"qr_code","qr_code_url":"https://example.com/qr","poll_interval_seconds":2}
    },"managed_secret_patch":{"qr_credential":"host-only"}});
    let valid: NotificationRuntimeResult = serde_json::from_value(response.clone()).unwrap();
    valid.validate_for(&input, &descriptor).unwrap();
    // Public DTO has no way to carry a managed credential or raw provider error.
    assert!(
        serde_json::from_value::<crate::NotificationAuthorizationView>(
            json!({"status":"authorized","bot_token":"private"})
        )
        .is_err()
    );
    for invalid in [
        json!({"type":"authorization","authorization":{"status":"pending"}}),
        json!({"type":"authorization","authorization":{"status":"authorized","user_action":{"kind":"qr_code","qr_code_url":"https://example.com/qr","poll_interval_seconds":2}}}),
        json!({"type":"authorization","authorization":{"status":"unauthorized"},"managed_secret_patch":{"unknown_secret":"private"}}),
        json!({"type":"send","outcome":{"status":"unknown","failure_code":"provider_result_unknown"}}),
    ] {
        let invalid: NotificationRuntimeResult = serde_json::from_value(invalid).unwrap();
        assert!(invalid.validate_for(&input, &descriptor).is_err());
    }
    for url in [
        "file:///credential",
        "http://example.com/qr",
        "https://token@example.com/qr",
        "not-a-url",
    ] {
        let mut value = response.clone();
        value["authorization"]["user_action"]["qr_code_url"] = json!(url);
        let value: NotificationRuntimeResult = serde_json::from_value(value).unwrap();
        assert!(value.validate_for(&input, &descriptor).is_err());
    }
    for operation in ["poll", "cancel", "maintain"] {
        let input: NotificationRuntimeInput = serde_json::from_value(json!({"operation":{"type":"authorize","operation":{"type":operation}},"managed_secrets":{"qr_credential":"persisted"}})).unwrap();
        input.validate(&descriptor).unwrap();
    }
    let invalid: NotificationRuntimeInput = serde_json::from_value(
        json!({"operation":{"type":"authorize","operation":{"type":"begin","action":"other"}}}),
    )
    .unwrap();
    assert!(invalid.validate(&descriptor).is_err());
}

#[test]
fn notification_send_outcomes_never_conflate_ambiguous_delivery_or_revoke_ready_account() {
    use crate::{NotificationRuntimeInput, NotificationRuntimeResult};
    let descriptor = authorizable_descriptor();
    let input: NotificationRuntimeInput = serde_json::from_value(json!({"operation":{"type":"send","input_payload":{"text":"hello"}},"managed_secrets":{"bound_user_id":"scanned-peer"}})).unwrap();
    for value in [
        json!({"type":"send","outcome":{"status":"succeeded","result":{"accepted":true}},"authorization":{"status":"authorized"},"managed_secret_patch":{"cursor":"next","context_token":"host-only"}}),
        json!({"type":"send","outcome":{"status":"failed","failure_code":"recipient_not_ready"},"authorization":{"status":"authorized"}}),
        json!({"type":"send","outcome":{"status":"failed","failure_code":"authorization_invalid"},"authorization":{"status":"invalid"},"managed_secret_patch":{"bot_token":null,"context_token":null}}),
        json!({"type":"send","outcome":{"status":"failed","failure_code":"authorization_required"},"authorization":{"status":"unauthorized"}}),
        json!({"type":"send","outcome":{"status":"failed","failure_code":"provider_rejected"}}),
        json!({"type":"send","outcome":{"status":"unknown","failure_code":"provider_result_unknown"}}),
    ] {
        let value: NotificationRuntimeResult = serde_json::from_value(value).unwrap();
        value.validate_for(&input, &descriptor).unwrap();
    }
    for value in [
        json!({"type":"send","outcome":{"status":"failed","failure_code":"provider_result_unknown"}}),
        json!({"type":"send","outcome":{"status":"unknown","failure_code":"provider_rejected"}}),
        json!({"type":"send","outcome":{"status":"succeeded","result":{"accepted":"yes"}}}),
        json!({"type":"send","outcome":{"status":"failed","failure_code":"authorization_invalid"}}),
        json!({"type":"send","outcome":{"status":"failed","failure_code":"recipient_not_ready"},"authorization":{"status":"invalid"}}),
    ] {
        let value: NotificationRuntimeResult = serde_json::from_value(value).unwrap();
        assert!(value.validate_for(&input, &descriptor).is_err());
    }
    assert!(serde_json::from_value::<NotificationRuntimeResult>(
        json!({"type":"send","outcome":{"status":"failed","failure_code":"raw-provider-error"}})
    )
    .is_err());
}

#[test]
fn delivery_window_contract_preserves_legacy_and_checks_evidence_pairs() {
    use crate::{NotificationAuthorizationView, NotificationRuntimeInput};
    let legacy: NotificationAuthorizationView =
        serde_json::from_value(json!({"status":"authorized"})).unwrap();
    assert!(legacy.delivery_window.is_none());
    assert_eq!(
        serde_json::to_value(legacy).unwrap(),
        json!({"status":"authorized"})
    );
    for window in [
        json!({"status":"unknown"}),
        json!({"status":"unknown","last_received_at_ms":1,"expires_at_ms":2}),
        json!({"status":"waiting_for_message"}),
        json!({"status":"active","last_received_at_ms":1,"expires_at_ms":2}),
        json!({"status":"expired","last_received_at_ms":1,"expires_at_ms":u64::MAX}),
    ] {
        let view: NotificationAuthorizationView =
            serde_json::from_value(json!({"status":"authorized","delivery_window":window}))
                .unwrap();
        view.validate().unwrap();
        let input: NotificationRuntimeInput = serde_json::from_value(json!({"operation":{"type":"send","input_payload":{"text":"hello"}},"delivery_window":window})).unwrap();
        input.validate(&authorizable_descriptor()).unwrap();
        assert!(input.managed_secrets.is_empty());
        assert_eq!(
            serde_json::to_value(input).unwrap()["delivery_window"],
            window
        );
    }
    for window in [
        json!({"status":"active"}),
        json!({"status":"expired"}),
        json!({"status":"waiting_for_message","last_received_at_ms":1,"expires_at_ms":2}),
        json!({"status":"unknown","last_received_at_ms":1}),
        json!({"status":"unknown","expires_at_ms":2}),
        json!({"status":"active","last_received_at_ms":0,"expires_at_ms":2}),
        json!({"status":"active","last_received_at_ms":2,"expires_at_ms":2}),
        json!({"status":"active","last_received_at_ms":3,"expires_at_ms":2}),
    ] {
        let view: NotificationAuthorizationView =
            serde_json::from_value(json!({"status":"authorized","delivery_window":window}))
                .unwrap();
        assert!(view.validate().is_err(), "{window}");
        let input: NotificationRuntimeInput = serde_json::from_value(json!({"operation":{"type":"send","input_payload":{"text":"hello"}},"delivery_window":window})).unwrap();
        assert!(input.validate(&authorizable_descriptor()).is_err());
    }
    for value in [-1.0, 1.5] {
        assert!(serde_json::from_value::<NotificationAuthorizationView>(json!({"status":"authorized","delivery_window":{"status":"active","last_received_at_ms":value,"expires_at_ms":2}})).is_err());
    }
    assert!(serde_json::from_value::<crate::NotificationAuthorizationOperation>(json!({"type":"maintain","delivery_window":{"status":"active","last_received_at_ms":1,"expires_at_ms":2}})).is_err());
    for operation in ["poll", "cancel", "maintain"] {
        let valid = json!({"type":operation});
        let parsed: crate::NotificationAuthorizationOperation =
            serde_json::from_value(valid.clone()).unwrap();
        assert_eq!(serde_json::to_value(parsed).unwrap(), valid);
        for field in ["delivery_window", "managed_secrets", "unexpected"] {
            let mut injected = valid.clone();
            injected[field] = json!({"injected":true});
            assert!(
                serde_json::from_value::<crate::NotificationAuthorizationOperation>(injected)
                    .is_err()
            );
        }
    }
}

#[test]
fn subscription_failures_keep_account_authorized_and_round_trip_fixed_codes() {
    use crate::{NotificationRuntimeInput, NotificationRuntimeResult};
    let descriptor = authorizable_descriptor();
    let input: NotificationRuntimeInput = serde_json::from_value(
        json!({"operation":{"type":"send","input_payload":{"text":"hello"}}}),
    )
    .unwrap();
    for code in [
        "subscription_required",
        "subscription_expired",
        "subscription_unavailable",
    ] {
        for status in ["authorized", "unauthorized", "invalid", "pending"] {
            let authorization = if status == "pending" {
                json!({"status":status,"user_action":{"kind":"qr_code","qr_code_url":"https://example.com/qr","poll_interval_seconds":2}})
            } else {
                json!({"status":status})
            };
            let wire = json!({"type":"send","outcome":{"status":"failed","failure_code":code},"authorization":authorization});
            let result: NotificationRuntimeResult = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(
                result.validate_for(&input, &descriptor).is_ok(),
                status == "authorized",
                "{wire}"
            );
            assert_eq!(serde_json::to_value(result).unwrap(), wire);
        }
    }
}
