use control_plane::application_public_api::publications::ApplicationPublicationVersionRecord;
use domain::ApplicationRecord;
use serde_json::{json, Value};

use crate::openapi_docs::{
    DocsCatalog, DocsCatalogCategory, DocsCatalogCategoryOperations, DocsCatalogOperation,
};

mod schemas;

use schemas::{
    anthropic_count_message_tokens_request_body, anthropic_count_tokens_responses,
    anthropic_message_request_body, anthropic_responses, native_create_run_request_body,
    native_create_run_responses, native_get_run_responses, native_model_list_responses,
    native_resume_run_request_body, native_resume_run_responses, native_upload_file_request_body,
    native_upload_responses, openai_chat_completion_request_body, openai_compact_request_body,
    openai_compact_responses, openai_model_list_responses, openai_response_create_request_body,
    openai_response_responses, openai_responses, operation_request_body, operation_responses,
};

const NATIVE_CATEGORY_ID: &str = "application-native-api";
const OPENAI_CATEGORY_ID: &str = "openai-compatible-api";
const ANTHROPIC_CATEGORY_ID: &str = "anthropic-compatible-api";

type RequestBodyBuilder = fn(&DocTextResolver) -> Value;
type ResponseBuilder = fn(&DocTextResolver) -> Value;

#[derive(Debug, Clone)]
pub struct ApplicationPublicDocsContext {
    pub application: ApplicationRecord,
    pub active_publication: Option<ApplicationPublicationVersionRecord>,
    pub locale: String,
}

#[derive(Debug, Clone, Copy)]
struct PublicOperation {
    id: &'static str,
    method: &'static str,
    path: &'static str,
    category_id: &'static str,
    doc_key: &'static str,
    request_body: Option<RequestBodyBuilder>,
    responses: ResponseBuilder,
    notes: OperationNotes,
}

