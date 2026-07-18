use super::*;

mod request_schemas;

use request_schemas::{
    anthropic_count_tokens_schema, anthropic_message_schema, native_create_run_schema,
    native_file_upload_schema, native_resume_run_schema, openai_chat_completion_schema,
    openai_response_create_schema,
};

pub(super) fn operation_request_body(
    operation: &PublicOperation,
    docs: &DocTextResolver,
) -> Option<Value> {
    operation.request_body.map(|builder| builder(docs))
}

pub(super) fn operation_responses(operation: &PublicOperation, docs: &DocTextResolver) -> Value {
    (operation.responses)(docs)
}

pub(super) fn native_create_run_request_body(docs: &DocTextResolver) -> Value {
    json_request_body(
        native_create_run_schema(docs),
        json!({
            "query": "Summarize the incident",
            "expand_id": "external-user-1",
            "title": "Customer incident summary",
            "response_mode": "blocking",
            "inputs": {"priority": "high"},
            "execution": {
                "model_parameters": {
                    "max_output_tokens": 4096,
                    "reasoning": {
                        "enabled": true,
                        "effort": "high"
                    }
                }
            },
            "conversation": {"user": "external-user-1"},
            "attachments": [{"source": "upload_file_id", "value": "00000000-0000-0000-0000-000000000000"}]
        }),
    )
}

pub(super) fn native_resume_run_request_body(docs: &DocTextResolver) -> Value {
    json_request_body(
        native_resume_run_schema(docs),
        json!({
            "callback_task_id": "00000000-0000-0000-0000-000000000000",
            "response_payload": {},
            "response_mode": "blocking"
        }),
    )
}

pub(super) fn native_upload_file_request_body(docs: &DocTextResolver) -> Value {
    json!({
        "required": true,
        "content": {
            "multipart/form-data": {
                "schema": native_file_upload_schema(docs)
            }
        }
    })
}

pub(super) fn openai_chat_completion_request_body(docs: &DocTextResolver) -> Value {
    json_request_body(
        openai_chat_completion_schema(docs),
        json!({
            "model": "provider/model",
            "messages": [{"role": "user", "content": "Hello"}],
            "stream": false
        }),
    )
}

pub(super) fn openai_response_create_request_body(docs: &DocTextResolver) -> Value {
    json_request_body(
        openai_response_create_schema(docs),
        json!({
            "model": "provider/model",
            "input": "Hello",
            "previous_response_id": "resp_00000000-0000-0000-0000-000000000000",
            "stream": false
        }),
    )
}

pub(super) fn anthropic_message_request_body(docs: &DocTextResolver) -> Value {
    json_request_body(
        anthropic_message_schema(docs),
        json!({
            "model": "provider/model",
            "max_tokens": 512,
            "messages": [{"role": "user", "content": "Hello"}],
            "metadata": {"expand_id": "external-user-1"},
            "stream": false
        }),
    )
}

pub(super) fn anthropic_count_message_tokens_request_body(_docs: &DocTextResolver) -> Value {
    json_request_body(
        anthropic_count_tokens_schema(),
        json!({
            "model": "provider/model",
            "messages": [{"role": "user", "content": "Hello"}],
            "tools": [{
                "name": "lookup_order",
                "description": "Find an order",
                "input_schema": {"type": "object"}
            }]
        }),
    )
}

pub(super) fn native_create_run_responses(docs: &DocTextResolver) -> Value {
    native_responses(docs, "201", true)
}

pub(super) fn native_get_run_responses(docs: &DocTextResolver) -> Value {
    native_responses(docs, "200", false)
}

pub(super) fn native_resume_run_responses(docs: &DocTextResolver) -> Value {
    native_responses(docs, "200", true)
}

pub(super) fn native_model_list_responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": json_response(
            docs.response_description("native_model_list"),
            native_model_list_response_schema()
        ),
        "401": json_response(docs.response_description("invalid_application_api_key"), native_error_body_schema()),
        "403": json_response(docs.response_description("forbidden"), native_error_body_schema()),
        "409": json_response(
            docs.response_description("application_not_published_or_run_state_not_supported"),
            native_error_body_schema()
        )
    })
}

