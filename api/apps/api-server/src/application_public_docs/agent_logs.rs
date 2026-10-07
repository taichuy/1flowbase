//! Agent Logs uses the existing application documentation projection.
use control_plane::ports::{AgentLogEventKind, AgentLogUsageBasis, AGENT_LOGS_SCHEMA_VERSION};

use super::*;

pub(super) const CATEGORY_ID: &str = "application-log-ingestion-api";
pub(super) const OPERATION_ID: &str = "applicationAgentLogsIngestEvents";
pub(super) const DOC_KEY: &str = "application_public_api.logs.ingest";

pub(super) static OPERATIONS: &[PublicOperation] = &[PublicOperation {
    id: OPERATION_ID,
    method: "POST",
    path: "/api/logs/v1/events",
    category_id: CATEGORY_ID,
    doc_key: DOC_KEY,
    request_body: Some(request_body),
    responses,
    notes: OperationNotes::Text {
        zh_hans: "使用当前应用 API 密钥作为 Bearer token。事件批次原子持久化后返回 ACK；同一 source_id / event_id 重传不重复记录，不同内容复用同一身份返回 409。无需发布应用，不执行模型或扣减余额。",
        en_us: "Authenticate with this application's API key as a Bearer token. The batch is persisted atomically before ACK. Replaying the same source_id / event_id does not duplicate records; reusing its identity with different content returns 409. No publication, model execution, or balance debit is required.",
    },
}];

fn text<'a>(docs: &DocTextResolver, zh: &'a str, en: &'a str) -> &'a str {
    match docs.locale {
        DocsLocale::ZhHans => zh,
        DocsLocale::EnUs => en,
    }
}

fn request_body(docs: &DocTextResolver) -> Value {
    let nullable_string = json!({"type": ["string", "null"]});
    let nullable_tokens = json!({"type": ["integer", "null"], "minimum": 0});
    let schema = json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["schema_version", "source_id", "source_client", "events"],
        "properties": {
            "schema_version": {"type": "string", "enum": [AGENT_LOGS_SCHEMA_VERSION]},
            "source_id": {"type": "string", "description": text(docs, "采集安装的稳定身份。", "Stable identity of the collector installation.")},
            "source_client": {"type": "string", "description": text(docs, "来源客户端，例如 codex。", "Source client, for example codex.")},
            "events": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["event_id", "source_session_id", "source_task_id", "sequence", "occurred_at", "kind"],
                    "properties": {
                        "event_id": {"type": "string", "description": text(docs, "来源事件的稳定身份；重试时保持不变。", "Stable source event identity; keep it unchanged when retrying.")},
                        "source_session_id": {"type": "string"},
                        "source_task_id": {"type": "string"},
                        "parent_source_task_id": nullable_string,
                        "sequence": {"type": "integer", "format": "int64", "minimum": 0},
                        "occurred_at": {"type": "string", "format": "date-time"},
                        "kind": {"type": "string", "enum": [AgentLogEventKind::System, AgentLogEventKind::User, AgentLogEventKind::Assistant, AgentLogEventKind::ToolCall, AgentLogEventKind::ToolResult, AgentLogEventKind::Usage, AgentLogEventKind::Context, AgentLogEventKind::TaskEnd]},
                        "content": nullable_string,
                        "phase": {"type": ["string", "null"], "description": text(docs, "assistant 或 task_end 用 final_answer 声明最终回答；task_end 用 cancelled 声明取消。中间内容保留在客户端轨迹。", "Use final_answer on assistant or task_end to declare the final answer, and cancelled on task_end to declare cancellation. Intermediate content remains in client trajectory.")},
                        "name": nullable_string,
                        "call_id": nullable_string,
                        "model_id": nullable_string,
                        "provider_code": nullable_string,
                        "usage": {
                            "type": ["object", "null"],
                            "additionalProperties": false,
                            "required": ["basis"],
                            "properties": {
                                "basis": {"type": "string", "enum": [AgentLogUsageBasis::Delta, AgentLogUsageBasis::Cumulative], "description": text(docs, "delta 为单次增量，cumulative 为累计快照；不要从文本猜测用量。", "delta is an increment; cumulative is a cumulative snapshot. Do not infer usage from text.")},
                                "response_id": nullable_string,
                                "input_tokens": nullable_tokens,
                                "output_tokens": nullable_tokens,
                                "input_cache_hit_tokens": nullable_tokens,
                                "cache_write_tokens": nullable_tokens,
                                "total_tokens": nullable_tokens
                            }
                        },
                        "inherited": {"type": "boolean", "default": false, "description": text(docs, "继承的历史上下文，不重复计量。", "Inherited historical context, excluded from repeated accounting.")},
                        "raw": {"description": text(docs, "采集到的原始来源数据，按需保留。", "Collected raw source data, retained as needed.")}
                    }
                }
            }
        }
    });
    json!({
        "required": true,
        "content": {"application/json": {
            "schema": schema,
            "example": {
                "schema_version": AGENT_LOGS_SCHEMA_VERSION,
                "source_id": "collector-installation",
                "source_client": "codex",
                "events": [{
                    "event_id": "event-1",
                    "source_session_id": "session-1",
                    "source_task_id": "turn-1",
                    "sequence": 1,
                    "occurred_at": "2026-10-07T08:00:00Z",
                    "kind": "user",
                    "content": "Hello"
                }]
            }
        }}
    })
}

fn responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": {
            "description": text(docs, "批次已持久化；重试包含重复事件时仍返回此 ACK。", "Batch persisted; replaying duplicate events returns the same ACK shape."),
            "content": {"application/json": {
                "schema": {
                    "type": "object",
                    "required": ["data", "meta"],
                    "properties": {
                        "data": {"type": "object", "required": ["accepted_events", "duplicate_events", "record_ids"], "properties": {
                            "accepted_events": {"type": "integer", "minimum": 0},
                            "duplicate_events": {"type": "integer", "minimum": 0},
                            "record_ids": {"type": "array", "items": {"type": "string", "format": "uuid"}}
                        }},
                        "meta": {}
                    }
                },
                "example": {"data": {"accepted_events": 1, "duplicate_events": 0, "record_ids": ["00000000-0000-0000-0000-000000000001"]}, "meta": null}
            }}
        },
        "400": {"description": text(docs, "协议版本或事件字段无效。", "Invalid protocol version or event fields.")},
        "401": {"description": text(docs, "缺少或无效的应用 API 密钥。", "Missing or invalid application API key.")},
        "409": {"description": text(docs, "同一来源事件身份已存在不同内容。", "A source event identity already exists with different content.")},
        "422": {"description": text(docs, "请求 JSON 不符合采集协议类型。", "The request JSON does not match the ingestion protocol types.")}
    })
}
