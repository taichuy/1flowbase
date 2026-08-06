use super::*;

pub(super) fn native_create_run_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["query"],
        "properties": {
            "query": {
                "type": "string",
                "description": docs.field_description("application_public_api.native.create_run.request.query")
            },
            "model": {
                "type": "string",
                "description": docs.field_description("application_public_api.native.create_run.request.model")
            },
            "system": {
                "type": "string",
                "description": docs.field_description("application_public_api.native.create_run.request.system")
            },
            "inputs": {
                "type": "object",
                "additionalProperties": true,
                "description": docs.field_description("application_public_api.native.create_run.request.inputs")
            },
            "history": {
                "type": "array",
                "items": {
                    "type": "object",
                    "required": ["role", "content"],
                    "properties": {
                        "role": {"type": "string", "enum": ["system", "user", "assistant"]},
                        "content": {"type": "string"}
                    },
                    "additionalProperties": false
                },
                "description": docs.field_description("application_public_api.native.create_run.request.history")
            },
            "attachments": {
                "type": "array",
                "items": native_attachment_schema(docs),
                "description": docs.field_description("application_public_api.native.create_run.request.attachments")
            },
            "conversation": {
                "type": "object",
                "additionalProperties": true,
                "description": docs.field_description("application_public_api.native.create_run.request.conversation")
            },
            "expand_id": {
                "type": "string",
                "description": docs.field_description("application_public_api.native.create_run.request.expand_id")
            },
            "title": {
                "type": "string",
                "maxLength": 255,
                "description": docs.field_description("application_public_api.native.create_run.request.title")
            },
            "response_mode": {
                "type": "string",
                "enum": ["blocking", "streaming"],
                "default": "blocking"
            },
            "stream_options": {
                "type": "object",
                "additionalProperties": false,
                "description": docs.field_description("application_public_api.native.create_run.request.stream_options"),
                "properties": {
                    "include_workflow_events": {
                        "type": "string",
                        "enum": ["none", "public"],
                        "default": "none",
                        "description": "public returns only the node id, type, title, and status summary. debug is reserved for an authorized browser session principal."
                    }
                }
            },
            "execution": {
                "type": "object",
                "additionalProperties": true,
                "description": docs.field_description("application_public_api.native.create_run.request.execution"),
                "properties": {
                    "idempotency_key": {"type": "string"},
                    "model_parameters": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "max_output_tokens": {
                                "type": "integer",
                                "minimum": 1
                            },
                            "reasoning": {
                                "type": "object",
                                "additionalProperties": false,
                                "properties": {
                                    "enabled": {"type": "boolean"},
                                    "effort": {"type": "string", "enum": ["minimal", "low", "medium", "high", "xhigh"]},
                                    "budget_tokens": {"type": "integer", "minimum": 1}
                                }
                            }
                        }
                    }
                }
            },
            "metadata": {
                "type": "object",
                "properties": {
                    "trace_id": {"type": "string"}
                },
                "additionalProperties": false,
                "description": docs.field_description("application_public_api.native.create_run.request.metadata")
            }
        }
    })
}

fn native_attachment_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["source", "value"],
        "properties": {
            "source": {
                "type": "string",
                "enum": ["upload_file_id", "url", "base64"]
            },
            "value": {
                "type": "string",
                "description": docs.field_description("application_public_api.native.create_run.request.attachments.value")
            },
            "name": {"type": "string"},
            "mime_type": {"type": "string"},
            "metadata": {"type": "object", "additionalProperties": true}
        },
        "additionalProperties": false
    })
}

pub(super) fn native_resume_run_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["callback_task_id"],
        "properties": {
            "callback_task_id": {
                "type": "string",
                "format": "uuid",
                "description": docs.field_description("application_public_api.native.resume_run.request.callback_task_id")
            },
            "response_payload": {
                "type": "object",
                "additionalProperties": true,
                "default": {}
            },
            "response_mode": {
                "type": "string",
                "enum": ["blocking", "streaming"],
                "default": "blocking"
            }
        }
    })
}