#[derive(Debug, Clone, Copy)]
enum OperationNotes {
    CategoryLimitations,
    Text {
        zh_hans: &'static str,
        en_us: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DocsLocale {
    EnUs,
    ZhHans,
}

fn docs_locale(context: &ApplicationPublicDocsContext) -> DocsLocale {
    match context.locale.as_str() {
        "zh_Hans" => DocsLocale::ZhHans,
        _ => DocsLocale::EnUs,
    }
}

pub fn build_application_public_docs_catalog(
    context: &ApplicationPublicDocsContext,
) -> DocsCatalog {
    let locale = docs_locale(context);
    let operations = public_operations();
    let categories = [
        (
            NATIVE_CATEGORY_ID,
            category_label(NATIVE_CATEGORY_ID, locale),
        ),
        (
            OPENAI_CATEGORY_ID,
            category_label(OPENAI_CATEGORY_ID, locale),
        ),
        (
            ANTHROPIC_CATEGORY_ID,
            category_label(ANTHROPIC_CATEGORY_ID, locale),
        ),
    ]
    .into_iter()
    .map(|(id, label)| DocsCatalogCategory {
        id: id.to_string(),
        label: label.unwrap_or(id).to_string(),
        operation_count: operations
            .iter()
            .filter(|operation| operation.category_id == id)
            .count(),
    })
    .collect();

    DocsCatalog {
        title: match locale {
            DocsLocale::ZhHans => "应用公开 API".to_string(),
            DocsLocale::EnUs => "Application Public API".to_string(),
        },
        version: "v1".to_string(),
        categories,
    }
}

pub fn build_application_public_docs_category_operations(
    context: &ApplicationPublicDocsContext,
    category_id: &str,
) -> Option<DocsCatalogCategoryOperations> {
    let locale = docs_locale(context);
    let operations = public_operations()
        .iter()
        .filter(|operation| operation.category_id == category_id)
        .map(|operation| to_catalog_operation(operation, locale))
        .collect::<Vec<_>>();
    if operations.is_empty() {
        return None;
    }
    let label = category_label(category_id, locale)?;
    Some(DocsCatalogCategoryOperations {
        id: category_id.to_string(),
        label: label.to_string(),
        operations,
    })
}

pub fn build_application_public_docs_category_spec(
    context: &ApplicationPublicDocsContext,
    category_id: &str,
) -> Option<Value> {
    let operations = public_operations()
        .iter()
        .filter(|operation| operation.category_id == category_id)
        .collect::<Vec<_>>();
    if operations.is_empty() {
        return None;
    }
    Some(openapi_spec(context, operations))
}

pub fn build_application_public_docs_operation_spec(
    context: &ApplicationPublicDocsContext,
    operation_id: &str,
) -> Option<Value> {
    public_operations()
        .iter()
        .find(|operation| operation.id == operation_id)
        .map(|operation| openapi_spec(context, vec![operation]))
}

#[derive(Debug, Clone, Copy)]
struct DocTextResolver {
    locale: DocsLocale,
}

impl DocTextResolver {
    fn new(locale: DocsLocale) -> Self {
        Self { locale }
    }

    fn operation_summary(&self, doc_key: &str) -> &'static str {
        match (doc_key, self.locale) {
            ("application_public_api.native.create_run", DocsLocale::ZhHans) => "创建原生公开运行",
            ("application_public_api.native.get_run", DocsLocale::ZhHans) => "获取原生公开运行",
            ("application_public_api.native.cancel_run", DocsLocale::ZhHans) => "取消原生公开运行",
            ("application_public_api.native.resume_run", DocsLocale::ZhHans) => "恢复原生公开运行",
            ("application_public_api.native.upload_file", DocsLocale::ZhHans) => "上传原生公开文件",
            ("application_public_api.native.list_models", DocsLocale::ZhHans) => {
                "拉取原生模型能力列表"
            }
            ("application_public_api.openai.chat_completion", DocsLocale::ZhHans) => {
                "创建 OpenAI 兼容聊天补全"
            }
            ("application_public_api.openai.response", DocsLocale::ZhHans) => {
                "创建 OpenAI 兼容响应"
            }
            ("application_public_api.openai.response_compact", DocsLocale::ZhHans) => {
                "创建 OpenAI 兼容紧凑响应"
            }
            ("application_public_api.openai.list_models", DocsLocale::ZhHans) => {
                "拉取 OpenAI 兼容模型列表"
            }
            ("application_public_api.anthropic.message", DocsLocale::ZhHans) => {
                "创建 Anthropic 兼容消息"
            }
            ("application_public_api.anthropic.count_message_tokens", DocsLocale::ZhHans) => {
                "统计 Anthropic 兼容消息输入 tokens"
            }
            ("application_public_api.native.create_run", DocsLocale::EnUs) => {
                "Create Native public run"
            }
            ("application_public_api.native.get_run", DocsLocale::EnUs) => "Get Native public run",
            ("application_public_api.native.cancel_run", DocsLocale::EnUs) => {
                "Cancel Native public run"
            }
            ("application_public_api.native.resume_run", DocsLocale::EnUs) => {
                "Resume Native public run"
            }
            ("application_public_api.native.upload_file", DocsLocale::EnUs) => {
                "Upload Native public file"
            }
            ("application_public_api.native.list_models", DocsLocale::EnUs) => {
                "List Native model capabilities"
            }
            ("application_public_api.openai.chat_completion", DocsLocale::EnUs) => {
                "Create OpenAI-compatible chat completion"
            }
            ("application_public_api.openai.response", DocsLocale::EnUs) => {
                "Create OpenAI-compatible response"
            }
            ("application_public_api.openai.response_compact", DocsLocale::EnUs) => {
                "Create OpenAI-compatible compact response"
            }
            ("application_public_api.openai.list_models", DocsLocale::EnUs) => {
                "List OpenAI-compatible models"
            }
            ("application_public_api.anthropic.message", DocsLocale::EnUs) => {
                "Create Anthropic-compatible message"
            }
            ("application_public_api.anthropic.count_message_tokens", DocsLocale::EnUs) => {
                "Count Anthropic-compatible message input tokens"
            }
            _ => "Public API operation",
        }
    }

    fn operation_description(&self, doc_key: &str) -> &'static str {
        match (doc_key, self.locale) {
            ("application_public_api.native.create_run", DocsLocale::ZhHans) => {
                "基于当前应用的活跃发布版本创建一次运行。"
            }
            ("application_public_api.native.get_run", DocsLocale::ZhHans) => {
                "读取由当前应用 API 密钥创建的公开运行。"
            }
            ("application_public_api.native.cancel_run", DocsLocale::ZhHans) => {
                "取消由当前应用 API 密钥创建的公开运行。"
            }
            ("application_public_api.native.resume_run", DocsLocale::ZhHans) => {
                "完成原生公开运行中等待回调的任务。"
            }
            ("application_public_api.native.upload_file", DocsLocale::ZhHans) => {
                "上传可供原生公开运行使用的文件。"
            }
            ("application_public_api.native.list_models", DocsLocale::ZhHans) => {
                "读取当前应用活跃发布版本中起始节点声明的模型能力目录。"
            }
            ("application_public_api.openai.chat_completion", DocsLocale::ZhHans) => {
                "将 OpenAI Chat Completions 请求适配为原生公开运行。"
            }
            ("application_public_api.openai.response", DocsLocale::ZhHans) => {
                "将 OpenAI Responses 请求适配为原生公开运行，previous_response_id 会解析回原生公开运行。"
            }
            ("application_public_api.openai.response_compact", DocsLocale::ZhHans) => {
                "调用当前发布版本的 Responses Compact provider 绑定，并原样返回 provider 的 ResponseItem 数组；不会创建公开运行。"
            }
            ("application_public_api.openai.list_models", DocsLocale::ZhHans) => {
                "读取当前应用活跃发布版本中起始节点暴露的 OpenAI 兼容模型列表。"
            }
            ("application_public_api.anthropic.message", DocsLocale::ZhHans) => {
                "将 Anthropic Messages 请求适配为原生公开运行。"
            }
            ("application_public_api.anthropic.count_message_tokens", DocsLocale::ZhHans) => {
                "将 Anthropic Messages 请求转为当前发布版本绑定的 provider CountTokens 调用。provider 原生计数不可用时，host 使用带 method、coverage、unknown_block_count 与 fallback_reason 的本地总估算生成 input_tokens；未绑定目标仍返回明确错误，且不创建原生公开运行。"
            }
            ("application_public_api.native.create_run", DocsLocale::EnUs) => {
                "Creates a run against the active published application version."
            }
            ("application_public_api.native.get_run", DocsLocale::EnUs) => {
                "Reads a public run created by this application API key."
            }
            ("application_public_api.native.cancel_run", DocsLocale::EnUs) => {
                "Cancels a public run created by this application API key."
            }
            ("application_public_api.native.resume_run", DocsLocale::EnUs) => {
                "Completes a waiting callback task for a Native public run."
            }
            ("application_public_api.native.upload_file", DocsLocale::EnUs) => {
                "Uploads a file for use by Native public runs."
            }
            ("application_public_api.native.list_models", DocsLocale::EnUs) => {
                "Lists model capabilities declared by the active published application's start node."
            }
            ("application_public_api.openai.chat_completion", DocsLocale::EnUs) => {
                "Adapts an OpenAI Chat Completions request to a Native public run."
            }
            ("application_public_api.openai.response", DocsLocale::EnUs) => {
                "Adapts an OpenAI Responses request to a Native public run and resolves previous_response_id back to a Native public run."
            }
            ("application_public_api.openai.response_compact", DocsLocale::EnUs) => {
                "Calls the active published Responses Compact provider binding and returns its ResponseItem array unchanged; no public run is created."
            }
            ("application_public_api.openai.list_models", DocsLocale::EnUs) => {
                "Lists OpenAI-compatible models exposed by the active published application's start node."
            }
            ("application_public_api.anthropic.message", DocsLocale::EnUs) => {
                "Adapts an Anthropic Messages request to a Native public run."
            }
            ("application_public_api.anthropic.count_message_tokens", DocsLocale::EnUs) => {
                "Translates an Anthropic Messages request into the active published binding's provider CountTokens call. When native provider counting is unavailable, the host derives input_tokens from a total local estimate carrying method, coverage, unknown_block_count, and fallback_reason; an unbound target still returns an explicit error and never creates a Native public run."
            }
            _ => "Public API operation.",
        }
    }

