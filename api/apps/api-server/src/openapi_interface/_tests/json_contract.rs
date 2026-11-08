use super::{body, query, request, response, result, JsonApiInput, JsonApiOutput};
use crate::{
    event_center::api_projection,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
    routes::organization::DepartmentResponse,
};
use domain::ActorContext;
use interface_runtime::*;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};

#[derive(Serialize, utoipa::ToSchema)]
struct BodyDto {
    title: String,
    optional: Option<String>,
}
#[derive(Serialize)]
#[serde(untagged)]
enum Input {
    Empty,
    Localized {
        #[serde(skip)]
        _locale: ConsoleLocaleHints,
    },
    #[serde(serialize_with = "body")]
    Create(BodyDto),
    Update {
        id: String,
        body: BodyDto,
    },
    #[serde(serialize_with = "query")]
    Search(QueryDto),
}
#[derive(Serialize)]
struct QueryDto {
    limit: u32,
    cursor: Option<String>,
    nested: Value,
}
impl InterfaceContract for Input {
    const CONTRACT_ID: &'static str = "public-json-proof-input";
    const CONTRACT_VERSION: &'static str = "1";
}
impl JsonApiInput for Input {}

#[derive(Serialize)]
#[serde(untagged)]
enum Output {
    Item(DepartmentResponse),
    Empty,
}
impl InterfaceContract for Output {
    const CONTRACT_ID: &'static str = "public-json-proof-output";
    const CONTRACT_VERSION: &'static str = "1";
}
impl JsonApiOutput for Output {}

#[test]
fn public_json_contract_preserves_body_nulls_omits_query_nulls_and_excludes_opaque_locale() {
    assert_eq!(request(&Input::Empty, "/fixture").unwrap(), json!({}));
    let mut headers = axum::http::HeaderMap::new();
    headers.insert("x-1flowbase-locale", "zh-CN".parse().unwrap());
    assert_eq!(
        request(
            &Input::Localized {
                _locale: ConsoleLocaleHints::from_headers(&headers),
            },
            "/fixture",
        )
        .unwrap(),
        json!({})
    );
    let input = Input::Create(BodyDto {
        title: "Full title, not a byte-count projection".into(),
        optional: None,
    });
    assert_eq!(
        request(&input, "/fixture").unwrap(),
        json!({"body":{"title":"Full title, not a byte-count projection","optional":null}})
    );
    assert_eq!(
        request(
            &Input::Search(QueryDto {
                limit: 100,
                cursor: None,
                nested: json!({"still_null":null}),
            }),
            "/fixture",
        )
        .unwrap(),
        json!({"query":{"limit":100,"nested":{"still_null":null}}})
    );
    let input = Input::Update {
        id: "actual-id".into(),
        body: BodyDto {
            title: "Updated".into(),
            optional: None,
        },
    };
    for path in ["/fixture/:id", "/fixture/{id}"] {
        assert_eq!(
            request(&input, path).unwrap(),
            json!({"path":{"id":"actual-id"},"body":{"title":"Updated","optional":null}})
        );
    }
    assert!(
        request(&input, "/fixture").is_err(),
        "unmapped root id must fail closed"
    );
}

#[test]
fn public_json_contract_response_and_event_result_serialize_the_complete_same_dto() {
    let dto = DepartmentResponse {
        id: "department-id".into(),
        name: "Actual department name".into(),
        parent_id: None,
        role_codes: vec!["reader".into()],
        member_count: 7,
    };
    // The DTO itself owns its field set; this catches enum tags and summaries without
    // maintaining another handwritten copy of every output field.
    let expected = serde_json::to_value(&dto).unwrap();
    let output = Output::Item(dto);
    assert_eq!(result(&output).unwrap(), expected);
    let axum::Json(http) = response(&output).unwrap();
    assert_eq!(http.data, expected);
    assert_eq!(result(&Output::Empty).unwrap(), Value::Null);
    assert_eq!(response(&Output::Empty).unwrap().0.data, Value::Null);
}

struct Port(Arc<Mutex<Option<Value>>>);
impl ConsoleInterfacePort<Input, Output> for Port {
    fn execute<'a>(
        &'a self,
        _: &'a UserPrincipal,
        input: Input,
    ) -> ConsoleInterfaceFuture<'a, Output> {
        Box::pin(async move {
            *self.0.lock().unwrap() = Some(request(&input, "/fixture/:id").unwrap());
            let Input::Update { id, body } = input else {
                panic!("Update")
            };
            Ok(Output::Item(DepartmentResponse {
                id,
                name: body.title,
                parent_id: body.optional,
                role_codes: vec![],
                member_count: 0,
            }))
        })
    }
}
struct Mutate;
impl InterfaceBeforeHook<Input> for Mutate {
    fn before<'a>(
        &'a self,
        _: InterfaceHookContext,
        input: &'a mut Input,
    ) -> InterfaceBeforeHookFuture<'a> {
        Box::pin(async move {
            let Input::Update { body, .. } = input else {
                panic!("Update")
            };
            body.title = "typed Before value".into();
            Ok(())
        })
    }
}
#[derive(Default)]
struct Observer(Mutex<Vec<DocumentOperationEvent>>);
impl DocumentOperationObserver for Observer {
    fn observe(&self, event: DocumentOperationEvent) -> DocumentOperationObserverFuture<'_> {
        self.0.lock().unwrap().push(event);
        Box::pin(async { Ok(()) })
    }
}
struct Authorization;
impl InterfaceAuthorizationPort for Authorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.console.compiled-operation").unwrap()
    }
    fn authorize(&self, _: InterfaceAuthorizationRequest) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
