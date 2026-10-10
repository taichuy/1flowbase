use super::*;
use control_plane::i18n_catalog::management::ApplyTemplateTranslationCommand;
use control_plane_contracts::{
    portable_template::{
        template_resource_fingerprint, TemplateBaselineScope, TemplateResourceKey,
        TemplateWriteIntent,
    },
    ports::PortableTemplateBaselineRepository,
};

fn fingerprint(value: &str) -> String {
    template_resource_fingerprint(&serde_json::json!({"translation": value})).unwrap()
}
fn command(fixture: &ManagementFixture, key: &str, value: &str) -> ApplyTemplateTranslationCommand {
    let source_id = serde_json::to_string(&[key, "zh_Hans"]).unwrap();
    ApplyTemplateTranslationCommand {
        access: fixture.root_access(),
        value: translation(key, value),
        expected_revision: fixture.revision,
        scope: TemplateBaselineScope {
            workspace_id: fixture.workspace_id,
            template_id: "i18n-atomic-test".into(),
        },
        key: TemplateResourceKey {
            kind: "i18n_entry".into(),
            source_id: source_id.clone(),
        },
        intent: TemplateWriteIntent {
            operation_id: Uuid::now_v7(),
            target_id: source_id,
            expected_fingerprint: None,
            desired_fingerprint: fingerprint(value),
        },
    }
}
async fn prepare(
    fixture: &ManagementFixture,
    command: &ApplyTemplateTranslationCommand,
    generation: Option<i64>,
) {
    assert!(fixture
        .store
        .prepare_template_write(&command.scope, &command.key, generation, &command.intent)
        .await
        .unwrap());
}
async fn catalog_revision(fixture: &ManagementFixture) -> WorkspaceCatalogRevision {
    fixture
        .store
        .get_workspace_catalog_state(fixture.workspace_id)
        .await
        .unwrap()
        .unwrap()
        .revision()
}
async fn template_audits(fixture: &ManagementFixture) -> i64 {
    sqlx::query_scalar("select count(*) from audit_logs where workspace_id=$1 and event_code='i18n_catalog.template_translation.upserted'")
        .bind(fixture.workspace_id).fetch_one(fixture.store.pool()).await.unwrap()
}
async fn custom_rows(fixture: &ManagementFixture, key: &str) -> Vec<(String, String)> {
    sqlx::query_as("select locale,translation from workspace_i18n_catalog_custom_translations where workspace_id=$1 and key=$2 order by locale")
        .bind(fixture.workspace_id).bind(key).fetch_all(fixture.store.pool()).await.unwrap()
}
async fn assert_pending_without_receipt(
    fixture: &ManagementFixture,
    command: &ApplyTemplateTranslationCommand,
) {
    let baselines = fixture
        .store
        .load_template_baselines(&command.scope)
        .await
        .unwrap();
    let baseline = baselines.iter().find(|row| row.key == command.key).unwrap();
    assert_eq!(baseline.pending.as_ref(), Some(&command.intent));
    assert_eq!(baseline.committed_operation_id, None);
    assert_eq!(baseline.committed_fingerprint, None);
    assert!(!fixture
        .store
        .finalize_template_write(&command.scope, &command.key, command.intent.operation_id)
        .await
        .unwrap());
}
async fn assert_receipt_and_finalize(
    fixture: &ManagementFixture,
    command: &ApplyTemplateTranslationCommand,
) {
    let baselines = fixture
        .store
        .load_template_baselines(&command.scope)
        .await
        .unwrap();
    let baseline = baselines.iter().find(|row| row.key == command.key).unwrap();
    assert_eq!(baseline.pending.as_ref(), Some(&command.intent));
    assert_eq!(
        baseline.committed_operation_id,
        Some(command.intent.operation_id)
    );
    assert_eq!(
        baseline.committed_fingerprint.as_deref(),
        Some(command.intent.desired_fingerprint.as_str())
    );
    assert!(fixture
        .store
        .finalize_template_write(&command.scope, &command.key, command.intent.operation_id)
        .await
        .unwrap());
    let baselines = fixture
        .store
        .load_template_baselines(&command.scope)
        .await
        .unwrap();
    let baseline = baselines.iter().find(|row| row.key == command.key).unwrap();
    assert_eq!(
        baseline.applied_fingerprint.as_deref(),
        Some(command.intent.desired_fingerprint.as_str())
    );
    assert_eq!(baseline.pending, None);
    assert_eq!(baseline.committed_operation_id, None);
    assert!(!fixture
        .store
        .finalize_template_write(&command.scope, &command.key, command.intent.operation_id)
        .await
        .unwrap());
}

#[tokio::test]
async fn template_custom_write_commits_audit_receipt_and_only_explicit_locale() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let command = command(&fixture, "Template greeting", "你好");
    prepare(&fixture, &command, None).await;
    let state = service
        .apply_template_translation(command.clone())
        .await
        .unwrap();
    assert_eq!(state.revision().value(), fixture.revision.value() + 1);
    assert_eq!(
        custom_rows(&fixture, "Template greeting").await,
        vec![("zh_Hans".into(), "你好".into())]
    );
    let audit: (Option<Uuid>, String, String, String, String) = sqlx::query_as(
        "select actor_user_id,payload->>'key',payload->>'locale',payload->>'template_id',payload->>'operation_id' from audit_logs where workspace_id=$1 and event_code='i18n_catalog.template_translation.upserted'"
    ).bind(fixture.workspace_id).fetch_one(fixture.store.pool()).await.unwrap();
    assert_eq!(
        audit,
        (
            Some(fixture.actor.user_id),
            "Template greeting".into(),
            "zh_Hans".into(),
            command.scope.template_id.clone(),
            command.intent.operation_id.to_string()
        )
    );
    assert_eq!(template_audits(&fixture).await, 1);
    assert_receipt_and_finalize(&fixture, &command).await;
}