    fn field_description(&self, key: &str) -> &'static str {
        match (key, self.locale) {
            ("application_public_api.native.create_run.request.query", DocsLocale::ZhHans) => {
                "用户输入，会映射到当前应用发布配置中的 query target。"
            }
            ("application_public_api.native.create_run.request.model", DocsLocale::ZhHans) => {
                "可选模型标识，会通过应用 API 映射传入运行。"
            }
            ("application_public_api.native.create_run.request.system", DocsLocale::ZhHans) => {
                "系统指令上下文，会与历史中的旧 system 消息合并为运行级 system。"
            }
            ("application_public_api.native.create_run.request.inputs", DocsLocale::ZhHans) => {
                "附加输入对象，会通过应用 API 映射写入目标节点。"
            }
            ("application_public_api.native.create_run.request.history", DocsLocale::ZhHans) => {
                "可供发布运行使用的 user / assistant / tool 会话历史。"
            }
            (
                "application_public_api.native.create_run.request.attachments",
                DocsLocale::ZhHans,
            ) => "可供发布运行使用的文件或外部资源。",
            (
                "application_public_api.native.create_run.request.attachments.value",
                DocsLocale::ZhHans,
            ) => "当前附件来源对应的值。",
            (
                "application_public_api.native.create_run.request.conversation",
                DocsLocale::ZhHans,
            ) => "外部会话元数据，例如用户或会话 ID。",
            ("application_public_api.native.create_run.request.expand_id", DocsLocale::ZhHans) => {
                "显式外部用户 ID。会写入公开运行日志，并优先于 conversation.user。"
            }
            ("application_public_api.native.create_run.request.title", DocsLocale::ZhHans) => {
                "运行标题。未传时默认使用用户输入，并截断到 255 个字符。"
            }
            (
                "application_public_api.native.create_run.request.stream_options",
                DocsLocale::ZhHans,
            ) => "流式返回选项，include_workflow_events=public 会启用公开工作流事件。",
            ("application_public_api.native.create_run.request.execution", DocsLocale::ZhHans) => {
                "发布运行的执行选项。支持 model_parameters.max_output_tokens 与 model_parameters.reasoning 作为运行时大语言模型参数；execution.compatibility_mode 不受支持。"
            }
            ("application_public_api.native.create_run.request.metadata", DocsLocale::ZhHans) => {
                "调用方 metadata 只会在初始运行响应中回显；持久化的公开运行字段仅限已定义的 trace 与会话标识。"
            }
            ("application_public_api.anthropic.message.request.metadata", DocsLocale::ZhHans) => {
                "附加元数据。metadata.expand_id 会映射为公开运行的外部用户标识。"
            }
            (
                "application_public_api.native.resume_run.request.callback_task_id",
                DocsLocale::ZhHans,
            ) => "required_action 返回的回调任务 ID。",
            (
                "application_public_api.native.upload_file.request.file_table_id",
                DocsLocale::ZhHans,
            ) => "目标文件表 ID。",
            ("application_public_api.native.upload_file.request.file", DocsLocale::ZhHans) => {
                "要上传的文件二进制内容。"
            }
            (
                "application_public_api.openai.chat_completion.request.stream",
                DocsLocale::ZhHans,
            )
            | ("application_public_api.openai.response.request.stream", DocsLocale::ZhHans)
            | ("application_public_api.anthropic.message.request.stream", DocsLocale::ZhHans) => {
                "true 会将请求映射为流式响应模式。"
            }
            ("application_public_api.native.create_run.request.query", DocsLocale::EnUs) => {
                "User input mapped to the published application's query target."
            }
            ("application_public_api.native.create_run.request.model", DocsLocale::EnUs) => {
                "Optional model identifier passed through application mapping."
            }
            ("application_public_api.native.create_run.request.system", DocsLocale::EnUs) => {
                "System instruction context merged with legacy system messages from history."
            }
            ("application_public_api.native.create_run.request.inputs", DocsLocale::EnUs) => {
                "Additional input values mapped by the application API mapping."
            }
            ("application_public_api.native.create_run.request.history", DocsLocale::EnUs) => {
                "User, assistant, and tool conversation history entries available to the published run."
            }
            ("application_public_api.native.create_run.request.attachments", DocsLocale::EnUs) => {
                "Files or external assets available to the published run."
            }
            (
                "application_public_api.native.create_run.request.attachments.value",
                DocsLocale::EnUs,
            ) => "Attachment value for the selected source.",
            ("application_public_api.native.create_run.request.conversation", DocsLocale::EnUs) => {
                "External conversation metadata such as user or conversation id."
            }
            ("application_public_api.native.create_run.request.expand_id", DocsLocale::EnUs) => {
                "Explicit external user id persisted on the public run and preferred over conversation.user."
            }
            ("application_public_api.native.create_run.request.title", DocsLocale::EnUs) => {
                "Run title. Defaults to the user query and is truncated to 255 characters."
            }
            (
                "application_public_api.native.create_run.request.stream_options",
                DocsLocale::EnUs,
            ) => {
                "Streaming options. include_workflow_events=public enables public workflow events."
            }
            ("application_public_api.native.create_run.request.execution", DocsLocale::EnUs) => {
                "Execution options for the published run. Supports model_parameters.max_output_tokens and model_parameters.reasoning as runtime language-model parameters; execution.compatibility_mode is unsupported."
            }
            ("application_public_api.native.create_run.request.metadata", DocsLocale::EnUs) => {
                "Caller metadata is echoed only in the initial run response; durable public-run fields are limited to defined trace and conversation identities."
            }
            ("application_public_api.anthropic.message.request.metadata", DocsLocale::EnUs) => {
                "Additional metadata. metadata.expand_id maps to the public run external user id."
            }
            (
                "application_public_api.native.resume_run.request.callback_task_id",
                DocsLocale::EnUs,
            ) => "Callback task id returned in required_action.",
            (
                "application_public_api.native.upload_file.request.file_table_id",
                DocsLocale::EnUs,
            ) => "Target file table id.",
            ("application_public_api.native.upload_file.request.file", DocsLocale::EnUs) => {
                "File binary content."
            }
            ("application_public_api.openai.chat_completion.request.stream", DocsLocale::EnUs)
            | ("application_public_api.openai.response.request.stream", DocsLocale::EnUs)
            | ("application_public_api.anthropic.message.request.stream", DocsLocale::EnUs) => {
                "true maps the request to streaming response mode."
            }
            _ => "Field value.",
        }
    }

    fn response_description(&self, key: &str) -> &'static str {
        match (key, self.locale) {
            ("compatible_response", DocsLocale::ZhHans) => "兼容响应",
            ("compatible_token_count", DocsLocale::ZhHans) => "兼容输入 token 统计",
            ("published_count_tokens_unavailable", DocsLocale::ZhHans) => {
                "已发布 CountTokens 操作未绑定或目标不可用"
            }
            ("provider_rate_limited", DocsLocale::ZhHans) => "provider token 统计请求被限流",
            ("provider_count_tokens_failure", DocsLocale::ZhHans) => {
                "provider token 统计请求失败或返回无效响应"
            }
            ("published_compact_unavailable", DocsLocale::ZhHans) => {
                "已发布 Compact 操作未绑定、目标不可用或 provider 未声明该能力"
            }
            ("provider_compact_failure", DocsLocale::ZhHans) => {
                "provider Compact 请求失败、被限流或返回无效响应"
            }
            ("compatible_model_list", DocsLocale::ZhHans) => "OpenAI 兼容模型列表",
            ("native_model_list", DocsLocale::ZhHans) => "原生模型能力列表",
            ("native_run", DocsLocale::ZhHans) => "原生运行",
            ("native_run_created", DocsLocale::ZhHans) => "原生运行已创建",
            ("file_uploaded", DocsLocale::ZhHans) => "文件已上传",
            ("invalid_request", DocsLocale::ZhHans) => "请求无效",
            ("invalid_application_api_key", DocsLocale::ZhHans) => "应用 API 密钥无效",
            ("forbidden", DocsLocale::ZhHans) => "无权访问该公开资源",
            ("not_found", DocsLocale::ZhHans) => "公开资源不存在",
            ("application_not_published_or_run_state_not_supported", DocsLocale::ZhHans) => {
                "应用未发布，或运行状态不支持当前操作"
            }
            ("compatible_response", DocsLocale::EnUs) => "Compatible response",
            ("compatible_token_count", DocsLocale::EnUs) => "Compatible input token count",
            ("published_count_tokens_unavailable", DocsLocale::EnUs) => {
                "Published CountTokens operation is unbound or unavailable"
            }
            ("provider_rate_limited", DocsLocale::EnUs) => "Provider CountTokens request was rate limited",
            ("provider_count_tokens_failure", DocsLocale::EnUs) => {
                "Provider CountTokens request failed or returned an invalid response"
            }
            ("published_compact_unavailable", DocsLocale::EnUs) => {
                "Published Compact operation is unbound, unavailable, or not declared by its provider"
            }
            ("provider_compact_failure", DocsLocale::EnUs) => {
                "Provider Compact request failed, was rate limited, or returned an invalid response"
            }
            ("compatible_model_list", DocsLocale::EnUs) => "OpenAI-compatible model list",
            ("native_model_list", DocsLocale::EnUs) => "Native model capability list",
            ("native_run", DocsLocale::EnUs) => "Native run",
            ("native_run_created", DocsLocale::EnUs) => "Native run created",
            ("file_uploaded", DocsLocale::EnUs) => "File uploaded",
            ("invalid_request", DocsLocale::EnUs) => "Invalid request",
            ("invalid_application_api_key", DocsLocale::EnUs) => "Invalid application API key",
            ("forbidden", DocsLocale::EnUs) => "Forbidden public resource",
            ("not_found", DocsLocale::EnUs) => "Public resource not found",
            ("application_not_published_or_run_state_not_supported", DocsLocale::EnUs) => {
                "Application is not published or run state is not supported"
            }
            _ => "Response",
        }
    }

    fn operation_notes(&self, operation: &PublicOperation) -> &'static str {
        match operation.notes {
            OperationNotes::CategoryLimitations => self.unsupported_notes(operation.category_id),
            OperationNotes::Text { zh_hans, en_us } => match self.locale {
                DocsLocale::ZhHans => zh_hans,
                DocsLocale::EnUs => en_us,
            },
        }
    }

    fn unsupported_notes(&self, category_id: &str) -> &'static str {
        match (category_id, self.locale) {
            (OPENAI_CATEGORY_ID, DocsLocale::ZhHans) => {
                "此 v1 兼容端点支持 Chat Completions 和 Responses 外层协议适配，tools、tool_choice、function_call 字段和图片输入会进入模型供应商调用，支持 tool 消息历史回传和返回 tool_calls；stream=true 时返回 text/event-stream，心跳文本为 heartbeat，推理增量会投影为对应协议事件。暂不支持音频输出、文件内容和多模态输出生成。如需查看 required_action 或恢复运行，请使用原生 API。"
            }
            (ANTHROPIC_CATEGORY_ID, DocsLocale::ZhHans) => {
                "此 v1 兼容端点支持顶层 tools/tool_choice 与 image/document blocks 进入模型供应商调用，支持 tool_use / tool_result 文本块历史回传；stream=true 时返回 text/event-stream，心跳文本为 heartbeat，推理增量映射为 thinking_delta。暂不支持 computer use 和等待态恢复。如需查看 required_action 或恢复运行，请使用原生 API。"
            }
            (_, DocsLocale::ZhHans) => {
                "原生 API 支持查看 required_action 并恢复运行。response_mode=streaming 时返回 text/event-stream，并包含心跳、reasoning.delta、message.delta 和终态事件。公开路径不会包含 application_id。"
            }
            (OPENAI_CATEGORY_ID, DocsLocale::EnUs) => {
                "This v1 compatible endpoint adapts Chat Completions and Responses protocol shapes, forwards tools, tool_choice, and function_call fields plus image input into the model-provider invocation, forwards tool message history, and can return tool_calls. stream=true returns text/event-stream with heartbeat text heartbeat and protocol-shaped events. Unsupported: audio output, file content, and multimodal output generation. Use the Native API for required_action inspection and resume."
            }
            (ANTHROPIC_CATEGORY_ID, DocsLocale::EnUs) => {
                "This v1 compatible endpoint forwards top-level tools/tool_choice and image/document blocks into the model-provider invocation and supports tool_use/tool_result text block history. stream=true returns text/event-stream with heartbeat text heartbeat and reasoning deltas as thinking_delta. Unsupported: computer use and waiting-state resume. Use the Native API for required_action inspection and resume."
            }
            (_, DocsLocale::EnUs) => {
                "Native API supports required_action inspection and resume. response_mode=streaming returns text/event-stream with heartbeat, reasoning.delta, message.delta, and terminal run events. Public paths never include application_id."
            }
        }
    }
}

