use std::{future::Future, pin::Pin, sync::Arc};

use control_plane::application_public_api::{
    native::{NativeRunRequest, NativeRunResult},
    protocol_translation::TranslationProtocol,
};
use interface_runtime::{
    ApplicationPrincipal, AuthenticationAdapterReference, AuthorizationAdapterReference,
    AuthorizationOperation, BindingId, CompiledInterfaceRegistry, ContractIdentity,
    GraphFingerprint, HandlerReference, InterfaceAccess, InterfaceAuditPolicy,
    InterfaceAuthenticationPolicy, InterfaceAuthorizationFuture, InterfaceAuthorizationPort,
    InterfaceAuthorizationRequest, InterfaceContract, InterfaceContracts, InterfaceDefinition,
    InterfaceErrorPolicy, InterfaceEventStream, InterfaceExecution, InterfaceExecutionMode,
    InterfaceHandler, InterfaceHandlerContext, InterfaceHandlerFuture, InterfaceId,
    InterfaceIdentity, InterfaceLifecycle, InterfaceOwner, InterfaceScope, InterfaceStreamHandler,
    InterfaceStreamHandlerFuture, InterfaceTargetFailure, InterfaceVersion, InvocationAdapterPlan,
    ProtocolBinding, ProtocolProjection, RegistryCompiler, RouteIdentity, TargetReference,
};

use super::native::NativeApiError;

pub(crate) const ASYNC_INTERFACE_ID: &str = "application.native.runs.create-async";
pub(crate) const BLOCKING_INTERFACE_ID: &str = "application.native.runs.execute-blocking";
pub(crate) const STREAM_INTERFACE_ID: &str = "application.native.runs.execute-stream";
pub(crate) const ASYNC_BINDING_ID: &str = "http.application.native.runs.create-async.v1";
pub(crate) const BLOCKING_BINDING_ID: &str = "http.application.native.runs.execute-blocking.v1";
pub(crate) const STREAM_BINDING_ID: &str = "http.application.native.runs.execute-stream.v1";
const INTERFACE_VERSION: &str = "1";
const ASYNC_HANDLER_REFERENCE: &str = "api-server.application-native-run.create-async";
const BLOCKING_HANDLER_REFERENCE: &str = "api-server.application-native-run.execute-blocking";
const STREAM_HANDLER_REFERENCE: &str = "api-server.application-native-run.execute-stream";
const TARGET_REFERENCE: &str = "control-plane.application-native-run.create";
const OWNER: &str = "api-server.application-public-api";
const OPERATION: &str = "application.native.runs.create";

pub(crate) struct ApplicationNativeRunInput {
    pub(crate) request: NativeRunRequest,
    pub(crate) protocol: TranslationProtocol,
}

