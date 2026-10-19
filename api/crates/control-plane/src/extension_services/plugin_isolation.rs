#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIsolationPolicy {
    pub process_model: &'static str,
    pub secret_scope: &'static str,
    pub network_scope: &'static str,
    pub file_scope: &'static str,
    pub db_write: &'static str,
    pub host_callback: &'static str,
    pub can_register_route: bool,
    pub can_execute_host_tool: bool,
    pub approval_required: &'static str,
    pub rate_limit_required: bool,
}

pub fn policy_for(kind: &str) -> Option<RuntimeIsolationPolicy> {
    match kind {
        "model_provider_plugin" => Some(RuntimeIsolationPolicy {
            process_model: "process_per_call_or_worker",
            secret_scope: "provider_scoped",
            network_scope: "provider_endpoint",
            file_scope: "temp_dir_only",
            db_write: "none",
            host_callback: "provider_event_only",
            can_register_route: false,
            can_execute_host_tool: false,
            approval_required: "provider_policy",
            rate_limit_required: true,
        }),
        "capability_plugin" => Some(RuntimeIsolationPolicy {
            process_model: "capability_runtime",
            secret_scope: "capability_scoped",
            network_scope: "declared_capability",
            file_scope: "declared_capability",
            db_write: "host_api_only",
            host_callback: "capability_result",
            can_register_route: false,
            can_execute_host_tool: true,
            approval_required: "optional_or_forced",
            rate_limit_required: true,
        }),
        "external_agent_bridge" => Some(RuntimeIsolationPolicy {
            process_model: "ingress_only",
            secret_scope: "bridge_token",
            network_scope: "ingest_only",
            file_scope: "none",
            db_write: "host_api_only",
            host_callback: "telemetry_ingest",
            can_register_route: false,
            can_execute_host_tool: false,
            approval_required: "not_applicable",
            rate_limit_required: true,
        }),
        _ => None,
    }
}