fn category_label(category_id: &str, locale: DocsLocale) -> Option<&'static str> {
    match (category_id, locale) {
        (NATIVE_CATEGORY_ID, DocsLocale::ZhHans) => Some("应用原生 API"),
        (OPENAI_CATEGORY_ID, DocsLocale::ZhHans) => Some("OpenAI 兼容 API"),
        (ANTHROPIC_CATEGORY_ID, DocsLocale::ZhHans) => Some("Anthropic 兼容 API"),
        (NATIVE_CATEGORY_ID, DocsLocale::EnUs) => Some("Application Native API"),
        (OPENAI_CATEGORY_ID, DocsLocale::EnUs) => Some("OpenAI Compatible API"),
        (ANTHROPIC_CATEGORY_ID, DocsLocale::EnUs) => Some("Anthropic Compatible API"),
        _ => None,
    }
}

fn to_catalog_operation(operation: &PublicOperation, locale: DocsLocale) -> DocsCatalogOperation {
    let docs = DocTextResolver::new(locale);
    let category_label =
        category_label(operation.category_id, locale).unwrap_or(operation.category_id);
    DocsCatalogOperation {
        id: operation.id.to_string(),
        method: operation.method.to_string(),
        path: operation.path.to_string(),
        summary: Some(docs.operation_summary(operation.doc_key).to_string()),
        description: Some(docs.operation_description(operation.doc_key).to_string()),
        tags: vec![category_label.to_string()],
        group: category_label.to_string(),
        deprecated: false,
    }
}

