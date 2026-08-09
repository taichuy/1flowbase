use control_plane::ports::{
    CreateUiCodeTemplateInput, ReviseUiCodeTemplateInput, ReviseUiComponentContractInput,
    UiManagementRepository,
};
use domain::{
    FrontendComponentContract, FrontendComponentExample, UiCodeTemplateLanguage,
    UiComponentLocator, UiComponentOverrideState,
};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn store() -> PgControlPlaneStore {
    let database = postgres_test_support::PostgresTestSchema::create(&database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

#[tokio::test]
async fn ac_002_template_revisions_publish_and_default_are_transactional() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let created = store
        .create_ui_code_template(&CreateUiCodeTemplateInput {
            provider_code: "1flowbase".into(),
            contribution_code: "frontstage.js-ui-block".into(),
            name: "Dashboard".into(),
            source: "export default function Block() { return null }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(created.latest_revision.revision, 1);
    assert!(store
        .set_ui_code_template_default(created.id, actor)
        .await
        .is_err());

    let revised = store
        .revise_ui_code_template(&ReviseUiCodeTemplateInput {
            template_id: created.id,
            name: "Dashboard".into(),
            source: "export default function Block() { return <main /> }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(revised.latest_revision.revision, 2);
    let published = store
        .publish_ui_code_template_revision(created.id, 2, actor)
        .await
        .unwrap();
    assert_eq!(published.published_revision.unwrap().revision, 2);
    store
        .set_ui_code_template_default(created.id, actor)
        .await
        .unwrap();
    assert!(
        store
            .get_ui_code_template(created.id)
            .await
            .unwrap()
            .unwrap()
            .is_default
    );
}

#[tokio::test]
async fn ac_004_component_requires_revision_before_publish_and_preserves_hidden_state() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let locator = UiComponentLocator {
        provider_code: "1flowbase".into(),
        contribution_code: "frontstage.js-ui-block".into(),
        module_source: "antd".into(),
        export_name: "Button".into(),
    };
    assert!(store
        .set_ui_component_state(&locator, UiComponentOverrideState::Published, actor)
        .await
        .is_err());
    store
        .revise_ui_component_contract(&ReviseUiComponentContractInput {
            locator: locator.clone(),
            contract: FrontendComponentContract {
                component_code: "button".into(),
                export_name: "Button".into(),
                upstream: None,
                description: "Action button".into(),
                props: vec![],
                limitations: vec!["Registered export only".into()],
                examples: vec![FrontendComponentExample {
                    title: "Save".into(),
                    code: "<Button>Save</Button>".into(),
                }],
                insert_snippet: "<Button>Save</Button>".into(),
            },
            actor_user_id: actor,
        })
        .await
        .unwrap();
    let published = store
        .set_ui_component_state(&locator, UiComponentOverrideState::Published, actor)
        .await
        .unwrap();
    assert!(published.published_revision.is_some());
    let hidden = store
        .set_ui_component_state(&locator, UiComponentOverrideState::Hidden, actor)
        .await
        .unwrap();
    assert_eq!(hidden.state, UiComponentOverrideState::Hidden);
    assert!(hidden.published_revision.is_some());
}
