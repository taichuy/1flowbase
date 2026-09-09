use std::sync::Arc;

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::callable_interfaces::FrontstageInterfaceCapabilityQuery;
use crate::{
    error_response::ApiError,
    openapi_interface::{
        OpenApiCapabilityCatalogDependencies, OpenApiCapabilityCatalogEntry,
        OpenApiCapabilityCatalogPage, OpenApiCapabilityCatalogQuery, get_openapi_capability_with,
        query_openapi_capability_catalog_with,
    },
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

const INTERFACE_CAPABILITY_PAGE_SIZE: usize = 20;

pub(crate) enum FrontstageCallableCatalogInput {
    List(FrontstageInterfaceCapabilityQuery),
    Get { interface_id: String },
}

impl InterfaceContract for FrontstageCallableCatalogInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "path_prefixes",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "path_query",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "adapter_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "method",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                ("interface_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "path_prefixes",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).path_prefixes).len()),
                            )]),
                        ),
                        (
                            "path_query",
                            match (&(_field_0).path_query).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "adapter_id",
                            match (&(_field_0).adapter_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "method",
                            match (&(_field_0).method).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "offset",
                            match (&(_field_0).offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_0).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Get {
                interface_id: _field_interface_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Get".to_owned())),
                ("interface_id", mp::text(_field_interface_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-frontstage-callable-catalog-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed catalog output is projected immediately into the frontstage response"
)]
pub(crate) enum FrontstageCallableCatalogOutput {
    Page(OpenApiCapabilityCatalogPage),
    Entry(OpenApiCapabilityCatalogEntry),
}

impl InterfaceContract for FrontstageCallableCatalogOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Page")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("interface_id",mp::text_schema()), ("method",mp::text_schema()), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("StaticApiDocs"))]), mp::object_schema(&[("variant",mp::tag_schema("ActivatedInterfaceOperation"))]), mp::object_schema(&[("variant",mp::tag_schema("BuiltinDataModelCrud"))]), mp::object_schema(&[("variant",mp::tag_schema("WorkspaceDataModelCrud"))]), mp::object_schema(&[("variant",mp::tag_schema("PublishedWorkflow"))])]))])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("offset", serde_json::json!({"type":"integer"})),
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                        (
                            "next_offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "adapter_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "methods",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Entry")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "interface",
                            mp::object_schema(&[
                                ("operation_id", mp::text_schema()),
                                ("method", mp::text_schema()),
                                (
                                    "path",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "description",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "parameter_descriptors",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                ("request_schema", mp::json_summary_schema()),
                                ("response_schema", mp::json_summary_schema()),
                                (
                                    "request_media_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "response_media_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                ("security", mp::json_summary_schema()),
                            ]),
                        ),
                        (
                            "source",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("StaticApiDocs"))]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("ActivatedInterfaceOperation"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("BuiltinDataModelCrud"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("WorkspaceDataModelCrud"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("PublishedWorkflow"),
                                )]),
                            ]),
                        ),
                        (
                            "risk_level",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("bindable", serde_json::json!({"type":"boolean"})),
                        (
                            "disabled_reason",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "activated_operation",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("operation_id",mp::text_schema()), ("input_contract_id",mp::text_schema()), ("input_contract_version",mp::text_schema()), ("output_contract_id",mp::text_schema()), ("output_contract_version",mp::text_schema()), ("required_core_permission",mp::object_schema(&[("byte_count",mp::count_schema())])), ("auth_policy",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Anonymous"))]), mp::object_schema(&[("variant",mp::tag_schema("Authenticated"))])])), ("audit_policy",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("ReadOnly"))]), mp::object_schema(&[("variant",mp::tag_schema("Mutating"))])])), ("error_policy",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("TypedTarget"))])])), ("graph_fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("registry_fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("owner",mp::object_schema(&[("byte_count",mp::count_schema())]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Page(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Page".to_owned())), ("0",mp::object_value(&[("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("interface_id",mp::text(&(item).interface_id)?), ("method",mp::text(&(item).method)?), ("path",mp::object_value(&[("byte_count",serde_json::json!((&(item).path).len()))])), ("source",match &(item).source {crate::openapi_interface::OpenApiCapabilitySource::StaticApiDocs => mp::object_value(&[("variant",serde_json::Value::String("StaticApiDocs".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::ActivatedInterfaceOperation => mp::object_value(&[("variant",serde_json::Value::String("ActivatedInterfaceOperation".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::BuiltinDataModelCrud => mp::object_value(&[("variant",serde_json::Value::String("BuiltinDataModelCrud".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::WorkspaceDataModelCrud => mp::object_value(&[("variant",serde_json::Value::String("WorkspaceDataModelCrud".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::PublishedWorkflow => mp::object_value(&[("variant",serde_json::Value::String("PublishedWorkflow".to_owned()))])})]))).collect::<Option<Vec<_>>>()?) }), ("total",serde_json::json!(*(&(_field_0).total))), ("offset",serde_json::json!(*(&(_field_0).offset))), ("limit",serde_json::json!(*(&(_field_0).limit))), ("has_more",serde_json::Value::Bool(*(&(_field_0).has_more))), ("next_offset",match (&(_field_0).next_offset).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("adapter_ids",{ if (&(_field_0).adapter_ids).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).adapter_ids).iter().map(|item| Some(mp::text(item)?)).collect::<Option<Vec<_>>>()?) }), ("methods",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).methods).len()))]))]))]), Self::Entry(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Entry".to_owned())), ("0",mp::object_value(&[("interface",mp::object_value(&[("operation_id",mp::text(&(&(_field_0).interface).operation_id)?), ("method",mp::text(&(&(_field_0).interface).method)?), ("path",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).interface).path).len()))])), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).interface).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).interface).description).len()))])), ("parameter_descriptors",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).interface).parameter_descriptors).len()))])), ("request_schema",mp::json_summary(&(&(_field_0).interface).request_schema)), ("response_schema",mp::json_summary(&(&(_field_0).interface).response_schema)), ("request_media_type",match (&(&(_field_0).interface).request_media_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("response_media_type",match (&(&(_field_0).interface).response_media_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("security",mp::json_summary(&(&(_field_0).interface).security))])), ("source",match &(_field_0).source {crate::openapi_interface::OpenApiCapabilitySource::StaticApiDocs => mp::object_value(&[("variant",serde_json::Value::String("StaticApiDocs".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::ActivatedInterfaceOperation => mp::object_value(&[("variant",serde_json::Value::String("ActivatedInterfaceOperation".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::BuiltinDataModelCrud => mp::object_value(&[("variant",serde_json::Value::String("BuiltinDataModelCrud".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::WorkspaceDataModelCrud => mp::object_value(&[("variant",serde_json::Value::String("WorkspaceDataModelCrud".to_owned()))]), crate::openapi_interface::OpenApiCapabilitySource::PublishedWorkflow => mp::object_value(&[("variant",serde_json::Value::String("PublishedWorkflow".to_owned()))])}), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).risk_level).len()))])), ("bindable",serde_json::Value::Bool(*(&(_field_0).bindable))), ("disabled_reason",match (&(_field_0).disabled_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("activated_operation",match (&(_field_0).activated_operation).as_ref() { Some(item) => mp::object_value(&[("operation_id",mp::text(&(item).operation_id)?), ("input_contract_id",mp::text(&(item).input_contract_id)?), ("input_contract_version",mp::text(&(item).input_contract_version)?), ("output_contract_id",mp::text(&(item).output_contract_id)?), ("output_contract_version",mp::text(&(item).output_contract_version)?), ("required_core_permission",mp::object_value(&[("byte_count",serde_json::json!((&(item).required_core_permission).len()))])), ("auth_policy",match &(item).auth_policy {interface_runtime::registry::InterfaceAuthenticationPolicy::Anonymous => mp::object_value(&[("variant",serde_json::Value::String("Anonymous".to_owned()))]), interface_runtime::registry::InterfaceAuthenticationPolicy::Authenticated => mp::object_value(&[("variant",serde_json::Value::String("Authenticated".to_owned()))])}), ("audit_policy",match &(item).audit_policy {interface_runtime::registry::InterfaceAuditPolicy::ReadOnly => mp::object_value(&[("variant",serde_json::Value::String("ReadOnly".to_owned()))]), interface_runtime::registry::InterfaceAuditPolicy::Mutating => mp::object_value(&[("variant",serde_json::Value::String("Mutating".to_owned()))])}), ("error_policy",match &(item).error_policy {interface_runtime::registry::InterfaceErrorPolicy::TypedTarget => mp::object_value(&[("variant",serde_json::Value::String("TypedTarget".to_owned()))])}), ("graph_fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(item).graph_fingerprint).len()))])), ("registry_fingerprint",mp::object_value(&[("byte_count",serde_json::json!((&(item).registry_fingerprint).len()))])), ("owner",mp::object_value(&[("byte_count",serde_json::json!((&(item).owner).len()))]))]), None => serde_json::Value::Null })]))])})
    }

    const CONTRACT_ID: &'static str = "console-frontstage-callable-catalog-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[derive(Clone)]
pub(crate) struct FrontstageCallableCatalogDependencies {
    pub(crate) openapi: OpenApiCapabilityCatalogDependencies,
}

struct FrontstageCallableCatalogAdapter(FrontstageCallableCatalogDependencies);

impl FrontstageCallableCatalogAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: FrontstageCallableCatalogInput,
    ) -> Result<FrontstageCallableCatalogOutput, ApiError> {
        let actor = principal.actor();
        if !actor.has_permission("frontstage.page.design") {
            return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
                "frontstage.page.design",
            )
            .into());
        }
        match input {
            FrontstageCallableCatalogInput::List(query) => {
                Ok(FrontstageCallableCatalogOutput::Page(
                    query_openapi_capability_catalog_with(
                        &self.0.openapi,
                        actor.current_workspace_id,
                        OpenApiCapabilityCatalogQuery {
                            path_prefixes: query.path_prefixes,
                            path_query: query.path_query,
                            adapter_id: query.adapter_id,
                            method: query.method,
                            offset: query.offset.unwrap_or(0),
                            limit: query
                                .limit
                                .unwrap_or(INTERFACE_CAPABILITY_PAGE_SIZE)
                                .clamp(1, INTERFACE_CAPABILITY_PAGE_SIZE),
                        },
                    )
                    .await?,
                ))
            }
            FrontstageCallableCatalogInput::Get { interface_id } => {
                let entry = get_openapi_capability_with(
                    &self.0.openapi,
                    actor.current_workspace_id,
                    &interface_id,
                )
                .await?
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "frontstage_interface_capability",
                ))?;
                Ok(FrontstageCallableCatalogOutput::Entry(entry))
            }
        }
    }
}

impl ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>
    for FrontstageCallableCatalogAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: FrontstageCallableCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageCallableCatalogOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) fn port(
    dependencies: FrontstageCallableCatalogDependencies,
) -> Arc<dyn ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>>
{
    Arc::new(FrontstageCallableCatalogAdapter(dependencies))
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.callable_interfaces.list",
        binding_id: "http.console.frontstage.interface-capabilities.list.get.v1",
        method: "GET",
        path: "/api/console/frontstage/interface-capabilities",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "frontstage.callable_interfaces.view",
        binding_id: "http.console.frontstage.interface-capabilities.detail.get.v1",
        method: "GET",
        path: "/api/console/frontstage/interface-capabilities/:interface_id",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    port: Arc<
        dyn ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>,
    >,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-frontstage-callable-catalog",
        "graph:console-frontstage-callable-catalog-v1",
        DECLARATIONS,
        port,
    )
}

#[cfg(test)]
struct UnavailableFrontstageCallableCatalogPort;

#[cfg(test)]
impl ConsoleInterfacePort<FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput>
    for UnavailableFrontstageCallableCatalogPort
{
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: FrontstageCallableCatalogInput,
    ) -> ConsoleInterfaceFuture<'a, FrontstageCallableCatalogOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("frontstage callable catalog fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f12c1_registry_freezes_callable_catalog_bindings() {
        let registry =
            compile_registry(Arc::new(UnavailableFrontstageCallableCatalogPort)).unwrap();
        for declaration in DECLARATIONS {
            let binding = registry
                .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
                .expect("declared callable catalog binding must be frozen");
            let route = binding.projection().http_route().unwrap();
            assert_eq!(route.method(), declaration.method);
            assert_eq!(route.path(), declaration.path);
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