fn openapi_spec(
    context: &ApplicationPublicDocsContext,
    operations: Vec<&PublicOperation>,
) -> Value {
    let locale = docs_locale(context);
    let mut paths = serde_json::Map::new();
    for operation in operations {
        let method = operation.method.to_ascii_lowercase();
        let path_item = paths
            .entry(operation.path.to_string())
            .or_insert_with(|| json!({}));
        path_item
            .as_object_mut()
            .expect("path item object")
            .insert(method, operation_openapi_spec(operation, locale));
    }

    json!({
        "openapi": "3.1.0",
        "info": {
            "title": application_title(context),
            "version": publication_version(context),
            "description": application_description(context),
        },
        "servers": [{"url": "/"}],
        "paths": paths,
        "components": {
            "securitySchemes": {
                "applicationApiKey": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "Application API Key",
                    "description": security_scheme_description(locale)
                },
                "anthropicApplicationApiKey": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "x-api-key",
                    "description": security_scheme_description(locale)
                }
            }
        },
        "x-1flowbase-application": {
            "id": context.application.id,
            "name": context.application.name,
            "api_enabled": context
                .active_publication
                .as_ref()
                .map(|publication| publication.api_enabled)
                .unwrap_or(false),
            "active_publication_version": context
                .active_publication
                .as_ref()
                .map(|publication| publication.version_sequence),
            "mapping": context
                .active_publication
                .as_ref()
                .map(mapping_summary)
                .unwrap_or_else(|| json!({"status": "not_published"}))
        }
    })
}

