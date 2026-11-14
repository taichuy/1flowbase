//! Agent Logs uses the existing application documentation projection.
use control_plane::ports::{AgentLogEventKind, AgentLogUsageBasis, AGENT_LOGS_SCHEMA_VERSION};

use super::*;

pub(super) const CATEGORY_ID: &str = "application-log-ingestion-api";
pub(super) const OPERATION_ID: &str = "applicationAgentLogsIngestEvents";
pub(super) const DOC_KEY: &str = "application_public_api.logs.ingest";
pub(super) const OTLP_OPERATION_ID: &str = "applicationAgentLogsIngestOtlpTraces";
pub(super) const OTLP_DOC_KEY: &str = "application_public_api.logs.otlp.traces.ingest";

pub(super) static OPERATIONS: &[PublicOperation] = &[PublicOperation {
    id: OPERATION_ID,
    method: "POST",
    path: "/api/logs/v1/events",
    category_id: CATEGORY_ID,
    doc_key: DOC_KEY,
    request_body: Some(request_body),
    responses,
    notes: OperationNotes::Text {
        zh_hans: "使用当前应用 API 密钥作为 Bearer token。事件批次原子持久化后返回 ACK；同一 source_id / event_id 重传不重复记录，不同内容复用同一身份返回 409。无需发布应用，但必须启用日志接收；停用时返回 403（agent_logs_ingestion_disabled），不写入事件。不执行模型或扣减余额。",
        en_us: "Authenticate with this application's API key as a Bearer token. The batch is persisted atomically before ACK. Replaying the same source_id / event_id does not duplicate records; reusing its identity with different content returns 409. No publication is required, but log reception must be enabled. Disabled reception returns 403 (agent_logs_ingestion_disabled) without storing events. No model execution or balance debit occurs.",
    },
}, PublicOperation {
    id: OTLP_OPERATION_ID,
    method: "POST",
    path: "/api/logs/otlp/v1/traces",
    category_id: CATEGORY_ID,
    doc_key: OTLP_DOC_KEY,
    request_body: Some(otlp_request_body),
    responses: otlp_responses,
    notes: OperationNotes::Text {
        zh_hans: "配置 OTLP endpoint 为 HOST/api/logs/otlp，客户端自动追加 /v1/traces；使用当前应用 API 密钥作为 Bearer token。支持 application/json、application/x-protobuf 和 Content-Encoding: gzip，仅接收 Trace。整请求先校验，各 source 批次原子持久化；全部完成后返回标准 OTLP ACK（JSON {} 或空 protobuf），没有 data/meta 包装。部分 source 已写入后失败时返回 200 partialSuccess 和准确 rejectedSpans，不应重试部分 ACK。日志接收停用或应用类型不支持时返回 403；错误是保留真实 HTTP 状态的标准 google.rpc.Status。没有 publication、模型执行或余额扣减。",
        en_us: "Set the OTLP endpoint to HOST/api/logs/otlp; the client appends /v1/traces. Authenticate with this application's API key as a Bearer token. Supports application/json, application/x-protobuf and Content-Encoding: gzip, for Trace only. The whole request is validated first, and each source batch commits atomically. Full success returns the standard OTLP ACK (JSON {} or empty protobuf), without data/meta. A failure after earlier source commits returns HTTP 200 partialSuccess with the exact rejectedSpans count; do not retry a partial ACK. Disabled reception or an unsupported application type returns 403. Errors use standard google.rpc.Status with the actual HTTP status. No publication, model execution or balance debit is required.",
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
        "403": {"description": text(docs, "日志接收已停用（agent_logs_ingestion_disabled），或应用不支持日志采集。", "Log reception is disabled (agent_logs_ingestion_disabled), or the application does not support log ingestion.")},
        "409": {"description": text(docs, "同一来源事件身份已存在不同内容。", "A source event identity already exists with different content.")},
        "422": {"description": text(docs, "请求 JSON 不符合采集协议类型。", "The request JSON does not match the ingestion protocol types.")}
    })
}