#[tokio::test]
async fn template_official_translation_updates_override_without_changing_release() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let mut command = command(&fixture, "Settings", "模板设置");
    command.intent.expected_fingerprint = Some(fingerprint("设置"));
    // Explicit historical template baseline fixture; production never adopts existing values.
    sqlx::query("insert into application_template_resource_baselines (id,workspace_id,scope_id,template_id,kind,source_id,target_id,applied_fingerprint) values ($1,$2,$2,$3,$4,$5,$5,$6)")
        .bind(Uuid::now_v7()).bind(fixture.workspace_id).bind(&command.scope.template_id)
        .bind(&command.key.kind).bind(&command.key.source_id).bind(fingerprint("设置"))
        .execute(fixture.store.pool()).await.unwrap();
    let generation = fixture
        .store
        .load_template_baselines(&command.scope)
        .await
        .unwrap()[0]
        .generation;
    prepare(&fixture, &command, Some(generation)).await;
    service
        .apply_template_translation(command.clone())
        .await
        .unwrap();
    let detail = service
        .detail(GetCatalogEntryCommand {
            access: fixture.root_access(),
            identity: identity("Settings"),
            locale: CatalogLocale::new("zh_Hans").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(detail.official_translation.as_deref(), Some("设置"));
    assert_eq!(detail.override_translation.as_deref(), Some("模板设置"));
    assert_eq!(detail.origin, CatalogManagementOrigin::OfficialOverride);
    assert!(custom_rows(&fixture, "Settings").await.is_empty());
    assert_eq!(template_audits(&fixture).await, 1);
    assert_receipt_and_finalize(&fixture, &command).await;
}

#[tokio::test]
async fn template_stale_revision_and_current_fingerprint_do_not_mutate_catalog() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let initial = command(&fixture, "Template greeting", "初始");
    prepare(&fixture, &initial, None).await;
    let initial_state = service
        .apply_template_translation(initial.clone())
        .await
        .unwrap();
    assert_receipt_and_finalize(&fixture, &initial).await;
    let generation = fixture
        .store
        .load_template_baselines(&initial.scope)
        .await
        .unwrap()[0]
        .generation;
    let mut update = command(&fixture, "Template greeting", "新版");
    update.expected_revision = initial_state.revision();
    update.intent.expected_fingerprint = Some(fingerprint("初始"));
    prepare(&fixture, &update, Some(generation)).await;
    let edited = service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: translation("Template greeting", "用户修改"),
            expected_revision: initial_state.revision(),
        })
        .await
        .unwrap();
    let before_rows = custom_rows(&fixture, "Template greeting").await;
    let before_audits: i64 =
        sqlx::query_scalar("select count(*) from audit_logs where workspace_id=$1")
            .bind(fixture.workspace_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    // The first attempt is rejected by revision; the second by actual content CAS.
    let error = service
        .apply_template_translation(update.clone())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("i18n_catalog_revision"));
    update.expected_revision = edited.revision();
    let error = service
        .apply_template_translation(update.clone())
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("template_i18n_current_fingerprint"));
    assert_eq!(catalog_revision(&fixture).await, edited.revision());
    assert_eq!(
        custom_rows(&fixture, "Template greeting").await,
        before_rows
    );
    assert_eq!(template_audits(&fixture).await, 1);
    let after_audits: i64 =
        sqlx::query_scalar("select count(*) from audit_logs where workspace_id=$1")
            .bind(fixture.workspace_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!(after_audits, before_audits);
    assert_pending_without_receipt(&fixture, &update).await;
}

#[tokio::test]
async fn template_missing_or_wrong_journal_receipt_rolls_back_translation_revision_and_audit() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let missing = command(&fixture, "Missing receipt", "不应提交");
    let error = service
        .apply_template_translation(missing.clone())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("template_i18n_receipt"));
    assert!(fixture
        .store
        .load_template_baselines(&missing.scope)
        .await
        .unwrap()
        .is_empty());
    assert!(custom_rows(&fixture, "Missing receipt").await.is_empty());
    assert_eq!(catalog_revision(&fixture).await, fixture.revision);
    assert_eq!(template_audits(&fixture).await, 0);

    let prepared = command(&fixture, "Wrong receipt", "不应提交");
    prepare(&fixture, &prepared, None).await;
    let mut wrong_operation = prepared.clone();
    wrong_operation.intent.operation_id = Uuid::now_v7();
    let error = service
        .apply_template_translation(wrong_operation)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("template_i18n_receipt"));
    assert!(custom_rows(&fixture, "Wrong receipt").await.is_empty());
    assert_eq!(catalog_revision(&fixture).await, fixture.revision);
    assert_eq!(template_audits(&fixture).await, 0);
    assert_pending_without_receipt(&fixture, &prepared).await;
    // Failed receipt attempts left the valid prepared operation usable.
    service
        .apply_template_translation(prepared.clone())
        .await
        .unwrap();
    assert_receipt_and_finalize(&fixture, &prepared).await;
}
