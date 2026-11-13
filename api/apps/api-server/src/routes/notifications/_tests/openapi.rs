use super::*;
use control_plane::notifications::{NotificationInstance, NotificationTypeCatalogEntry};
use domain::{NotificationInstanceRecord, NotificationInstanceStatus, SystemNotificationRecord};
use extension_contracts::{
    ContributionId, NotificationAuthorizationAction, NotificationAuthorizationActionKind,
    NotificationAuthorizationDescriptor, NotificationAuthorizationOperation,
    NotificationAuthorizationStatus, NotificationAuthorizationUserAction,
    NotificationAuthorizationView, NotificationDeliveryWindowStatus,
    NotificationDeliveryWindowView, NotificationMcpOperation, NotificationMcpToolDescriptor,
    NotificationReceiveDescriptor, PluginFormOption,
};
use time::OffsetDateTime;

fn schema<T: ToSchema>() -> Value {
    let mut components = Vec::new();
    T::schemas(&mut components);
    components.push((T::name().into_owned(), T::schema()));
    let schemas = components
        .into_iter()
        .map(|(name, value)| (name, serde_json::to_value(value).unwrap()))
        .collect::<serde_json::Map<_, _>>();
    crate::openapi_interface::close_api_event_schema(
        &json!({"components":{"schemas":schemas}}),
        &serde_json::to_value(T::schema()).unwrap(),
    )
}
fn validate<T: ToSchema>(wire: &Value) {
    let schema = schema::<T>();
    assert!(
        jsonschema::validator_for(&schema).unwrap().is_valid(wire),
        "actual wire value rejected: {wire}; schema: {schema}"
    );
}
fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
}

#[test]
fn notification_openapi_matches_real_public_instance_and_authorization_serialization() {
    let authorization = NotificationAuthorizationView {
        delivery_window: Some(NotificationDeliveryWindowView {
            status: NotificationDeliveryWindowStatus::Active,
            last_received_at_ms: Some(100),
            expires_at_ms: Some(200),
        }),
        status: NotificationAuthorizationStatus::Pending,
        user_action: Some(NotificationAuthorizationUserAction {
            kind: NotificationAuthorizationActionKind::QrCode,
            qr_code_url: "https://example.test/qr".into(),
            poll_interval_seconds: 3,
            expires_at: Some("2030-01-01T00:00:00Z".into()),
        }),
    };
    authorization.validate().unwrap();
    validate::<NotificationAuthorizationViewSchema>(&serde_json::to_value(&authorization).unwrap());
    let instance = NotificationInstance {
        record: NotificationInstanceRecord {
            id: Uuid::nil(),
            workspace_id: Uuid::now_v7(),
            type_code: "fixture/chat".into(),
            installation_id: None,
            display_name: "Actual channel".into(),
            status: NotificationInstanceStatus::Enabled,
            config_json: json!({"display_theme":"light"}),
            configured_secret_fields: vec!["token".into()],
            created_at: now(),
            updated_at: now(),
        },
        available: true,
        unavailable_reason: None,
        authorization: Some(authorization),
    };
    let wire = serde_json::to_value(&instance).unwrap();
    validate::<NotificationInstanceSchema>(&wire);
    assert!(wire["installation_id"].is_null());
    assert!(
        wire["created_at"].is_string(),
        "explicit RFC3339 serializer must not be documented as time's default tuple9"
    );
    let mut invalid = wire.clone();
    invalid["authorization"]["user_action"]["poll_interval_seconds"] = json!("3");
    assert!(
        !jsonschema::validator_for(&schema::<NotificationInstanceSchema>())
            .unwrap()
            .is_valid(&invalid)
    );
    let mut no_authorization = instance;
    no_authorization.authorization = None;
    validate::<NotificationInstanceSchema>(&serde_json::to_value(no_authorization).unwrap());
    for operation in [
        NotificationAuthorizationOperation::Begin {
            action: "qr".into(),
        },
        NotificationAuthorizationOperation::Poll,
        NotificationAuthorizationOperation::Cancel,
        NotificationAuthorizationOperation::Maintain,
    ] {
        validate::<NotificationAuthorizationOperationSchema>(
            &serde_json::to_value(operation).unwrap(),
        );
    }
    assert!(
        !jsonschema::validator_for(&schema::<NotificationAuthorizationOperationSchema>())
            .unwrap()
            .is_valid(&json!({"type":"poll","action":"injected"}))
    );
    // The raw request is still the canonical service DTO; no documentation type parses it.
    let _: control_plane::notifications::CreateNotificationInstance = serde_json::from_value(
        json!({"type_code":"system","display_name":"Inbox","config_json":{}}),
    )
    .unwrap();
    validate::<CreateNotificationInstanceSchema>(
        &json!({"type_code":"system","display_name":"Inbox","config_json":{}}),
    );
    let _: control_plane::notifications::PatchNotificationInstance =
        serde_json::from_value(json!({"display_name":null,"status":"disabled","config_json":null}))
            .unwrap();
    validate::<PatchNotificationInstanceSchema>(
        &json!({"display_name":null,"status":"disabled","config_json":null}),
    );
}