pub(super) fn native_file_upload_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["file_table_id", "file"],
        "properties": {
            "file_table_id": {
                "type": "string",
                "format": "uuid",
                "description": docs.field_description("application_public_api.native.upload_file.request.file_table_id")
            },
            "file": {
                "type": "string",
                "format": "binary",
                "description": docs.field_description("application_public_api.native.upload_file.request.file")
            }
        }
    })
}

pub(super) fn openai_chat_completion_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["model", "messages"],
        "properties": {
            "model": {"type": "string"},
            "messages": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["role", "content"],
                    "properties": {
                        "role": {"type": "string", "enum": ["system", "user", "assistant", "tool"]},
                        "content": {
                            "oneOf": [
                                {"type": "string"},
                                {"type": "null"},
                                {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "type": {"type": "string", "enum": ["text"]},
                                            "text": {"type": "string"}
                                        }
                                    }
                                }
                            ]
                        },
                        "name": {"type": "string"},
                        "tool_call_id": {"type": "string"},
                        "tool_calls": {
                            "type": "array",
                            "items": openai_tool_call_schema()
                        }
                    }
                }
            },
            "stream": {
                "type": "boolean",
                "description": docs.field_description("application_public_api.openai.chat_completion.request.stream")
            },
            "user": {"type": "string"},
            "tools": {
                "type": "array",
                "items": openai_tool_schema()
            },
            "tool_choice": {
                "oneOf": [
                    {"type": "string", "enum": ["none", "auto", "required"]},
                    {"type": "object", "additionalProperties": true}
                ]
            },
            "function_call": {
                "oneOf": [
                    {"type": "string", "enum": ["none", "auto"]},
                    {"type": "object", "additionalProperties": true}
                ]
            },
            "metadata": {
                "type": "object",
                "properties": {"trace_id": {"type": "string"}},
                "additionalProperties": false
            }
        }
    })
}

pub(super) fn openai_response_create_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["model", "input"],
        "properties": {
            "model": {"type": "string"},
            "input": {
                "oneOf": [
                    {"type": "string"},
                    {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "role": {"type": "string", "enum": ["system", "user", "assistant"]},
                                "content": {
                                    "oneOf": [
                                        {"type": "string"},
                                        {
                                            "type": "array",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "type": {"type": "string", "enum": ["input_text", "text"]},
                                                    "text": {"type": "string"}
                                                }
                                            }
                                        }
                                    ]
                                }
                            },
                            "additionalProperties": true
                        }
                    }
                ]
            },
            "instructions": {"type": "string"},
            "previous_response_id": {"type": "string"},
            "stream": {
                "type": "boolean",
                "description": docs.field_description("application_public_api.openai.response.request.stream")
            },
            "user": {"type": "string"},
            "tools": {
                "type": "array",
                "items": openai_tool_schema()
            },
            "tool_choice": {
                "oneOf": [
                    {"type": "string", "enum": ["none", "auto", "required"]},
                    {"type": "object", "additionalProperties": true}
                ]
            },
            "metadata": {
                "type": "object",
                "properties": {"trace_id": {"type": "string"}},
                "additionalProperties": false
            }
        }
    })
}

