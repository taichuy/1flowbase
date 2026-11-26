use crate::managed_services::ManagedServiceRegistration;
use serde_json::json;
pub fn fixture() -> ManagedServiceRegistration {
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
