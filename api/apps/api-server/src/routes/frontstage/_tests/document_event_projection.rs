use super::*;
use crate::openapi_interface::contract::{ApiInput, ApiOperationContext, ApiOutput};
use crate::routes::console_interface::ConsoleLocaleHints;
use crate::routes::frontstage::{
    block_tree::{
        interface::{FrontstageBlocksInput, FrontstageBlocksOutput},
        FrontstageBlockNodeCodeResponse,
    },
    callable_interface_catalog::{FrontstageCallableCatalogInput, FrontstageCallableCatalogOutput},
    callable_interface_dispatch::{
        FrontstageCallableDispatchInput, FrontstageCallableDispatchOutput,
    },
    components::FrontstageComponentsOutput,
    interface_pages::{FrontstagePagesInput, FrontstagePagesOutput},
    FrontstagePageTabResponse,
};

fn operation(interface_id: &str, method: &str, path: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: interface_id.into(),
        method: method.into(),
        path: path.into(),
    }
}

#[test]
fn frontstage_document_tuple_positions_and_patch_presence_match_http() {
    let context = operation(
        "frontstage.blocks.update",
        "PUT",
        "/api/console/frontstage/pages/:page_id/tabs/:tab_id/block-descriptors",
    );
    let source = json!({"updates":[{"block_id":"block-one","runtime_descriptor":{"contract":"actual descriptor"}}]});
    let input = FrontstageBlocksInput::UpdateDescriptors(
        "page-one".into(),
        "tab-one".into(),
        serde_json::from_value(source.clone()).unwrap(),
    );
    assert_eq!(
        input.project_document_input(&context).unwrap(),
        json!({"path":{"page_id":"page-one","tab_id":"tab-one"},"body":source})
    );
    let query = FrontstageBlocksInput::Search(
        "page-one".into(),
        serde_json::from_value(json!({"tab_id":"tab-one","query":"actual search","limit":50}))
            .unwrap(),
    );
    assert_eq!(
        query.project_document_input(&context).unwrap(),
        json!({"path":{"page_id":"page-one"},"query":{"tab_id":"tab-one","query":"actual search","limit":50}})
    );
    let patch = FrontstagePagesInput::Update(
        "page-one".into(),
        serde_json::from_value(json!({"title":null,"is_hidden":true})).unwrap(),
    );
    let value = patch.project_document_input(&context).unwrap();
    assert_eq!(value["body"]["title"], Value::Null);
    assert_eq!(value["body"]["is_hidden"], true);
    assert!(
        value["body"].get("icon").is_none(),
        "absent PATCH value must not turn into explicit clear"
    );
}

#[test]
fn frontstage_document_locale_and_conditional_headers_are_transport_only() {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "x-1flowbase-locale",
        "LOCALE_DOCUMENT_SENTINEL".parse().unwrap(),
    );
    let input = FrontstagePagesInput::Detail(
        "page-one".into(),
        "default".into(),
        ConsoleLocaleHints::from_headers(&headers),
    );
    let context = operation(
        "frontstage.pages.view",
        "GET",
        "/api/console/frontstage/pages/:page_id/tabs/:tab_reference",
    );
    let value = input.project_document_input(&context).unwrap();
    assert_eq!(
        value,
        json!({"path":{"page_id":"page-one","tab_reference":"default"}})
    );
    assert!(!value.to_string().contains("LOCALE_DOCUMENT_SENTINEL"));
    let input = FrontstageBlocksInput::GetCode(
        "page-one".into(),
        "block-one".into(),
        Some("ETAG_DOCUMENT_SENTINEL".into()),
    );
    assert_eq!(
        input.project_document_input(&context).unwrap(),
        json!({"path":{"page_id":"page-one","block_id":"block-one"}})
    );
}