pub(super) fn anthropic_message_schema(docs: &DocTextResolver) -> Value {
    json!({
        "type": "object",
        "required": ["model", "messages"],
        "properties": {
            "model": {"type": "string"},
            "max_tokens": {"type": "integer", "minimum": 1},
            "system": {"type": "string"},
            "messages": {
                "type": "array",
                "minItems": 1,
                "items": {
                    "type": "object",
                    "required": ["role", "content"],
                    "properties": {
                        "role": {"type": "string", "enum": ["user", "assistant"]},
                        "content": {
                            "oneOf": [
                                {"type": "string"},
                                {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "type": {"type": "string", "enum": ["text", "tool_use", "tool_result"]},
                                            "text": {"type": "string"},
                                            "id": {"type": "string"},
                                            "name": {"type": "string"},
                                            "input": {"type": "object", "additionalProperties": true},
                                            "tool_use_id": {"type": "string"},
                                            "is_error": {"type": "boolean"},
                                            "content": {
                                                "oneOf": [
                                                    {"type": "string"},
                                                    {
                                                        "type": "array",
                                                        "items": {"type": "object", "additionalProperties": true}
                                                    },
                                                    {"type": "object", "additionalProperties": true}
                                                ]
                                            }
                                        }
                                    }
                                }
                            ]
                        }
                    }
                }
            },
            "stream": {
                "type": "boolean",
                "description": docs.field_description("application_public_api.anthropic.message.request.stream")
            },
            "tools": {
                "type": "array",
                "items": anthropic_tool_schema()
            },
            "tool_choice": {"type": "object", "additionalProperties": true},
            "metadata": {
                "type": "object",
                "properties": {
                    "user_id": {"type": "string"},
                    "expand_id": {"type": "string"},
                    "session_id": {"type": "string"},
                    "trace_id": {"type": "string"}
                },
                "additionalProperties": false,
                "description": docs.field_description("application_public_api.anthropic.message.request.metadata")
            }
        }
    })
}

pub(super) fn anthropic_count_tokens_schema() -> Value {
    let content_block_schema = anthropic_count_tokens_content_block_schema();
    let message_content_schema = json!({
        "oneOf": [
            {"type": "string"},
            {
                "type": "array",
                "items": content_block_schema
            }
        ]
    });
    let message_schema = json!({
        "type": "object",
        "required": ["role", "content"],
        "properties": {
            "role": {"type": "string", "enum": ["user", "assistant"]},
            "content": message_content_schema
        },
        "additionalProperties": true
    });
    json!({
        "type": "object",
        "required": ["model", "messages"],
        "properties": {
            "model": {"type": "string"},
            "system": {
                "oneOf": [
                    {"type": "string"},
                    {
                        "type": "array",
                        "items": {"type": "object", "additionalProperties": true}
                    }
                ]
            },
            "messages": {
                "type": "array",
                "minItems": 1,
                "items": message_schema
            },
            "tools": {
                "type": "array",
                "items": anthropic_tool_schema()
            },
            "tool_choice": {
                "oneOf": [
                    {"type": "string", "enum": ["auto", "any", "none"]},
                    {"type": "object", "additionalProperties": true}
                ]
            },
            "thinking": {"type": "object", "additionalProperties": true},
            "container": {"type": "object", "additionalProperties": true},
            "context_management": {"type": "object", "additionalProperties": true},
            "metadata": {
                "type": "object",
                "properties": {
                    "user_id": {"type": "string"},
                    "expand_id": {"type": "string"},
                    "session_id": {"type": "string"},
                    "trace_id": {"type": "string"}
                },
                "additionalProperties": false
            }
        },
        "additionalProperties": true
    })
}

fn anthropic_count_tokens_content_block_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": {
                "type": "string",
                "enum": [
                    "text",
                    "tool_use",
                    "tool_result",
                    "thinking",
                    "redacted_thinking",
                    "image",
                    "document"
                ]
            },
            "text": {"type": "string"},
            "id": {"type": "string"},
            "name": {"type": "string"},
            "input": {"type": "object", "additionalProperties": true},
            "tool_use_id": {"type": "string"},
            "is_error": {"type": "boolean"},
            "thinking": {"type": "string"},
            "signature": {"type": "string"},
            "source": {"type": "object", "additionalProperties": true},
            "content": {
                "oneOf": [
                    {"type": "string"},
                    {
                        "type": "array",
                        "items": {"type": "object", "additionalProperties": true}
                    },
                    {"type": "object", "additionalProperties": true}
                ]
            }
        },
        "additionalProperties": true
    })
}
