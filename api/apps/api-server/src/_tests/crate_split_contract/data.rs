//! Fixed nonempty cases from managed_services/_tests/mod.rs and the PostgreSQL
//! application_public_api_repository publication fixture. No HTTP sign-in.
use crate::{app_state::ApiState, managed_services::ManagedServiceRegistration};
use anyhow::Result;
use serde_json::{Value, json};
use uuid::Uuid;

pub(super) fn managed_service() -> ManagedServiceRegistration {
    ManagedServiceRegistration { root_workspace_only: false, resource_imports: Vec::new(), publisher_namespace: "test".into(), data_models: Vec::new(),
        display_name: "Fixture plugin".into(), description: "Fixture plugin operations".into(),
        installation_id: uuid::Uuid::nil(), plugin_code:"fixture".into(), plugin_version:"1.0.0".into(),
        declaration:serde_json::from_value(json!({"scope":"system","feature":{"feature_id":"fixture.settings","label":"Fixture service","description":"Configure fixture service","route_id":"fixture","path":"/settings/fixture"},"operations":[{
            "interface_id":"fixture.list","contribution_id":"fixture.list","method":"GET","path":"/api/console/managed-services/fixture/items","summary":"List fixture items","description":"List shared fixture items.","input_schema":{"type":"object","properties":{"path":{"type":"object"},"query":{"type":"object"},"body":{"type":"object"}}},"output_schema":{"type":"object"}
        }]})).unwrap(),
        pages:vec![plugin_framework::PluginSettingsPageManifest { code_modules: Vec::new(),
feature_id:"fixture.settings".into(), contribution_code:"settings".into(), source_file:"ui/Settings.tsx".into(), language:"tsx".into() }],
    }
}

// Keep all IDs that appear in the exported contract fixed across fresh databases.
pub(super) async fn seed_publication(state: &ApiState) -> Result<Value> {
    let pool = state.store.pool();
    let actor: Uuid = sqlx::query_scalar("select id from users where account = 'root'")
        .fetch_one(pool)
        .await?;
    let application_id = Uuid::from_u128(0x11111111111111111111111111111111);
    let flow_id = Uuid::from_u128(0x44444444444444444444444444444444);
    let draft_id = Uuid::from_u128(0x88888888888888888888888888888888);
    let version_id = Uuid::from_u128(0x55555555555555555555555555555555);
    let plan_id = Uuid::from_u128(0x66666666666666666666666666666666);
    let document = json!({"graph":{"nodes":[
        {"id":"node-workflow-start","type":"workflow_start","config":{"input_fields":[
            {"key":"order_id","valueType":"string","source":"path","required":true}
        ]}},
        {"id":"node-workflow-end","type":"workflow_end","outputs":[{"key":"accepted","valueType":"boolean"}]}
    ]}});
    let mapping = json!({
        "input":{"query_target":"node-workflow-start.query","model_target":null,"inputs_target":null,"history_target":null,"attachments_target":null},
        "output":{"answer_selector":null,"usage_selector":null,"files_selector":null,"error_selector":null},
        "extension":{"slug":"crate-split/orders/{order_id}","method":"POST","response_mode":"sync"}
    });
    sqlx::query("insert into applications (id, workspace_id, application_type, workflow_trigger_type, name, description, created_by, updated_by) values ($1,$2,'workflow','extension','Crate split workflow','Fixed OpenAPI publication',$3,$3)")
        .bind(application_id).bind(state.bootstrap_workspace_id).bind(actor).execute(pool).await?;
    sqlx::query("insert into flows (id,application_id,scope_id,created_by,updated_by) values ($1,$2,$3,$4,$4)")
        .bind(flow_id).bind(application_id).bind(state.bootstrap_workspace_id).bind(actor).execute(pool).await?;
    sqlx::query("insert into flow_drafts (id,flow_id,scope_id,schema_version,document,created_by,updated_by) values ($1,$2,$3,$4,$5,$6,$6)")
        .bind(draft_id).bind(flow_id).bind(state.bootstrap_workspace_id).bind(domain::FLOW_SCHEMA_VERSION).bind(&document).bind(actor).execute(pool).await?;
    sqlx::query("insert into flow_versions (id,flow_id,scope_id,sequence,trigger,change_kind,summary,summary_is_custom,is_user_protected,document,created_by,updated_by) values ($1,$2,$3,1,'autosave','logical','published',false,true,$4,$5,$5)")
        .bind(version_id).bind(flow_id).bind(state.bootstrap_workspace_id).bind(&document).bind(actor).execute(pool).await?;
    sqlx::query("insert into flow_compiled_plans (id,flow_id,flow_draft_id,schema_version,document_hash,document_updated_at,plan,scope_id,created_by,updated_by) select $1,$2,$3,$4,'sha256:crate-split',updated_at,$5,$6,$7,$7 from flow_drafts where id=$3")
        .bind(plan_id).bind(flow_id).bind(draft_id).bind(domain::FLOW_SCHEMA_VERSION).bind(json!({"schema_version":domain::FLOW_SCHEMA_VERSION})).bind(state.bootstrap_workspace_id).bind(actor).execute(pool).await?;
    sqlx::query("insert into application_publication_versions (id,application_id,scope_id,flow_id,flow_version_id,compiled_plan_id,extension_slug,version_sequence,active,api_enabled,flow_schema_version,document_hash,document_snapshot,mapping_snapshot,runtime_profile_snapshot,output_selector,dependency_snapshot,created_by,updated_by,created_at,updated_at) values ($1,$2,$3,$4,$5,$6,'crate-split/orders/{order_id}',1,true,true,$7,'sha256:crate-split',$8,$9,'{}','{}','[]',$10,$10,'2026-01-01T00:00:00Z','2026-01-01T00:00:00Z')")
        .bind(Uuid::from_u128(0x22222222222222222222222222222222)).bind(application_id).bind(state.bootstrap_workspace_id).bind(flow_id).bind(version_id).bind(plan_id).bind(domain::FLOW_SCHEMA_VERSION).bind(&document).bind(&mapping).bind(actor).execute(pool).await?;
    Ok(json!({"application_id":application_id,"document":document,"mapping":mapping}))
}