fn operation_openapi_spec(operation: &PublicOperation, locale: DocsLocale) -> Value {
    let docs = DocTextResolver::new(locale);
    let mut spec = json!({
        "operationId": operation.id,
        "summary": docs.operation_summary(operation.doc_key),
        "description": format!("{}\n\n{}", docs.operation_description(operation.doc_key), docs.operation_notes(operation)),
        "tags": [category_label(operation.category_id, locale).unwrap_or(operation.category_id)],
        "responses": operation_responses(operation, &docs),
        "security": operation_security(operation.category_id)
    });
    let spec_object = spec.as_object_mut().expect("operation spec object");
    let parameters = operation_parameters(operation);
    if !parameters.is_empty() {
        spec_object.insert("parameters".to_string(), Value::Array(parameters));
    }
    if let Some(request_body) = operation_request_body(operation, &docs) {
        spec_object.insert("requestBody".to_string(), request_body);
    }
    spec
}

fn operation_parameters(operation: &PublicOperation) -> Vec<Value> {
    let mut parameters = Vec::new();
    if operation.path.contains("{run_id}") {
        parameters.push(json!({
            "name": "run_id",
            "in": "path",
            "required": true,
            "description": "Published run id",
            "schema": {
                "type": "string",
                "format": "uuid"
            }
        }));
    }
    parameters
}

