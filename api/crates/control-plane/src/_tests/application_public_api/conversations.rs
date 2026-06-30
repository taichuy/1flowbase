use control_plane::application_public_api::{
    api_keys::{ApplicationApiKeyService, CreateApplicationApiKeyCommand},
    mapping::{
        ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
    },
    native::{ApplicationNativeRunService, CreateNativeRunCommand},
    publications::{ApplicationPublicationService, PublishApplicationCommand},
    ApplicationPublicApiTestHarness,
};
use serde_json::json;
use uuid::Uuid;

fn actor_user_id() -> Uuid {
    Uuid::from_u128(0x11111111111111111111111111111111)
}

fn other_user_id() -> Uuid {
    Uuid::from_u128(0x22222222222222222222222222222222)
}

fn published_mapping() -> ApplicationApiMappingConfig {
    ApplicationApiMappingConfig {
        input: ApplicationApiMappingInput {
            query_target: "node-start.query".into(),
            model_target: None,
            inputs_target: None,
            history_target: None,
            attachments_target: None,
        },
        output: ApplicationApiMappingOutput::default(),
        extension: None,
    }
}

async fn issue_key(
    harness: &ApplicationPublicApiTestHarness,
    application_id: Uuid,
    owner_user_id: Uuid,
) -> String {
    ApplicationApiKeyService::new(harness.repository())
        .create_api_key(CreateApplicationApiKeyCommand {
            actor_user_id: owner_user_id,
            application_id,
            name: "Native runner".into(),
            expires_at: None,
        })
        .await
        .unwrap()
        .token
}

async fn publish_application(
    harness: &ApplicationPublicApiTestHarness,
    application_id: Uuid,
    owner_user_id: Uuid,
) {
    ApplicationPublicationService::new(harness.repository())
        .publish_active_version(PublishApplicationCommand {
            actor_user_id: owner_user_id,
            application_id,
            mapping: published_mapping(),
            api_enabled: true,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn native_run_generates_external_conversation_id_when_missing() {
    let harness = ApplicationPublicApiTestHarness::new();
    let application = harness.seed_application(actor_user_id(), "Generated Conversation App");
    let token = issue_key(&harness, application.id, actor_user_id()).await;
    publish_application(&harness, application.id, actor_user_id()).await;
    let service = ApplicationNativeRunService::new(harness.repository());

    let run = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: token,
            request: serde_json::from_value(json!({
                "query": "Continue",
                "conversation": {
                    "user": "customer-1"
                },
                "response_mode": "blocking"
            }))
            .unwrap(),
        })
        .await
        .unwrap();

    let generated_id = run.metadata["external_conversation_id"]
        .as_str()
        .expect("generated conversation id should be returned");
    assert!(generated_id.starts_with("conv_"));
    assert_eq!(
        run.metadata["request"]["conversation"]["id"],
        json!(generated_id)
    );
    assert_eq!(
        run.metadata["request"]["conversation"]["user"],
        json!("customer-1")
    );
}

#[tokio::test]
async fn native_run_conversation_binding_is_scoped_to_application_and_api_key() {
    let harness = ApplicationPublicApiTestHarness::new();
    let first_application = harness.seed_application(actor_user_id(), "First Conversation App");
    let second_application = harness.seed_application(other_user_id(), "Second Conversation App");
    let first_token = issue_key(&harness, first_application.id, actor_user_id()).await;
    let second_token = issue_key(&harness, second_application.id, other_user_id()).await;
    publish_application(&harness, first_application.id, actor_user_id()).await;
    publish_application(&harness, second_application.id, other_user_id()).await;
    let service = ApplicationNativeRunService::new(harness.repository());

    let first = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: first_token,
            request: serde_json::from_value(json!({
                "query": "Continue",
                "conversation": {
                    "id": "shared-external-id",
                    "user": "customer-1"
                }
            }))
            .unwrap(),
        })
        .await
        .unwrap();
    let second = service
        .create_native_run(CreateNativeRunCommand {
            bearer_token: second_token,
            request: serde_json::from_value(json!({
                "query": "Continue",
                "conversation": {
                    "id": "shared-external-id",
                    "user": "customer-1"
                }
            }))
            .unwrap(),
        })
        .await
        .unwrap();

    assert_eq!(
        first.metadata["external_conversation_id"],
        json!("shared-external-id")
    );
    assert_eq!(
        second.metadata["external_conversation_id"],
        json!("shared-external-id")
    );
    assert_ne!(
        harness
            .repository()
            .conversation_record_id_for_run(first.id),
        harness
            .repository()
            .conversation_record_id_for_run(second.id)
    );
}