#[test]
fn frontstage_document_ordinary_outputs_keep_all_items_source_and_field_names() {
    let context = operation(
        "frontstage.tabs.view",
        "GET",
        "/api/console/frontstage/pages/:page_id/tabs",
    );
    let tabs = (0..40)
        .map(|index| FrontstagePageTabResponse {
            id: format!("tab-{index}"),
            page_id: "page-one".into(),
            title: Some(format!("actual tab {index}")),
            rank: format!("{index}"),
            is_default: index == 0,
            route_segment: Some(format!("segment-{index}")),
            document_root_uid: format!("root-{index}"),
        })
        .collect::<Vec<_>>();
    let expected = serde_json::to_value(&tabs).unwrap();
    let output = FrontstagePagesOutput::Tabs(tabs);
    assert_eq!(output.project_document_output(&context).unwrap(), expected);
    let code = FrontstageBlockNodeCodeResponse {
        block_id: "block-one".into(),
        page_id: "page-one".into(),
        source_code: "export const actualValue = 'full source';\n".repeat(500),
        source_sha256: Some("actual-digest".into()),
    };
    let expected = serde_json::to_value(&code).unwrap();
    let output = FrontstageBlocksOutput::Code(code);
    let context = operation(
        "frontstage.blocks.code.view",
        "GET",
        "/api/console/frontstage/pages/:page_id/blocks/:block_id/code",
    );
    let value = output.project_document_output(&context).unwrap();
    assert_eq!(value, expected);
    assert!(value["source_code"].as_str().unwrap().len() > 10000);
    let validator = jsonschema::validator_for(
        &FrontstageBlocksOutput::document_output_schema(&context).unwrap(),
    )
    .unwrap();
    assert!(validator.is_valid(&value));
    let empty = FrontstageBlocksOutput::NotModified("actual-etag".into())
        .project_document_output(&context)
        .unwrap();
    assert_eq!(empty, Value::Null);
    assert!(validator.is_valid(&empty), "304 has no business JSON body");
}

#[test]
fn frontstage_document_catalog_reuses_the_actual_http_projection() {
    use crate::openapi_interface::{
        OpenApiCapabilityCatalogPage, OpenApiCapabilityCatalogSummary, OpenApiCapabilitySource,
    };
    let context = operation(
        "frontstage.callable_interfaces.list",
        "GET",
        "/api/console/frontstage/interface-capabilities",
    );
    let page = OpenApiCapabilityCatalogPage {
        items: (0..40)
            .map(|index| OpenApiCapabilityCatalogSummary {
                interface_id: format!("actual-interface-{index}"),
                method: "GET".into(),
                path: format!("/api/console/actual/{index}"),
                source: OpenApiCapabilitySource::StaticApiDocs,
            })
            .collect(),
        total: 40,
        offset: 0,
        limit: 40,
        has_more: false,
        next_offset: None,
        adapter_ids: vec!["actual adapter".into()],
        methods: vec!["GET".into()],
    };
    let expected = serde_json::to_value(
        crate::routes::frontstage::callable_interfaces::page_response(page.clone()),
    )
    .unwrap();
    let value = FrontstageCallableCatalogOutput::Page(page)
        .project_document_output(&context)
        .unwrap();
    assert_eq!(value, expected);
    assert_eq!(value["items"].as_array().unwrap().len(), 40);
    assert!(value["items"][0].get("adapter_id").is_some());
    assert!(value["items"][0].get("source").is_none());
    let query = FrontstageCallableCatalogInput::List(
        serde_json::from_value(
            json!({"path_prefixes":"/api/runtime/,/api/console/","path_query":"actual"}),
        )
        .unwrap(),
    );
    let value = query.project_document_input(&context).unwrap();
    assert_eq!(
        value["query"]["path_prefixes"],
        "/api/console/,/api/runtime/"
    );
    assert_eq!(value["query"]["path_query"], "actual");
}