fn operation_security(category_id: &str) -> Value {
    if category_id == ANTHROPIC_CATEGORY_ID {
        return json!([
            {"applicationApiKey": []},
            {"anthropicApplicationApiKey": []}
        ]);
    }

    json!([{"applicationApiKey": []}])
}

fn application_description(context: &ApplicationPublicDocsContext) -> String {
    let locale = docs_locale(context);
    let publication = context
        .active_publication
        .as_ref()
        .map(|publication| match locale {
            DocsLocale::ZhHans => {
                if publication.api_enabled {
                    format!("当前启用的是发布版本 v{}。", publication.version_sequence)
                } else {
                    format!("当前发布版本 v{} 未启用。", publication.version_sequence)
                }
            }
            DocsLocale::EnUs => {
                format!(
                    "Active publication v{} is {}.",
                    publication.version_sequence,
                    if publication.api_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                )
            }
        })
        .unwrap_or_else(|| match locale {
            DocsLocale::ZhHans => "当前没有活跃的公开 API 发布版本。".to_string(),
            DocsLocale::EnUs => "No active public API publication exists.".to_string(),
        });
    match locale {
        DocsLocale::ZhHans => format!(
            "{} 的应用级公开 API 文档。{}公开路径由应用 API 密钥选择，不通过 application_id 选择。",
            context.application.name, publication
        ),
        DocsLocale::EnUs => format!(
            "Application-scoped public API docs for {}. {} Public paths are selected by application API key, not by application_id.",
            context.application.name, publication
        ),
    }
}

fn application_title(context: &ApplicationPublicDocsContext) -> String {
    match docs_locale(context) {
        DocsLocale::ZhHans => format!("{} 公开 API", context.application.name),
        DocsLocale::EnUs => format!("{} Public API", context.application.name),
    }
}

fn publication_version(context: &ApplicationPublicDocsContext) -> String {
    context
        .active_publication
        .as_ref()
        .map(|publication| format!("publication-v{}", publication.version_sequence))
        .unwrap_or_else(|| "unpublished".to_string())
}

fn mapping_summary(publication: &ApplicationPublicationVersionRecord) -> Value {
    json!({
        "query_target": publication.mapping_snapshot.input.query_target,
        "model_target": publication.mapping_snapshot.input.model_target,
        "inputs_target": publication.mapping_snapshot.input.inputs_target,
        "history_target": publication.mapping_snapshot.input.history_target,
        "attachments_target": publication.mapping_snapshot.input.attachments_target,
        "answer_selector": publication.mapping_snapshot.output.answer_selector,
        "usage_selector": publication.mapping_snapshot.output.usage_selector,
        "files_selector": publication.mapping_snapshot.output.files_selector,
        "error_selector": publication.mapping_snapshot.output.error_selector,
    })
}

fn security_scheme_description(locale: DocsLocale) -> &'static str {
    match locale {
        DocsLocale::ZhHans => "使用在当前应用 API 页签中创建的应用 API 密钥。",
        DocsLocale::EnUs => "Use an application API key created from this application API tab.",
    }
}

fn public_operations() -> &'static [PublicOperation] {
    PUBLIC_OPERATION_REGISTRY
}