impl InterfaceContract for ApplicationNativeRunInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[
            (
                "request",
                mp::object_schema(&[
                    (
                        "query",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    ),
                    (
                        "system",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Text")), ("text",mp::object_schema(&[("byte_count",mp::count_schema())])), ("cache_control",serde_json::json!({"anyOf": [mp::object_schema(&[("cache_type",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Ephemeral"))])])), ("ttl",serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("FiveMinutes"))]), mp::object_schema(&[("variant",mp::tag_schema("OneHour"))])]), {"type":"null"}]}))]), {"type":"null"}]}))])])}),
                    ),
                    (
                        "model",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "history",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    ),
                    (
                        "attachments",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("source",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("UploadFileId"))]), mp::object_schema(&[("variant",mp::tag_schema("Url"))]), mp::object_schema(&[("variant",mp::tag_schema("Base64"))])])), ("value",mp::object_schema(&[("byte_count",mp::count_schema())])), ("name",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("mime_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])}),
                    ),
                    (
                        "expand_id",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    ),
                    (
                        "response_mode",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    ),
                    (
                        "stream_options",
                        mp::object_schema(&[(
                            "include_workflow_events",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("None"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Public"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Debug"))]),
                            ]),
                        )]),
                    ),
                    (
                        "request_context",
                        mp::object_schema(&[(
                            "end_user_reference",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        )]),
                    ),
                    (
                        "title",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                    ),
                    (
                        "client_protocol_envelope",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("source_protocol",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_request",serde_json::json!({"anyOf": [mp::object_schema(&[("authentication",serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("AuthorizationBearer"))]), mp::object_schema(&[("variant",mp::tag_schema("XApiKey"))])]), {"type":"null"}]})), ("body",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}))]), {"type":"null"}]})), ("query",mp::object_schema(&[("item_count",mp::count_schema())])), ("body",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                    ),
                ]),
            ),
            (
                "protocol",
                mp::union_schema(vec![
                    mp::object_schema(&[("variant", mp::tag_schema("Native"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("OpenAiChat"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("OpenAiResponses"))]),
                    mp::object_schema(&[("variant", mp::tag_schema("AnthropicMessages"))]),
                ]),
            ),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("request",mp::object_value(&[("query",mp::object_value(&[("byte_count",serde_json::json!((&(&(self).request).query).len()))])), ("system",{ if (&(&(self).request).system).len() > 32 { return None; } serde_json::Value::Array((&(&(self).request).system).iter().map(|item| Some(match item {extension_contracts::provider_contract::NativePromptBlock::Text {text: _field_text, cache_control: _field_cache_control, .. } => mp::object_value(&[("variant",serde_json::Value::String("Text".to_owned())), ("text",mp::object_value(&[("byte_count",serde_json::json!((_field_text).len()))])), ("cache_control",match (_field_cache_control).as_ref() { Some(item) => mp::object_value(&[("cache_type",match &(item).cache_type {extension_contracts::provider_contract::NativePromptCacheControlType::Ephemeral => mp::object_value(&[("variant",serde_json::Value::String("Ephemeral".to_owned()))])}), ("ttl",match (&(item).ttl).as_ref() { Some(item) => match item {extension_contracts::provider_contract::NativePromptCacheTtl::FiveMinutes => mp::object_value(&[("variant",serde_json::Value::String("FiveMinutes".to_owned()))]), extension_contracts::provider_contract::NativePromptCacheTtl::OneHour => mp::object_value(&[("variant",serde_json::Value::String("OneHour".to_owned()))])}, None => serde_json::Value::Null })]), None => serde_json::Value::Null })])})).collect::<Option<Vec<_>>>()?) }), ("model",match (&(&(self).request).model).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("history",mp::object_value(&[("item_count",serde_json::json!((&(&(self).request).history).len()))])), ("attachments",{ if (&(&(self).request).attachments).len() > 32 { return None; } serde_json::Value::Array((&(&(self).request).attachments).iter().map(|item| Some(mp::object_value(&[("source",match &(item).source {control_plane::application_public_api::native::NativeAttachmentSource::UploadFileId => mp::object_value(&[("variant",serde_json::Value::String("UploadFileId".to_owned()))]), control_plane::application_public_api::native::NativeAttachmentSource::Url => mp::object_value(&[("variant",serde_json::Value::String("Url".to_owned()))]), control_plane::application_public_api::native::NativeAttachmentSource::Base64 => mp::object_value(&[("variant",serde_json::Value::String("Base64".to_owned()))])}), ("value",mp::object_value(&[("byte_count",serde_json::json!((&(item).value).len()))])), ("name",match (&(item).name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("mime_type",match (&(item).mime_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("expand_id",match (&(&(self).request).expand_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("response_mode",match (&(&(self).request).response_mode).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("stream_options",mp::object_value(&[("include_workflow_events",match &(&(&(self).request).stream_options).include_workflow_events {control_plane::application_public_api::native::NativeWorkflowEventVisibility::None => mp::object_value(&[("variant",serde_json::Value::String("None".to_owned()))]), control_plane::application_public_api::native::NativeWorkflowEventVisibility::Public => mp::object_value(&[("variant",serde_json::Value::String("Public".to_owned()))]), control_plane::application_public_api::native::NativeWorkflowEventVisibility::Debug => mp::object_value(&[("variant",serde_json::Value::String("Debug".to_owned()))])})])), ("request_context",mp::object_value(&[("end_user_reference",match (&(&(&(self).request).request_context).end_user_reference).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("title",match (&(&(self).request).title).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("client_protocol_envelope",match (&(&(self).request).client_protocol_envelope).as_ref() { Some(item) => mp::object_value(&[("source_protocol",mp::object_value(&[("byte_count",serde_json::json!((&(item).source_protocol).len()))])), ("source_request",match (&(item).source_request).as_ref() { Some(item) => mp::object_value(&[("authentication",match (&(item).authentication).as_ref() { Some(item) => match item {extension_contracts::provider_contract::ProtocolAuthenticationPresentation::AuthorizationBearer => mp::object_value(&[("variant",serde_json::Value::String("AuthorizationBearer".to_owned()))]), extension_contracts::provider_contract::ProtocolAuthenticationPresentation::XApiKey => mp::object_value(&[("variant",serde_json::Value::String("XApiKey".to_owned()))])}, None => serde_json::Value::Null }), ("body",match (&(item).body).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null })]), None => serde_json::Value::Null }), ("query",mp::object_value(&[("item_count",serde_json::json!((&(item).query).len()))])), ("body",mp::object_value(&[("item_count",serde_json::json!((&(item).body).len()))]))]), None => serde_json::Value::Null })])), ("protocol",match &(self).protocol {control_plane::application_public_api::protocol_translation::TranslationProtocol::Native => mp::object_value(&[("variant",serde_json::Value::String("Native".to_owned()))]), control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiChat => mp::object_value(&[("variant",serde_json::Value::String("OpenAiChat".to_owned()))]), control_plane::application_public_api::protocol_translation::TranslationProtocol::OpenAiResponses => mp::object_value(&[("variant",serde_json::Value::String("OpenAiResponses".to_owned()))]), control_plane::application_public_api::protocol_translation::TranslationProtocol::AnthropicMessages => mp::object_value(&[("variant",serde_json::Value::String("AnthropicMessages".to_owned()))])})]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-create-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationNativeRunOutput(pub(crate) NativeRunResult);

impl InterfaceContract for ApplicationNativeRunOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                ("id", mp::text_schema()),
                ("application_id", mp::text_schema()),
                ("publication_version_id", mp::text_schema()),
                (
                    "status",
                    mp::union_schema(vec![
                        mp::object_schema(&[("variant", mp::tag_schema("Created"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Queued"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Running"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Waiting"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Succeeded"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Incomplete"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Failed"))]),
                        mp::object_schema(&[("variant", mp::tag_schema("Cancelled"))]),
                    ]),
                ),
                ("node_input_payload", mp::json_summary_schema()),
                ("metadata", mp::json_summary_schema()),
                (
                    "answer",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "answer_segments",
                    serde_json::json!({"anyOf": [serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Reasoning"))]), mp::object_schema(&[("variant",mp::tag_schema("Message"))])])), ("text",mp::object_schema(&[("byte_count",mp::count_schema())]))])}), {"type":"null"}]}),
                ),
                (
                    "required_action",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("action_type",mp::text_schema()), ("payload",mp::json_summary_schema())]), {"type":"null"}]}),
                ),
                (
                    "tool_calls",
                    serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                ),
                (
                    "error",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("details",mp::json_summary_schema())]), {"type":"null"}]}),
                ),
                (
                    "operation_terminal",
                    serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("CountTokens"))]), mp::object_schema(&[("variant",mp::tag_schema("Compact"))])]), {"type":"null"}]}),
                ),
                ("created_at", serde_json::json!({"type":"integer"})),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[("0",mp::object_value(&[("id",serde_json::Value::String((&(&(self).0).id).to_string())), ("application_id",serde_json::Value::String((&(&(self).0).application_id).to_string())), ("publication_version_id",serde_json::Value::String((&(&(self).0).publication_version_id).to_string())), ("status",match &(&(self).0).status {control_plane::application_public_api::native::NativeRunStatus::Created => mp::object_value(&[("variant",serde_json::Value::String("Created".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Queued => mp::object_value(&[("variant",serde_json::Value::String("Queued".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Running => mp::object_value(&[("variant",serde_json::Value::String("Running".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Waiting => mp::object_value(&[("variant",serde_json::Value::String("Waiting".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Succeeded => mp::object_value(&[("variant",serde_json::Value::String("Succeeded".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Incomplete => mp::object_value(&[("variant",serde_json::Value::String("Incomplete".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))]), control_plane::application_public_api::native::NativeRunStatus::Cancelled => mp::object_value(&[("variant",serde_json::Value::String("Cancelled".to_owned()))])}), ("node_input_payload",mp::json_summary(&(&(self).0).node_input_payload)), ("metadata",mp::json_summary(&(&(self).0).metadata)), ("answer",match (&(&(self).0).answer).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("answer_segments",match (&(&(self).0).answer_segments).as_ref() { Some(item) => { if (item).len() > 32 { return None; } serde_json::Value::Array((item).iter().map(|item| Some(mp::object_value(&[("kind",match &(item).kind {orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Reasoning => mp::object_value(&[("variant",serde_json::Value::String("Reasoning".to_owned()))]), orchestration_runtime::answer_projection::AnswerProjectionSegmentKind::Message => mp::object_value(&[("variant",serde_json::Value::String("Message".to_owned()))])}), ("text",mp::object_value(&[("byte_count",serde_json::json!((&(item).text).len()))]))]))).collect::<Option<Vec<_>>>()?) }, None => serde_json::Value::Null }), ("required_action",match (&(&(self).0).required_action).as_ref() { Some(item) => mp::object_value(&[("action_type",mp::text(&(item).action_type)?), ("payload",mp::json_summary(&(item).payload))]), None => serde_json::Value::Null }), ("tool_calls",match (&(&(self).0).tool_calls).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("error",match (&(&(self).0).error).as_ref() { Some(item) => mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("details",mp::json_summary(&(item).details))]), None => serde_json::Value::Null }), ("operation_terminal",match (&(&(self).0).operation_terminal).as_ref() { Some(item) => match item {extension_contracts::semantic_terminal::NativeOperationTerminal::CountTokens(_) => mp::object_value(&[("variant",serde_json::Value::String("CountTokens".to_owned()))]), extension_contracts::semantic_terminal::NativeOperationTerminal::Compact(_) => mp::object_value(&[("variant",serde_json::Value::String("Compact".to_owned()))])}, None => serde_json::Value::Null }), ("created_at",serde_json::json!((&(&(self).0).created_at).unix_timestamp()))]))]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-create-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ApplicationNativeRunTargetError(pub(crate) NativeApiError);

impl InterfaceContract for ApplicationNativeRunTargetError {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[
                (
                    "status",
                    serde_json::json!({"type":"integer","minimum":100,"maximum":599}),
                ),
                ("code", mp::text_schema()),
                (
                    "message",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[
                ("status", serde_json::json!((&(&(self).0).status).as_u16())),
                ("code", mp::text(&(&(self).0).code)?),
                (
                    "message",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((&(&(self).0).message).len()),
                    )]),
                ),
            ]),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-create-error";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type ApplicationNativeRunFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<ApplicationNativeRunOutput, ApplicationNativeRunTargetError>>
            + Send
            + 'a,
    >,
>;

pub(crate) struct ApplicationNativeRunStreamEvent(
    pub(crate) Result<axum::response::sse::Event, std::convert::Infallible>,
);

impl InterfaceContract for ApplicationNativeRunStreamEvent {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_schema(&[(
            "0",
            mp::object_schema(&[("is_ok", serde_json::json!({"type":"boolean"}))]),
        )]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::object_value(&[(
            "0",
            mp::object_value(&[("is_ok", serde_json::Value::Bool((&(self).0).is_ok()))]),
        )]))
    }

    const CONTRACT_ID: &'static str = "application-native-run-stream-event";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) type ApplicationNativeRunStreamFuture<'a> = Pin<
    Box<
        dyn Future<
                Output = Result<
                    InterfaceEventStream<
                        ApplicationNativeRunStreamEvent,
                        ApplicationNativeRunOutput,
                        ApplicationNativeRunTargetError,
                    >,
                    ApplicationNativeRunTargetError,
                >,
            > + Send
            + 'a,
    >,
>;

pub(crate) trait ApplicationNativeRunPort: Send + Sync + 'static {
    fn create<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a>;

    fn execute_blocking<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunFuture<'a>;

    fn execute_stream<'a>(
        &'a self,
        principal: &'a ApplicationPrincipal,
        input: ApplicationNativeRunInput,
    ) -> ApplicationNativeRunStreamFuture<'a>;
}

enum ApplicationNativeRunHandlerMode {
    Async,
    Blocking,
}

struct ApplicationNativeUnaryHandler {
    port: Arc<dyn ApplicationNativeRunPort>,
    mode: ApplicationNativeRunHandlerMode,
}

impl
    InterfaceHandler<
        ApplicationNativeRunInput,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
        ApplicationPrincipal,
    > for ApplicationNativeUnaryHandler
{
    fn invoke(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: ApplicationNativeRunInput,
    ) -> InterfaceHandlerFuture<ApplicationNativeRunOutput, ApplicationNativeRunTargetError> {
        let port = Arc::clone(&self.port);
        let blocking = matches!(self.mode, ApplicationNativeRunHandlerMode::Blocking);
        Box::pin(async move {
            let result = if blocking {
                port.execute_blocking(context.principal(), input).await
            } else {
                port.create(context.principal(), input).await
            };
            result.map_err(|error| InterfaceTargetFailure::new("application_native_run", error))
        })
    }
}

struct ApplicationNativeStreamHandler {
    port: Arc<dyn ApplicationNativeRunPort>,
}

impl
    InterfaceStreamHandler<
        ApplicationNativeRunInput,
        ApplicationNativeRunStreamEvent,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
        ApplicationPrincipal,
    > for ApplicationNativeStreamHandler
{
    fn invoke_stream(
        &self,
        context: InterfaceHandlerContext<ApplicationPrincipal>,
        input: ApplicationNativeRunInput,
    ) -> InterfaceStreamHandlerFuture<
        ApplicationNativeRunStreamEvent,
        ApplicationNativeRunOutput,
        ApplicationNativeRunTargetError,
    > {
        let port = Arc::clone(&self.port);
        Box::pin(async move {
            port.execute_stream(context.principal(), input)
                .await
                .map_err(|error| InterfaceTargetFailure::new("application_native_stream", error))
        })
    }
}

pub(crate) struct ApplicationNativeRunAuthorization;

impl InterfaceAuthorizationPort<ApplicationPrincipal> for ApplicationNativeRunAuthorization {
    fn adapter_reference(&self) -> AuthorizationAdapterReference {
        AuthorizationAdapterReference::new("api-server.application-native-run")
            .expect("static adapter is valid")
    }

    fn authorize(
        &self,
        _request: InterfaceAuthorizationRequest<ApplicationPrincipal>,
    ) -> InterfaceAuthorizationFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}

pub(crate) fn compile_registry(
    port: Arc<dyn ApplicationNativeRunPort>,
) -> Result<Arc<CompiledInterfaceRegistry>, interface_runtime::RegistryCompilationError> {
    let unary_contracts = InterfaceContracts::unary(
        contract::<ApplicationNativeRunInput>(),
        contract::<ApplicationNativeRunOutput>(),
        contract::<ApplicationNativeRunTargetError>(),
    );
    let stream_contracts = InterfaceContracts::server_stream(
        contract::<ApplicationNativeRunInput>(),
        contract::<ApplicationNativeRunStreamEvent>(),
        contract::<ApplicationNativeRunOutput>(),
        contract::<ApplicationNativeRunTargetError>(),
    );
    let owner = InterfaceOwner::new(OWNER).expect("static owner is valid");
    let operation = AuthorizationOperation::new(OPERATION).expect("static operation is valid");
    let mut compiler = RegistryCompiler::new(
        GraphFingerprint::new("graph:application-native-run-v1")
            .expect("static graph fingerprint is valid"),
        [operation.clone()],
        [owner.clone()],
    );
    for (interface_id, binding_id, handler_reference, mode, projection, contracts) in [
        (
            ASYNC_INTERFACE_ID,
            ASYNC_BINDING_ID,
            ASYNC_HANDLER_REFERENCE,
            InterfaceExecutionMode::Unary,
            ProtocolProjection::http_variant(route(), "async"),
            unary_contracts.clone(),
        ),
        (
            BLOCKING_INTERFACE_ID,
            BLOCKING_BINDING_ID,
            BLOCKING_HANDLER_REFERENCE,
            InterfaceExecutionMode::Unary,
            ProtocolProjection::http(route()),
            unary_contracts.clone(),
        ),
        (
            STREAM_INTERFACE_ID,
            STREAM_BINDING_ID,
            STREAM_HANDLER_REFERENCE,
            InterfaceExecutionMode::ServerStream,
            ProtocolProjection::http_variant(route(), "streaming"),
            stream_contracts.clone(),
        ),
    ] {
        let id = InterfaceId::new(interface_id).expect("static interface id is valid");
        let identity = InterfaceIdentity::new(
            id.clone(),
            InterfaceVersion::new(INTERFACE_VERSION).expect("static interface version is valid"),
        );
        compiler.register_definition(InterfaceDefinition::new(
            identity.clone(),
            contracts.clone(),
            InterfaceAccess::new(
                interface_runtime::PrincipalProfile::Application,
                InterfaceAuthenticationPolicy::Authenticated,
                operation.clone(),
                InterfaceScope::Workspace,
            ),
            InterfaceExecution::new(
                mode,
                HandlerReference::new(handler_reference).expect("static handler is valid"),
                TargetReference::new(TARGET_REFERENCE).expect("static target is valid"),
            ),
            InterfaceAuditPolicy::Mutating,
            InterfaceErrorPolicy::TypedTarget,
            InterfaceLifecycle::BootSnapshot,
            owner.clone(),
        ))?;
        compiler.register_authentication_adapter(
            &id,
            1,
            interface_runtime::InterfaceExtensionRegistration::new(
                interface_runtime::PluginIdentity::new("api-server.application-authentication")
                    .expect("static plugin is valid"),
                interface_runtime::InterfaceExtensionTier::BuiltIn,
                interface_runtime::InterfaceExtensionPoint::AuthenticationAdapter,
                interface_runtime::InterfaceExtensionPermission::Authenticate,
                InterfaceScope::Workspace,
                interface_runtime::InterfaceExtensionIsolation::TrustedInProcess,
                [],
            )
            .expect("built-in authentication registration is valid"),
            interface_runtime::ActivatedAuthenticationAdapter::new(
                interface_runtime::PluginIdentity::new("api-server.application-authentication")
                    .expect("static plugin is valid"),
                interface_runtime::InterfaceExtensionTier::BuiltIn,
                AuthenticationAdapterReference::new("api-server.application-api-key")
                    .expect("static adapter is valid"),
                interface_runtime::AuthenticationActivationIdentity::new(
                    "api-server.application-api-key.activation.v1",
                )
                .expect("static activation is valid"),
                interface_runtime::PrincipalProfile::Application,
            ),
        )?;
        compiler.register_binding(
            ProtocolBinding::new(
                BindingId::new(binding_id).expect("static binding is valid"),
                identity,
                contracts,
                projection,
            ),
            adapter_plan(),
        )?;
    }
    compiler.bind_handler::<ApplicationNativeRunInput, ApplicationNativeRunOutput, ApplicationNativeRunTargetError, ApplicationPrincipal>(
        &InterfaceId::new(ASYNC_INTERFACE_ID).expect("static async interface id is valid"),
        HandlerReference::new(ASYNC_HANDLER_REFERENCE)
            .expect("static async handler reference is valid"),
        Arc::new(ApplicationNativeUnaryHandler { port: Arc::clone(&port), mode: ApplicationNativeRunHandlerMode::Async }),
    )?;
    compiler.bind_handler::<ApplicationNativeRunInput, ApplicationNativeRunOutput, ApplicationNativeRunTargetError, ApplicationPrincipal>(
        &InterfaceId::new(BLOCKING_INTERFACE_ID)
            .expect("static blocking interface id is valid"),
        HandlerReference::new(BLOCKING_HANDLER_REFERENCE)
            .expect("static blocking handler reference is valid"),
        Arc::new(ApplicationNativeUnaryHandler { port: Arc::clone(&port), mode: ApplicationNativeRunHandlerMode::Blocking }),
    )?;
    compiler.bind_stream_handler::<ApplicationNativeRunInput, ApplicationNativeRunStreamEvent, ApplicationNativeRunOutput, ApplicationNativeRunTargetError, ApplicationPrincipal>(
        &InterfaceId::new(STREAM_INTERFACE_ID).expect("static stream interface id is valid"),
        HandlerReference::new(STREAM_HANDLER_REFERENCE)
            .expect("static stream handler reference is valid"),
        Arc::new(ApplicationNativeStreamHandler { port }),
    )?;
    compiler.compile()
}

fn route() -> RouteIdentity {
    RouteIdentity::new("POST", "/api/agent/v1/runs").expect("static route is valid")
}

fn adapter_plan() -> InvocationAdapterPlan {
    InvocationAdapterPlan::new(
        AuthenticationAdapterReference::new("api-server.application-api-key")
            .expect("static adapter is valid"),
        AuthorizationAdapterReference::new("api-server.application-native-run")
            .expect("static adapter is valid"),
        None,
    )
}

fn contract<T: InterfaceContract>() -> ContractIdentity {
    ContractIdentity::new(T::CONTRACT_ID, T::CONTRACT_VERSION)
        .expect("static interface contract is valid")
}
