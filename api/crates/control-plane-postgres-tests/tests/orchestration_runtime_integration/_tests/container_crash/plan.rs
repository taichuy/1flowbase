use super::*;
use orchestration_runtime::compiler::{
    ensure_plan_execution_contract, FlowCompileContext, FlowCompileProviderFamily,
    FlowCompileProviderInstance, FlowCompiler,
};
use std::collections::BTreeSet;

pub(super) async fn prepare(
    store: &PgControlPlaneStore,
    compiled: &domain::CompiledPlanRecord,
    break_case: bool,
    logs: &std::path::Path,
) {
    let seeded: Value = sqlx::query_scalar("select plan from flow_compiled_plans where id=$1")
        .bind(compiled.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let llm = &seeded["nodes"]["node-llm"]["llm_runtime"];
    let provider = llm["provider_code"].as_str().unwrap();
    let model = llm["model"].as_str().unwrap();
    let protocol = llm["protocol"].as_str().unwrap();
    let instance = llm["provider_instance_id"].as_str().unwrap();
    let mut context = FlowCompileContext::default();
    context.provider_families.insert(
        provider.into(),
        FlowCompileProviderFamily {
            provider_code: provider.into(),
            protocol: protocol.into(),
            is_ready: true,
            available_models: BTreeSet::from([model.into()]),
            allow_custom_models: false,
        },
    );
    context.provider_instances.insert(
        instance.into(),
        FlowCompileProviderInstance {
            provider_instance_id: instance.into(),
            display_name: "Crash fixture".into(),
            provider_code: provider.into(),
            protocol: protocol.into(),
            is_ready: true,
            is_runnable: true,
            included_in_main: true,
            available_models: BTreeSet::from([model.into()]),
            allow_custom_models: false,
        },
    );
    let document = json!({"schemaVersion":"1flowbase.flow/v2","graph":{
        "nodes":[
            {"id":"node-start","type":"start","alias":"Start","config":{},"bindings":{},"outputs":[]},
            {"id":"loop","type":"loop","alias":"Bounded crash matrix","config":{
                "max_rounds":2,"round_timeout_ms":120000,"parallelism":1,
                "error_policy":if break_case {"break"} else {"fail"}
            },"bindings":{"output":{"kind":"selector","value":["node-llm","text"]}},
            "outputs":[{"key":"result","title":"Result","valueType":"array"},{"key":"variables","title":"Variables","valueType":"object"}]},
            {"id":"loop-start","type":"container_start","alias":"Round start","containerId":"loop","config":{},"bindings":{},"outputs":[]},
            {"id":"node-llm","type":"llm","alias":"External effect","containerId":"loop",
                "config":{"model_provider":{"provider_code":provider,"model_id":model}},
                "bindings":{"prompt_messages":{"kind":"prompt_messages","value":[
                    {"id":"user","role":"user","content":{"kind":"templated_text","value":"Record this bounded execution."}}
                ]}},"outputs":[{"key":"text","title":"Text","valueType":"string"}]}
        ],"edges":[
            {"id":"root-loop","source":"node-start","target":"loop"},
            {"id":"round-leaf","source":"loop-start","target":"node-llm","containerId":"loop"}
        ]
    }});
    let result = FlowCompiler::compile(
        compiled.flow_id,
        &compiled.draft_id.to_string(),
        &document,
        &context,
    );
    let evidence = match &result {
        Ok(plan) => json!({"document":document,"plan":plan,"compile_issues":plan.compile_issues}),
        Err(error) => json!({"document":document,"compile_error":format!("{error:#}")}),
    };
    std::fs::write(
        logs.join("plan-validation.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    let plan = result
        .expect("crash fixture authoring document must compile; inspect plan-validation.json");
    assert!(
        plan.compile_issues.is_empty(),
        "crash fixture compile issues: {:?}",
        plan.compile_issues
    );
    // This is the exact public validator called by control-plane's private
    // ensure_compiled_plan_runnable, after its compile_issues guard.
    ensure_plan_execution_contract(&plan)
        .expect("crash fixture execution contract must pass before spawning children");
    sqlx::query("update flow_compiled_plans set plan=$2 where id=$1")
        .bind(compiled.id)
        .bind(serde_json::to_value(plan).unwrap())
        .execute(store.pool())
        .await
        .unwrap();
}