struct Admission;
impl InterfaceTargetAdmissionPort for Admission {
    fn adapter_reference(&self) -> AdmissionAdapterReference {
        AdmissionAdapterReference::new("api-server.console.protocol-admission").unwrap()
    }
    fn admit(&self, _: InterfaceTargetAdmissionRequest) -> InterfaceTargetAdmissionFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

#[tokio::test]
async fn public_json_contract_blanket_codec_observes_the_typed_before_value_received_by_handler() {
    let seen = Arc::new(Mutex::new(None));
    let family = console_interface::compile_registry(
        "api-server.public-json-proof",
        "graph:public-json-proof-family",
        &[ConsoleInterfaceDeclaration {
            interface_id: "fixture.public-json.update",
            binding_id: "http.fixture.public-json.update",
            method: "PATCH",
            path: "/fixture/:id",
            mutating: true,
        }],
        Arc::new(Port(seen.clone())),
    )
    .unwrap();
    let graph = GraphFingerprint::new("graph:public-json-proof-assembly").unwrap();
    let observer = Arc::new(Observer::default());
    let mut compiler = RegistryCompiler::new(
        graph.clone(),
        family
            .definitions()
            .map(|d| d.authorization_operation().clone()),
        family.definitions().map(|d| d.owner().clone()),
    )
    .with_document_operation_observer(observer.clone());
    compiler.absorb_snapshot(&family).unwrap();
    let interface = InterfaceId::new("fixture.public-json.update").unwrap();
    let plugin = PluginIdentity::new("fixture.public-json.mutate").unwrap();
    compiler
        .register_extension(
            &interface,
            1,
            InterfaceExtensionRegistration::new(
                plugin.clone(),
                InterfaceExtensionTier::HostExtension,
                InterfaceExtensionPoint::Before,
                InterfaceExtensionPermission::MutateInput,
                InterfaceScope::Workspace,
                InterfaceExtensionIsolation::TrustedInProcess,
                [InterfaceExtensionFact::TypedInput],
            )
            .unwrap(),
        )
        .unwrap();
    compiler
        .bind_hook_plan(
            &interface,
            Arc::new(
                TypedInterfaceHookPlan::<Input, Output>::new(graph)
                    .bind_before(plugin, Arc::new(Mutate)),
            ),
        )
        .unwrap();
    let request_contract = api_projection::schema::<BodyDto>();
    let result_contract = api_projection::schema::<DepartmentResponse>();
    let docs = crate::openapi_docs::build_api_docs_registry(json!({
        "openapi":"3.1.0","info":{"title":"Public JSON proof","version":"1"},
        "paths":{"/fixture/{id}":{"patch":{"operationId":"document.public-json.update","summary":"Update fixture",
            "parameters":[{"in":"path","name":"id","required":true,"schema":{"type":"string"}}],
            "requestBody":{"content":{"application/json":{"schema":request_contract}}},
            "responses":{"200":{"description":"Department DTO","content":{"application/json":{"schema":result_contract}}}}
        }}}
    })).unwrap();
    api_projection::refine_static_api_events(&mut compiler, &docs, &[family]).unwrap();
    let snapshot = compiler.compile().unwrap();
    let binding = BindingId::new("http.fixture.public-json.update").unwrap();
    let plan = snapshot.plan(&binding).unwrap();
    let envelope = InvocationEnvelope::with_principal(
        InvocationLineage::root(InvocationId::now_v7()),
        binding,
        InterfaceProtocol::Http,
        plan.authentication().adapter().clone(),
        plan.authentication().activation().clone(),
        UserPrincipal::server_delegation(ActorContext::root(
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            "root",
        )),
        None,
        Input::Update {
            id: "actual-id".into(),
            body: BodyDto {
                title: "raw request value".into(),
                optional: None,
            },
        },
    );
    let outcome = InterfaceInvocationKernel::with_target_admission(
        Arc::new(Authorization),
        Arc::new(Admission),
    )
    .invoke::<Input, Output, ConsoleInterfaceTargetError>(snapshot, envelope)
    .await
    .unwrap();
    let handler_input = seen.lock().unwrap().clone().unwrap();
    assert_eq!(handler_input["body"]["title"], "typed Before value");
    let events = observer.0.lock().unwrap();
    assert_eq!(events.len(), 2);
    let DocumentOperationEvent::Before { input, .. } = &events[0] else {
        panic!("Before")
    };
    assert_eq!(
        *input, handler_input,
        "raw request capture cannot satisfy this proof"
    );
    let input_schema = events[0].descriptor().schemas.request.as_ref().unwrap();
    let input_validator = jsonschema::validator_for(input_schema).unwrap();
    assert!(input_validator.is_valid(input), "{input_schema}: {input}");
    let mut invalid_input = input.clone();
    invalid_input["body"]["title"] = json!(42);
    assert!(
        !input_validator.is_valid(&invalid_input),
        "the inherited DTO schema must reject a wrong field type"
    );
    let DocumentOperationEvent::Completion { output, .. } = &events[1] else {
        panic!("Completion")
    };
    assert_eq!(*output, Some(result(outcome.value()).unwrap()));
    let output = output.as_ref().unwrap();
    assert_eq!(output["name"], "typed Before value");
    let output_schema = events[1].descriptor().schemas.result.as_ref().unwrap();
    let output_validator = jsonschema::validator_for(output_schema).unwrap();
    assert!(
        output_validator.is_valid(output),
        "{output_schema}: {output}"
    );
    let mut invalid_output = output.clone();
    invalid_output["member_count"] = json!("not an integer");
    assert!(
        !output_validator.is_valid(&invalid_output),
        "the inherited result schema must retain the actual DTO field types"
    );
}