fn native_responses(
    docs: &DocTextResolver,
    success_status: &'static str,
    supports_streaming: bool,
) -> Value {
    let mut responses = serde_json::Map::new();
    let success_schema = api_success_schema(native_run_response_schema());
    let success_response = if supports_streaming {
        json_and_event_stream_response(
            docs.response_description(if success_status == "201" {
                "native_run_created"
            } else {
                "native_run"
            }),
            success_schema,
            native_streaming_event_schema(),
        )
    } else {
        json_response(
            docs.response_description(if success_status == "201" {
                "native_run_created"
            } else {
                "native_run"
            }),
            success_schema,
        )
    };
    responses.insert(success_status.to_string(), success_response);
    responses.insert(
        "400".to_string(),
        json_response(
            docs.response_description("invalid_request"),
            native_error_body_schema(),
        ),
    );
    responses.insert(
        "401".to_string(),
        json_response(
            docs.response_description("invalid_application_api_key"),
            native_error_body_schema(),
        ),
    );
    responses.insert(
        "403".to_string(),
        json_response(
            docs.response_description("forbidden"),
            native_error_body_schema(),
        ),
    );
    responses.insert(
        "404".to_string(),
        json_response(
            docs.response_description("not_found"),
            native_error_body_schema(),
        ),
    );
    responses.insert(
        "409".to_string(),
        json_response(
            docs.response_description("application_not_published_or_run_state_not_supported"),
            native_error_body_schema(),
        ),
    );
    Value::Object(responses)
}

pub(super) fn native_upload_responses(docs: &DocTextResolver) -> Value {
    json!({
        "201": json_response(
            docs.response_description("file_uploaded"),
            api_success_schema(uploaded_file_response_schema())
        ),
        "400": json_response(docs.response_description("invalid_request"), native_error_body_schema()),
        "401": json_response(docs.response_description("invalid_application_api_key"), native_error_body_schema())
    })
}

pub(super) fn openai_responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": json_and_event_stream_response(
            docs.response_description("compatible_response"),
            openai_chat_completion_response_schema(),
            openai_streaming_event_schema()
        ),
        "400": json_response(docs.response_description("invalid_request"), openai_error_body_schema()),
        "401": json_response(docs.response_description("invalid_application_api_key"), openai_error_body_schema()),
        "403": json_response(docs.response_description("forbidden"), openai_error_body_schema()),
        "409": json_response(
            docs.response_description("application_not_published_or_run_state_not_supported"),
            openai_error_body_schema()
        )
    })
}

pub(super) fn openai_model_list_responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": json_response(
            docs.response_description("compatible_model_list"),
            openai_model_list_response_schema()
        ),
        "401": json_response(docs.response_description("invalid_application_api_key"), openai_error_body_schema()),
        "403": json_response(docs.response_description("forbidden"), openai_error_body_schema()),
        "409": json_response(
            docs.response_description("application_not_published_or_run_state_not_supported"),
            openai_error_body_schema()
        )
    })
}

pub(super) fn openai_response_responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": json_and_event_stream_response(
            docs.response_description("compatible_response"),
            openai_response_response_schema(),
            openai_response_streaming_event_schema()
        ),
        "400": json_response(docs.response_description("invalid_request"), openai_error_body_schema()),
        "401": json_response(docs.response_description("invalid_application_api_key"), openai_error_body_schema()),
        "403": json_response(docs.response_description("forbidden"), openai_error_body_schema()),
        "409": json_response(
            docs.response_description("application_not_published_or_run_state_not_supported"),
            openai_error_body_schema()
        )
    })
}

pub(super) fn anthropic_responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": json_and_event_stream_response(
            docs.response_description("compatible_response"),
            anthropic_message_response_schema(),
            anthropic_streaming_event_schema()
        ),
        "400": json_response(docs.response_description("invalid_request"), anthropic_error_body_schema()),
        "401": json_response(docs.response_description("invalid_application_api_key"), anthropic_error_body_schema()),
        "403": json_response(docs.response_description("forbidden"), anthropic_error_body_schema()),
        "409": json_response(
            docs.response_description("application_not_published_or_run_state_not_supported"),
            anthropic_error_body_schema()
        )
    })
}