static PUBLIC_OPERATION_REGISTRY: &[PublicOperation] = &[
    PublicOperation {
        id: "applicationNativeCreateRun",
        method: "POST",
        path: "/api/agent/v1/runs",
        category_id: NATIVE_CATEGORY_ID,
        doc_key: "application_public_api.native.create_run",
        request_body: Some(native_create_run_request_body),
        responses: native_create_run_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationNativeGetRun",
        method: "GET",
        path: "/api/agent/v1/runs/{run_id}",
        category_id: NATIVE_CATEGORY_ID,
        doc_key: "application_public_api.native.get_run",
        request_body: None,
        responses: native_get_run_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationNativeCancelRun",
        method: "POST",
        path: "/api/agent/v1/runs/{run_id}/cancel",
        category_id: NATIVE_CATEGORY_ID,
        doc_key: "application_public_api.native.cancel_run",
        request_body: None,
        responses: native_get_run_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationNativeResumeRun",
        method: "POST",
        path: "/api/agent/v1/runs/{run_id}/resume",
        category_id: NATIVE_CATEGORY_ID,
        doc_key: "application_public_api.native.resume_run",
        request_body: Some(native_resume_run_request_body),
        responses: native_resume_run_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationNativeUploadFile",
        method: "POST",
        path: "/api/agent/v1/files",
        category_id: NATIVE_CATEGORY_ID,
        doc_key: "application_public_api.native.upload_file",
        request_body: Some(native_upload_file_request_body),
        responses: native_upload_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationNativeListModels",
        method: "GET",
        path: "/api/agent/v1/models",
        category_id: NATIVE_CATEGORY_ID,
        doc_key: "application_public_api.native.list_models",
        request_body: None,
        responses: native_model_list_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationOpenAiCreateChatCompletion",
        method: "POST",
        path: "/v1/chat/completions",
        category_id: OPENAI_CATEGORY_ID,
        doc_key: "application_public_api.openai.chat_completion",
        request_body: Some(openai_chat_completion_request_body),
        responses: openai_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationOpenAiCreateResponse",
        method: "POST",
        path: "/v1/responses",
        category_id: OPENAI_CATEGORY_ID,
        doc_key: "application_public_api.openai.response",
        request_body: Some(openai_response_create_request_body),
        responses: openai_response_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationOpenAiCreateResponseCompact",
        method: "POST",
        path: "/v1/responses/compact",
        category_id: OPENAI_CATEGORY_ID,
        doc_key: "application_public_api.openai.response_compact",
        request_body: Some(openai_compact_request_body),
        responses: openai_compact_responses,
        notes: OperationNotes::Text {
            zh_hans: "此端点只调用当前发布 Responses Compact 绑定的 provider，并返回其原始 ResponseItem 数组。它不会创建 flow run、运行时事件流或本地摘要；未绑定、不可用或 provider 失败会返回明确错误。",
            en_us: "This endpoint calls only the active published Responses Compact provider binding and returns its raw ResponseItem array. It never creates a flow run, runtime event stream, or local summary; unbound, unavailable, and provider failures are explicit errors.",
        },
    },
    PublicOperation {
        id: "applicationOpenAiListModels",
        method: "GET",
        path: "/v1/models",
        category_id: OPENAI_CATEGORY_ID,
        doc_key: "application_public_api.openai.list_models",
        request_body: None,
        responses: openai_model_list_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationAnthropicCreateMessage",
        method: "POST",
        path: "/v1/messages",
        category_id: ANTHROPIC_CATEGORY_ID,
        doc_key: "application_public_api.anthropic.message",
        request_body: Some(anthropic_message_request_body),
        responses: anthropic_responses,
        notes: OperationNotes::CategoryLimitations,
    },
    PublicOperation {
        id: "applicationAnthropicCountMessageTokens",
        method: "POST",
        path: "/v1/messages/count_tokens",
        category_id: ANTHROPIC_CATEGORY_ID,
        doc_key: "application_public_api.anthropic.count_message_tokens",
        request_body: Some(anthropic_count_message_tokens_request_body),
        responses: anthropic_count_tokens_responses,
        notes: OperationNotes::Text {
            zh_hans: "该端点调用当前发布 CountTokens 绑定的 provider；provider 原生计数不可用时，host 以可观察 method、coverage、unknown_block_count 与 fallback_reason 的本地总估算生成 input_tokens，且不写入原生公开运行记录。",
            en_us: "This endpoint calls the current published CountTokens provider binding. When native provider counting is unavailable, the host derives input_tokens from a total local estimate with observable method, coverage, unknown_block_count, and fallback_reason, without persisting a Native public run.",
        },
    },
];

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use utoipa::OpenApi;

    #[test]
    fn public_operation_registry_declares_unique_documented_operations() {
        let docs = DocTextResolver::new(DocsLocale::EnUs);
        let mut ids = HashSet::new();
        let mut routes = HashSet::new();

        for operation in public_operations() {
            assert!(ids.insert(operation.id), "duplicate operation id");
            assert!(
                routes.insert((operation.method, operation.path)),
                "duplicate public route"
            );
            assert!(
                category_label(operation.category_id, DocsLocale::EnUs).is_some(),
                "unknown category {}",
                operation.category_id
            );
            assert!(
                !operation_responses(operation, &docs)
                    .as_object()
                    .expect("operation responses should be an object")
                    .is_empty(),
                "operation {} must declare responses",
                operation.id
            );
            if let Some(request_body) = operation_request_body(operation, &docs) {
                assert!(
                    request_body.as_object().is_some(),
                    "operation {} request body should be an object",
                    operation.id
                );
            }
        }
    }

    #[test]
    fn public_operation_registry_matches_global_openapi_paths() {
        let spec = serde_json::to_value(crate::openapi::ApiDoc::openapi())
            .expect("global openapi should serialize");
        let paths = spec["paths"]
            .as_object()
            .expect("global openapi paths should be an object");

        for operation in public_operations() {
            let Some(path_item) = paths.get(operation.path) else {
                panic!("global openapi missing path {}", operation.path);
            };
            let method = operation.method.to_ascii_lowercase();
            assert!(
                path_item.get(&method).is_some(),
                "global openapi missing {} {}",
                operation.method,
                operation.path
            );
        }
    }
}