fn otlp_request_body(docs: &DocTextResolver) -> Value {
    let schema = json!({
        "type": "object",
        "properties": {
            "resourceSpans": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "resource": {
                            "type": "object"
                        },
                        "scopeSpans": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "scope": {
                                        "type": "object"
                                    },
                                    "spans": {
                                        "type": "array",
                                        "items": {
                                            "type": "object",
                                            "required": [
                                                "traceId",
                                                "spanId",
                                                "startTimeUnixNano"
                                            ],
                                            "properties": {
                                                "traceId": {
                                                    "type": "string",
                                                    "pattern": "^[a-fA-F0-9]{32}$"
                                                },
                                                "spanId": {
                                                    "type": "string",
                                                    "pattern": "^[a-fA-F0-9]{16}$"
                                                },
                                                "name": {
                                                    "type": "string"
                                                },
                                                "startTimeUnixNano": {
                                                    "oneOf": [
                                                        {
                                                            "type": "string",
                                                            "pattern": "^[0-9]+$"
                                                        },
                                                        {
                                                            "type": "integer",
                                                            "minimum": 0
                                                        }
                                                    ]
                                                },
                                                "endTimeUnixNano": {
                                                    "oneOf": [
                                                        {
                                                            "type": "string",
                                                            "pattern": "^[0-9]+$"
                                                        },
                                                        {
                                                            "type": "integer",
                                                            "minimum": 0
                                                        }
                                                    ]
                                                },
                                                "attributes": {
                                                    "type": "array",
                                                    "items": {
                                                        "type": "object",
                                                        "properties": {
                                                            "key": {
                                                                "type": "string"
                                                            },
                                                            "value": {
                                                                "type": "object",
                                                                "description": "OTLP typed AnyValue"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
    let example = json!({
        "resourceSpans": [
            {
                "resource": {
                    "attributes": [
                        {
                            "key": "service.name",
                            "value": {
                                "stringValue": "openlit"
                            }
                        }
                    ]
                },
                "scopeSpans": [
                    {
                        "scope": {
                            "name": "openlit"
                        },
                        "spans": [
                            {
                                "traceId": "112233445566778899aabbccddeeff00",
                                "spanId": "1122334455667788",
                                "name": "observed generation",
                                "startTimeUnixNano": "1791350400123456789",
                                "endTimeUnixNano": "1791350400123456790",
                                "attributes": [
                                    {
                                        "key": "gen_ai.output.messages",
                                        "value": {
                                            "stringValue": "[{\"role\":\"assistant\",\"content\":\"Observed answer\"}]"
                                        }
                                    }
                                ]
                            }
                        ]
                    }
                ]
            }
        ]
    });
    json!({
        "required": true,
        "description": text(docs, "标准 OTLP ExportTraceServiceRequest；保留资源、scope、span、typed AnyValue 与原始正文。可使用 gzip 请求体。", "Standard OTLP ExportTraceServiceRequest; retains resource, scope, spans, typed AnyValue and original bodies. Request compression may be gzip."),
        "content": {
            "application/json": {"schema":schema,"example":example},
            "application/x-protobuf":{"schema":{"type":"string","format":"binary"},"description":"Protobuf-encoded ExportTraceServiceRequest"}
        }
    })
}

fn otlp_responses(docs: &DocTextResolver) -> Value {
    let rpc = json!({"application/json":{"schema":{"type":"object","properties":{"code":{"type":"integer"},"message":{"type":"string"},"details":{"type":"array","items":{"type":"object"}}}}},"application/x-protobuf":{"schema":{"type":"string","format":"binary"}}});
    json!({
        "200":{"description":text(docs,"标准 OTLP ACK；部分接收会设置 partialSuccess。","Standard OTLP acknowledgement; partial acceptance sets partialSuccess."),"content":{"application/json":{"schema":{"type":"object","properties":{"partialSuccess":{"type":"object","properties":{"rejectedSpans":{"type":"string","pattern":"^[0-9]+$"},"errorMessage":{"type":"string"}}}}},"example":{}},"application/x-protobuf":{"schema":{"type":"string","format":"binary"}}}},
        "400":{"description":text(docs,"OTLP 数据或 gzip 正文无效；google.rpc.Status。","Invalid OTLP data or gzip body; google.rpc.Status."),"content":rpc},
        "401":{"description":text(docs,"缺少或无效的应用 API 密钥；google.rpc.Status。","Missing or invalid application API key; google.rpc.Status."),"content":rpc},
        "403":{"description":text(docs,"日志接收停用或应用类型不支持；google.rpc.Status。","Reception disabled or application type unsupported; google.rpc.Status."),"content":rpc},
        "409":{"description":text(docs,"不可变源事件与已有事实冲突；google.rpc.Status。","Immutable source event conflicts with a persisted fact; google.rpc.Status."),"content":rpc},
        "415":{"description":text(docs,"请求 Content-Type 或 Content-Encoding 不支持；google.rpc.Status。","Unsupported request Content-Type or Content-Encoding; google.rpc.Status."),"content":rpc}
    })
}