pub(super) fn anthropic_count_tokens_responses(docs: &DocTextResolver) -> Value {
    json!({
        "200": json_response(
            docs.response_description("compatible_token_count"),
            anthropic_count_tokens_response_schema()
        ),
        "400": json_response(docs.response_description("invalid_request"), anthropic_error_body_schema()),
        "401": json_response(docs.response_description("invalid_application_api_key"), anthropic_error_body_schema())
    })
}

fn json_response(description: &'static str, schema: Value) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": schema
            }
        }
    })
}

fn json_and_event_stream_response(
    description: &'static str,
    json_schema: Value,
    event_stream_schema: Value,
) -> Value {
    json!({
        "description": description,
        "content": {
            "application/json": {
                "schema": json_schema
            },
            "text/event-stream": {
                "schema": event_stream_schema
            }
        }
    })
}

fn json_request_body(schema: Value, example: Value) -> Value {
    json!({
        "required": true,
        "content": {
            "application/json": {
                "schema": schema,
                "example": example
            }
        }
    })
}

fn api_success_schema(data_schema: Value) -> Value {
    json!({
        "type": "object",
        "required": ["data"],
        "properties": {
            "data": data_schema,
            "meta": {
                "oneOf": [
                    {"type": "object", "additionalProperties": true},
                    {"type": "null"}
                ]
            }
        }
    })
}

fn native_run_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "application_id", "api_key_id", "publication_version_id", "status", "node_input_payload", "metadata", "created_at"],
        "properties": {
            "id": {"type": "string", "format": "uuid"},
            "application_id": {"type": "string", "format": "uuid"},
            "api_key_id": {"type": "string", "format": "uuid"},
            "publication_version_id": {"type": "string", "format": "uuid"},
            "status": {"type": "string"},
            "node_input_payload": {"type": "object", "additionalProperties": true},
            "metadata": {"type": "object", "additionalProperties": true},
            "answer": {"oneOf": [{"type": "string"}, {"type": "null"}]},
            "required_action": {"oneOf": [{"type": "object", "additionalProperties": true}, {"type": "null"}]},
            "tool_calls": {"oneOf": [{"type": "array", "items": tool_call_schema()}, {"type": "null"}]},
            "usage": {"oneOf": [{"type": "object", "additionalProperties": true}, {"type": "null"}]},
            "error": {"oneOf": [{"type": "object", "additionalProperties": true}, {"type": "null"}]},
            "created_at": {"type": "string", "format": "date-time"}
        }
    })
}

fn native_model_list_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["object", "data"],
        "properties": {
            "object": {"type": "string", "enum": ["list"]},
            "data": {
                "type": "array",
                "items": native_model_object_schema()
            }
        }
    })
}

fn native_model_object_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "capabilities"],
        "properties": {
            "id": {"type": "string"},
            "name": {"type": "string"},
            "context_window": {"type": "integer"},
            "max_context_window": {"type": "integer"},
            "max_output_tokens": {"type": "integer"},
            "auto_compact_token_limit": {"type": "integer"},
            "capabilities": model_capabilities_schema(),
            "reasoning": model_reasoning_schema()
        }
    })
}

fn model_capabilities_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "reasoning": {"type": "boolean"},
            "tool_call": {"type": "boolean"},
            "multimodal": {"type": "boolean"},
            "structured_output": {"type": "boolean"}
        }
    })
}

fn model_reasoning_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "default_effort": {"type": "string"},
            "supported_efforts": {
                "type": "array",
                "items": {"type": "string"}
            }
        }
    })
}

fn native_error_body_schema() -> Value {
    json!({
        "type": "object",
        "required": ["code", "message"],
        "properties": {
            "code": {"type": "string"},
            "message": {"type": "string"}
        }
    })
}

fn uploaded_file_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["storage_id", "record"],
        "properties": {
            "storage_id": {"type": "string"},
            "record": {"type": "object", "additionalProperties": true}
        }
    })
}

fn tool_call_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": {"type": "string"},
            "type": {"type": "string"},
            "name": {"type": "string"},
            "arguments": {
                "oneOf": [
                    {"type": "object", "additionalProperties": true},
                    {"type": "array"},
                    {"type": "string"},
                    {"type": "number"},
                    {"type": "boolean"},
                    {"type": "null"}
                ]
            }
        },
        "additionalProperties": true
    })
}