#[test]
fn frontstage_document_callable_outer_view_excludes_target_credentials_and_tracks_actual_status() {
    use crate::openapi_interface::{
        CallableDispatchForwarding, CallableDispatchHeader, CallableDispatchHttpResponse,
    };
    let context = operation(
        "frontstage.callable_interfaces.dispatch",
        "POST",
        "/api/console/frontstage/pages/:page_id/tabs/:tab_id/callable-interfaces/dispatch",
    );
    let mut input=FrontstageCallableDispatchInput {
        page_id:"page-one".into(),tab_id:"tab-one".into(),
        body:serde_json::from_value(json!({"block_id":"block-one","method":"POST","path":"/api/console/user-api-keys","request":{"headers":{"authorization":"DISPATCH_DOCUMENT_SECRET"},"query":{"token":"DISPATCH_DOCUMENT_SECRET"},"body":{"password":"DISPATCH_DOCUMENT_SECRET"}}})).unwrap(),
        forwarding:CallableDispatchForwarding { cookie:Some(b"DISPATCH_DOCUMENT_SECRET".to_vec()),authorization:Some(b"DISPATCH_DOCUMENT_SECRET".to_vec()),accept_language:Some(b"private locale".to_vec()),csrf_token:Some(b"DISPATCH_DOCUMENT_SECRET".to_vec()),callable_depth:2 },
    };
    let value = input.project_document_input(&context).unwrap();
    assert_eq!(
        value,
        json!({"path":{"page_id":"page-one","tab_id":"tab-one"},"body":{"block_id":"block-one","method":"POST","path":"/api/console/user-api-keys"}})
    );
    assert!(!value.to_string().contains("DISPATCH_DOCUMENT_SECRET"));
    let schema = FrontstageCallableDispatchInput::document_input_schema(&context).unwrap();
    assert!(!schema.to_string().contains("\"request\""));
    assert!(!schema.to_string().contains("\"forwarding\""));
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&value));
    let mut unsafe_value = value.clone();
    unsafe_value["body"]["request"] = json!({"body":{"password":"DISPATCH_DOCUMENT_SECRET"}});
    assert!(!validator.is_valid(&unsafe_value));
    input.body.path = "/api/target?token=DISPATCH_DOCUMENT_SECRET".into();
    assert!(input.project_document_input(&context).is_err());
    let response = |status| CallableDispatchHttpResponse {
        status,
        headers: vec![CallableDispatchHeader {
            name: "set-cookie".into(),
            value: b"DISPATCH_DOCUMENT_SECRET".to_vec(),
        }],
        body: b"DISPATCH_DOCUMENT_SECRET".to_vec(),
    };
    let outputs = [
        (
            FrontstageCallableDispatchOutput::Json(
                json!({"data":{"token":"DISPATCH_DOCUMENT_SECRET"}}),
            ),
            200,
        ),
        (FrontstageCallableDispatchOutput::NoContent, 204),
        (FrontstageCallableDispatchOutput::Media(response(206)), 206),
        (FrontstageCallableDispatchOutput::Target(response(403)), 403),
    ];
    let schema = FrontstageCallableDispatchOutput::document_output_schema(&context).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (output, status) in outputs {
        let value = output.project_document_output(&context).unwrap();
        assert_eq!(value, json!({"status":status}));
        assert!(validator.is_valid(&value));
        assert!(!value.to_string().contains("DISPATCH_DOCUMENT_SECRET"));
    }
}

#[test]
fn frontstage_document_component_asset_restriction_is_route_specific() {
    let list = operation(
        "frontstage.components.view",
        "GET",
        "/api/console/frontstage/components",
    );
    let asset = operation(
        "frontstage.components.view",
        "GET",
        "/api/console/frontstage/component-module-assets/:sha256",
    );
    assert!(FrontstageComponentsOutput::document_output_unavailable(&list).is_none());
    assert_eq!(
        FrontstageComponentsOutput::document_output_unavailable(&asset),
        Some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    );
}