#[test]
fn notification_openapi_matches_real_descriptor_forms_locales_and_receive_metadata() {
    let mut descriptor = control_plane::notifications::system_notification_descriptor();
    descriptor.input_form.fields[0]
        .options
        .push(PluginFormOption {
            label: "Member".into(),
            value: json!(Uuid::nil()),
            description: None,
            disabled: Some(false),
        });
    descriptor.i18n.insert("en_US".into(),serde_json::from_value(json!({"display_name":"Inbox","config_form":{"title":"Channel"},"input_form":{"fields":{"user_ids":{"label":"Recipients"}}},"authorization":{"actions":{"qr":{"label":"Scan"}}}})).unwrap());
    descriptor.authorization = Some(NotificationAuthorizationDescriptor {
        actions: vec![NotificationAuthorizationAction {
            code: "qr".into(),
            label: "Scan".into(),
            kind: NotificationAuthorizationActionKind::QrCode,
        }],
        managed_secret_keys: vec!["managed_token".into(), "account_identity".into()],
        managed_identity_keys: vec!["account_identity".into()],
        supports_delivery_window: true,
    });
    descriptor.receive = Some(NotificationReceiveDescriptor {
        mcp_instance_id: None,
        supports_send_receipts: true,
        supports_activity: true,
        contribution_id: ContributionId::new("fixture.receive").unwrap(),
        default_system_prompt: "Answer the incoming conversation".into(),
        mcp_tools: [
            (
                "history_messages",
                NotificationMcpOperation::HistoryMessages,
            ),
            ("history_files", NotificationMcpOperation::HistoryFiles),
            ("read_file", NotificationMcpOperation::ReadFile),
        ]
        .into_iter()
        .map(|(name, operation)| NotificationMcpToolDescriptor {
            name: name.into(),
            description: format!("Invoke {name}"),
            input_schema: json!({"type":"object","properties":{}}),
            operation,
        })
        .collect(),
    });
    descriptor.validate().unwrap();
    let entry = NotificationTypeCatalogEntry {
        descriptor,
        installation_id: Some(Uuid::now_v7()),
        available: true,
        unavailable_reason: None,
    };
    let wire = serde_json::to_value(&entry).unwrap();
    validate::<NotificationTypeCatalogEntrySchema>(&wire);
    assert_eq!(
        wire["authorization"]["managed_identity_keys"],
        json!(["account_identity"])
    );
    let mut absent = entry.clone();
    absent
        .descriptor
        .authorization
        .as_mut()
        .unwrap()
        .managed_identity_keys
        .clear();
    let receive = absent.descriptor.receive.as_mut().unwrap();
    receive.supports_send_receipts = false;
    receive.supports_activity = false;
    let absent = serde_json::to_value(absent).unwrap();
    assert!(absent["authorization"]
        .get("managed_identity_keys")
        .is_none());
    assert!(absent["receive"].get("supports_send_receipts").is_none());
    assert_eq!(absent["receive"]["supports_activity"], json!(false));
    validate::<NotificationTypeCatalogEntrySchema>(&absent);
    let mut invalid_identity = wire.clone();
    invalid_identity["authorization"]["managed_identity_keys"] = json!([17]);
    assert!(
        !jsonschema::validator_for(&schema::<NotificationTypeCatalogEntrySchema>())
            .unwrap()
            .is_valid(&invalid_identity)
    );
    assert!(wire["input_form"]["fields"][0]["control"].is_string());
    assert!(wire["input_form"]["fields"][0]["min"].is_null());
    assert!(wire["input_form"]["fields"][0]["options"][0]["value"].is_string());
    let mut invalid = wire;
    invalid["receive"]["mcp_tools"][0]["operation"] = json!("arbitrary_tool");
    assert!(
        !jsonschema::validator_for(&schema::<NotificationTypeCatalogEntrySchema>())
            .unwrap()
            .is_valid(&invalid)
    );
}

#[test]
fn notification_domain_schema_preserves_rfc3339_and_present_null_fields() {
    let record = SystemNotificationRecord {
        id: Uuid::now_v7(),
        workspace_id: Uuid::now_v7(),
        instance_id: Uuid::nil(),
        recipient_user_id: Uuid::now_v7(),
        title: "Actual title".into(),
        content: "Actual content".into(),
        read_at: Some(now()),
        created_at: now(),
    };
    let wire = serde_json::to_value(&record).unwrap();
    validate::<SystemNotificationRecord>(&wire);
    assert_eq!(
        wire["created_at"],
        now()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    );
    let mut unread = record;
    unread.read_at = None;
    let mut wire = serde_json::to_value(unread).unwrap();
    assert!(wire["read_at"].is_null());
    validate::<SystemNotificationRecord>(&wire);
    wire.as_object_mut().unwrap().remove("read_at");
    assert!(
        !jsonschema::validator_for(&schema::<SystemNotificationRecord>())
            .unwrap()
            .is_valid(&wire)
    );
    wire["read_at"] = Value::Null;
    wire["created_at"] = json!([2027, 15, 0, 0, 0, 0, 0, 0, 0]);
    assert!(
        !jsonschema::validator_for(&schema::<SystemNotificationRecord>())
            .unwrap()
            .is_valid(&wire)
    );
}

#[test]
fn notification_success_counts_null_delete_and_sse_are_declared_as_their_actual_wire_types() {
    validate::<NotificationDeletedSchema>(&Value::Null);
    assert!(
        !jsonschema::validator_for(&schema::<NotificationDeletedSchema>())
            .unwrap()
            .is_valid(&json!({}))
    );
    validate::<super::super::UnreadNotificationCount>(&json!({"unread_count":2}));
    validate::<super::super::UpdatedNotificationCount>(&json!({"updated_count":3}));
    let operation = <super::super::events::__path_events as utoipa::Path>::operation();
    let operation = serde_json::to_value(operation).unwrap();
    assert!(operation["responses"]["200"]["content"]
        .get("text/event-stream")
        .is_some());
    assert!(operation["responses"]["200"]["content"]
        .get("application/json")
        .is_none());
}