fn openai_tool_call_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "type", "function"],
        "properties": {
            "id": {"type": "string"},
            "type": {"type": "string", "enum": ["function"]},
            "function": {
                "type": "object",
                "required": ["name", "arguments"],
                "properties": {
                    "name": {"type": "string"},
                    "arguments": {"type": "string"}
                }
            }
        }
    })
}

fn openai_tool_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "function"],
        "properties": {
            "type": {"type": "string", "enum": ["function"]},
            "function": {
                "type": "object",
                "required": ["name"],
                "properties": {
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "parameters": {"type": "object", "additionalProperties": true}
                }
            }
        },
        "additionalProperties": true
    })
}

fn anthropic_tool_schema() -> Value {
    json!({
        "type": "object",
        "required": ["name"],
        "properties": {
            "name": {"type": "string"},
            "description": {"type": "string"},
            "input_schema": {"type": "object", "additionalProperties": true}
        },
        "additionalProperties": true
    })
}

fn openai_chat_completion_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "object", "created", "model", "choices", "usage"],
        "properties": {
            "id": {"type": "string"},
            "object": {"type": "string", "enum": ["chat.completion"]},
            "created": {"type": "integer"},
            "model": {"type": "string"},
            "choices": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["index", "message", "finish_reason"],
                    "properties": {
                        "index": {"type": "integer"},
                        "message": {
                            "type": "object",
                            "required": ["role", "content"],
                            "properties": {
                                "role": {"type": "string", "enum": ["assistant"]},
                                "content": {"oneOf": [{"type": "string"}, {"type": "null"}]},
                                "tool_calls": {
                                    "type": "array",
                                    "items": openai_tool_call_schema()
                                }
                            }
                        },
                        "finish_reason": {"type": "string", "enum": ["stop", "tool_calls"]}
                    }
                }
            },
            "usage": {
                "type": "object",
                "properties": {
                    "prompt_tokens": {"type": "integer"},
                    "completion_tokens": {"type": "integer"},
                    "total_tokens": {"type": "integer"}
                }
            }
        }
    })
}

fn openai_model_list_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["object", "data"],
        "properties": {
            "object": {"type": "string", "enum": ["list"]},
            "data": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "object", "created", "owned_by"],
                    "properties": {
                        "id": {"type": "string"},
                        "object": {"type": "string", "enum": ["model"]},
                        "created": {"type": "integer"},
                        "owned_by": {"type": "string"},
                        "context_window": {"type": "integer"},
                        "max_context_window": {"type": "integer"},
                        "max_output_tokens": {"type": "integer"},
                        "auto_compact_token_limit": {"type": "integer"},
                        "capabilities": model_capabilities_schema(),
                        "reasoning": model_reasoning_schema(),
                        "limit": {
                            "type": "object",
                            "properties": {
                                "context": {"type": "integer"},
                                "input": {"type": "integer"},
                                "output": {"type": "integer"}
                            }
                        },
                        "name": {
                            "oneOf": [
                                {"type": "string"},
                                {"type": "null"}
                            ]
                        }
                    }
                }
            }
        }
    })
}

fn openai_response_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "object", "created_at", "status", "model", "output", "output_text", "usage"],
        "properties": {
            "id": {"type": "string"},
            "object": {"type": "string", "enum": ["response"]},
            "created_at": {"type": "integer"},
            "status": {"type": "string", "enum": ["completed"]},
            "model": {"type": "string"},
            "previous_response_id": {
                "oneOf": [
                    {"type": "string"},
                    {"type": "null"}
                ]
            },
            "output": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["id", "type", "status", "role", "content"],
                    "properties": {
                        "id": {"type": "string"},
                        "type": {"type": "string", "enum": ["message"]},
                        "status": {"type": "string", "enum": ["completed"]},
                        "role": {"type": "string", "enum": ["assistant"]},
                        "content": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["type", "text", "annotations"],
                                "properties": {
                                    "type": {"type": "string", "enum": ["output_text"]},
                                    "text": {"type": "string"},
                                    "annotations": {"type": "array", "items": {"type": "object", "additionalProperties": true}}
                                }
                            }
                        }
                    }
                }
            },
            "output_text": {"type": "string"},
            "usage": {
                "type": "object",
                "properties": {
                    "input_tokens": {"type": "integer"},
                    "output_tokens": {"type": "integer"},
                    "total_tokens": {"type": "integer"}
                }
            }
        }
    })
}

fn openai_error_body_schema() -> Value {
    json!({
        "type": "object",
        "required": ["error"],
        "properties": {
            "error": {
                "type": "object",
                "required": ["message", "type", "code"],
                "properties": {
                    "message": {"type": "string"},
                    "type": {"type": "string"},
                    "param": {"oneOf": [{"type": "string"}, {"type": "null"}]},
                    "code": {"type": "string"}
                }
            }
        }
    })
}

fn anthropic_message_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["id", "type", "role", "model", "content", "stop_reason", "usage"],
        "properties": {
            "id": {"type": "string"},
            "type": {"type": "string", "enum": ["message"]},
            "role": {"type": "string", "enum": ["assistant"]},
            "model": {"type": "string"},
            "content": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["type"],
                    "properties": {
                        "type": {"type": "string", "enum": ["text", "tool_use"]},
                        "text": {"type": "string"},
                        "id": {"type": "string"},
                        "name": {"type": "string"},
                        "input": {"type": "object", "additionalProperties": true}
                    }
                }
            },
            "stop_reason": {"type": "string", "enum": ["end_turn", "tool_use"]},
            "usage": {
                "type": "object",
                "properties": {
                    "input_tokens": {"type": "integer"},
                    "output_tokens": {"type": "integer"}
                }
            }
        }
    })
}

fn anthropic_error_body_schema() -> Value {
    json!({
        "type": "object",
        "required": ["type", "error"],
        "properties": {
            "type": {"type": "string", "enum": ["error"]},
            "error": {
                "type": "object",
                "required": ["type", "message"],
                "properties": {
                    "type": {"type": "string"},
                    "message": {"type": "string"}
                }
            }
        }
    })
}

fn anthropic_count_tokens_response_schema() -> Value {
    json!({
        "type": "object",
        "required": ["input_tokens"],
        "properties": {
            "input_tokens": {"type": "integer", "minimum": 1}
        }
    })
}

fn native_streaming_event_schema() -> Value {
    json!({
        "type": "string",
        "format": "event-stream",
        "description": "Server-Sent Events emitted when response_mode=streaming.",
        "x-1flowbase-heartbeat": true,
        "x-1flowbase-events": [
            "run.started",
            "reasoning.delta",
            "message.delta",
            "workflow.event",
            "required_action",
            "run.completed",
            "run.failed",
            "run.cancelled"
        ],
        "x-1flowbase-reasoning-delta": "event: reasoning.delta",
        "x-1flowbase-message-delta": "event: message.delta"
    })
}

fn openai_streaming_event_schema() -> Value {
    json!({
        "type": "string",
        "format": "event-stream",
        "description": "OpenAI-compatible chat completion chunks emitted when stream=true.",
        "x-1flowbase-heartbeat": {
            "interval_seconds": 10,
            "text": "heartbeat"
        },
        "x-1flowbase-reasoning-delta": "choices[0].delta.reasoning_content",
        "x-1flowbase-message-delta": "choices[0].delta.content",
        "x-1flowbase-terminal-data": "[DONE]"
    })
}

fn openai_response_streaming_event_schema() -> Value {
    json!({
        "type": "string",
        "format": "event-stream",
        "description": "OpenAI Responses-compatible events emitted when stream=true.",
        "x-1flowbase-heartbeat": {
            "interval_seconds": 10,
            "text": "heartbeat"
        },
        "x-1flowbase-created": "response.created",
        "x-1flowbase-message-delta": "response.output_text.delta",
        "x-1flowbase-reasoning-delta": "response.reasoning_text.delta",
        "x-1flowbase-terminal-events": [
            "response.completed",
            "response.failed"
        ]
    })
}

fn anthropic_streaming_event_schema() -> Value {
    json!({
        "type": "string",
        "format": "event-stream",
        "description": "Anthropic-compatible message stream events emitted when stream=true.",
        "x-1flowbase-heartbeat": {
            "interval_seconds": 10,
            "text": "heartbeat"
        },
        "x-1flowbase-reasoning-delta": {
            "type": "content_block_delta",
            "delta": {
                "type": "thinking_delta",
                "field": "thinking"
            }
        },
        "x-1flowbase-message-delta": {
            "type": "content_block_delta",
            "delta": {
                "type": "text_delta",
                "field": "text"
            }
        }
    })
}
