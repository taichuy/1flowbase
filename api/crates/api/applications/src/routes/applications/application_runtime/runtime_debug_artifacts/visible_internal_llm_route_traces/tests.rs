use super::*;

#[test]
fn route_trace_summary_proves_return_to_main_without_large_payloads() {
    let route_output = "route image description ".repeat(48);
    let debug_payload = json!({
        "llm_rounds": [
            {
                "round_index": 0,
                "assistant": {
                    "role": "assistant",
                    "content": "need image",
                    "tool_calls": [
                        {
                            "id": "call_image",
                            "name": "image_llm",
                            "arguments": {
                                "file": "uploads/agent-flow-node-detail-icon-aligned.png"
                            }
                        }
                    ]
                }
            },
            {
                "round_index": 1,
                "tool_results": [
                    {
                        "role": "tool",
                        "tool_call_id": "call_image",
                        "name": "image_llm",
                        "content": route_output
                    }
                ]
            },
            {
                "round_index": 2,
                "assistant": {
                    "role": "assistant",
                    "content": "main model saw the routed result and answered"
                },
                "usage": {
                    "total_tokens": 120
                }
            }
        ],
        "visible_internal_llm_tool_events": [
            {
                "event_type": "visible_internal_llm_tool_started",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "arguments": {
                    "file": "uploads/agent-flow-node-detail-icon-aligned.png"
                }
            },
            {
                "event_type": "visible_internal_llm_tool_waiting_callback",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "waiting_node_id": "node-llm-1",
                "waiting_node_alias": "Read image",
                "request_payload": {
                    "history": [
                        {
                            "role": "user",
                            "content_blocks": [
                                {
                                    "type": "image",
                                    "source": {
                                        "type": "base64",
                                        "media_type": "image/png",
                                        "data": "large-base64-data-should-stay-in-detail"
                                    }
                                }
                            ]
                        }
                    ]
                }
            },
            {
                "event_type": "visible_internal_llm_tool_completed",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "node_id": "node-llm-1",
                "provider_route": {
                    "model": "mimo-v2.5",
                    "provider_code": "anthropic"
                }
            }
        ]
    });

    let traces = collect_visible_internal_llm_tool_route_traces(&debug_payload);

    assert_eq!(traces.len(), 1);
    let summary = traces[0].summary_payload(Uuid::nil());
    assert_eq!(summary["kind"], json!(VISIBLE_INTERNAL_LLM_TOOL_TRACE_KIND));
    assert_eq!(summary["tool_call_id"], json!("call_image"));
    assert_eq!(summary["tool_name"], json!("image_llm"));
    assert_eq!(summary["route_model"], json!("mimo-v2.5"));
    assert_eq!(summary["returned_to_main"], json!(true));
    assert_eq!(summary["main_resume"], json!(true));
    assert_eq!(summary["callback_count"], json!(1));
    assert_eq!(
        summary["route_output_summary"]["char_count"],
        json!(route_output.chars().count() as i64)
    );
    assert_eq!(summary["route_output_summary"]["truncated"], json!(true));
    assert_eq!(summary["__runtime_debug_artifact"], json!(true));
    assert_eq!(summary["artifact_ref"], json!(Uuid::nil().to_string()));
    assert!(!summary.to_string().contains("request_payload"));
    assert!(!summary
        .to_string()
        .contains("large-base64-data-should-stay-in-detail"));
    assert!(!summary.to_string().contains(&route_output));

    let detail = traces[0].detail_payload();
    assert_eq!(detail["route"]["model"], json!("mimo-v2.5"));
    assert_eq!(detail["route_output"], json!(route_output));
    assert_eq!(
        detail["main_resume_output"]["content"],
        json!("main model saw the routed result and answered")
    );
    assert_eq!(
        detail["callback_requests"][0]["request_payload"]["history"][0]["content_blocks"][0]
            ["source"]["data"],
        json!("large-base64-data-should-stay-in-detail")
    );
}

#[test]
fn route_trace_ignores_plain_external_tool_rounds() {
    let debug_payload = json!({
        "llm_rounds": [
            {
                "round_index": 0,
                "assistant": {
                    "role": "assistant",
                    "tool_calls": [
                        {
                            "id": "call_external",
                            "name": "read_file"
                        }
                    ]
                }
            },
            {
                "round_index": 1,
                "tool_results": [
                    {
                        "role": "tool",
                        "tool_call_id": "call_external",
                        "name": "read_file",
                        "content": "plain tool result"
                    }
                ]
            }
        ],
        "visible_internal_llm_tool_events": []
    });

    assert!(collect_visible_internal_llm_tool_route_traces(&debug_payload).is_empty());
}

#[test]
fn route_trace_uses_completed_event_content_without_persisted_rounds() {
    let debug_payload = json!({
        "visible_internal_llm_tool_events": [
            {
                "event_type": "visible_internal_llm_tool_started",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "arguments": {
                    "media": [
                        {
                            "kind": "image",
                            "path": "uploads/test-01.png",
                            "source": "workspace_path"
                        }
                    ]
                }
            },
            {
                "event_type": "visible_internal_llm_tool_completed",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "node_id": "node-llm-1",
                "provider_route": {
                    "model": "mimo-v2.5",
                    "provider_code": "anthropic"
                },
                "content": "图片是 1flowbase 顶部导航栏。"
            }
        ]
    });

    let traces = collect_visible_internal_llm_tool_route_traces(&debug_payload);

    assert_eq!(traces.len(), 1);
    let summary = traces[0].inline_summary_payload();
    assert_eq!(summary["status"], json!("returned_to_main"));
    assert_eq!(summary["route_model"], json!("mimo-v2.5"));
    assert_eq!(
        summary["route_output_summary"]["preview"],
        json!("图片是 1flowbase 顶部导航栏。")
    );
}

#[test]
fn route_trace_marks_recoverable_media_precondition_failure_as_intercepted() {
    let debug_payload = json!({
        "llm_rounds": [
            {
                "round_index": 0,
                "assistant": {
                    "role": "assistant",
                    "tool_calls": [
                        {
                            "id": "call_image",
                            "name": "image_llm"
                        }
                    ]
                }
            },
            {
                "round_index": 1,
                "tool_results": [
                    {
                        "role": "tool",
                        "tool_call_id": "call_image",
                        "name": "image_llm",
                        "content": "read the file with a client file tool first"
                    }
                ]
            },
            {
                "round_index": 2,
                "assistant": {
                    "role": "assistant",
                    "content": "main model handled the media guidance"
                }
            }
        ],
        "visible_internal_llm_tool_events": [
            {
                "event_type": "visible_internal_llm_tool_started",
                "main_node_id": "node-llm",
                "target_node_id": "node-image-llm",
                "tool_name": "image_llm",
                "tool_call_id": "call_image"
            },
            {
                "event_type": "visible_internal_llm_tool_failed",
                "main_node_id": "node-llm",
                "target_node_id": "node-image-llm",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "error_payload": {
                    "error_code": "visible_internal_llm_tool_failed",
                    "message": "visible internal LLM tool branch node failed",
                    "details": {
                        "error_code": "visible_internal_llm_tool_media_unavailable",
                        "message": "visible internal LLM tool media was not available to the server",
                        "recoverable": true
                    }
                }
            }
        ]
    });

    let traces = collect_visible_internal_llm_tool_route_traces(&debug_payload);

    assert_eq!(traces.len(), 1);
    let summary = traces[0].inline_summary_payload();
    assert_eq!(summary["status"], json!("intercepted"));
    assert_eq!(summary["returned_to_main"], json!(true));
    assert_eq!(summary["main_resume"], json!(true));

    let detail = traces[0].detail_payload();
    assert_eq!(detail["status"], json!("intercepted"));
    assert_eq!(
        detail["tool_result"]["content"],
        json!("read the file with a client file tool first")
    );
}

#[test]
fn route_trace_projects_node_output_as_main_resume_for_current_run_sample() {
    let debug_payload = json!({
        "visible_internal_llm_tool_events": [
            {
                "event_type": "visible_internal_llm_tool_started",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image"
            },
            {
                "event_type": "visible_internal_llm_tool_completed",
                "main_node_id": "node-llm",
                "target_node_id": "node-llm-1",
                "tool_name": "image_llm",
                "tool_call_id": "call_image",
                "node_id": "node-llm-1",
                "provider_route": {
                    "model": "mimo-v2.5",
                    "provider_code": "anthropic"
                }
            }
        ]
    });
    let node_output = json!({
        "text": "很好，图片分析出来了！这是一个 1flowbase 的顶部导航栏。"
    });

    let traces = collect_visible_internal_llm_tool_route_traces_with_main_output(
        &debug_payload,
        Some(&node_output),
    );

    assert_eq!(traces.len(), 1);
    let summary = traces[0].inline_summary_payload();
    assert_eq!(summary["status"], json!("succeeded"));
    assert_eq!(summary["returned_to_main"], json!(true));
    assert_eq!(summary["main_resume"], json!(true));
    assert_eq!(
        summary["final_output_summary"]["preview"],
        json!("很好，图片分析出来了！这是一个 1flowbase 的顶部导航栏。")
    );
}

#[test]
fn fusion_trace_projects_panel_branch_summaries_and_fan_in_detail() {
    let debug_payload = json!({
        "llm_rounds": [
            {
                "round_index": 0,
                "assistant": {
                    "role": "assistant",
                    "tool_calls": [
                        {
                            "id": "call_fusion",
                            "name": "fusion_review",
                            "arguments": {
                                "topic": "refund policy"
                            }
                        }
                    ]
                }
            },
            {
                "round_index": 1,
                "tool_results": [
                    {
                        "role": "tool",
                        "tool_call_id": "call_fusion",
                        "name": "fusion_review",
                        "content": "panel A says strict\npanel B says flexible"
                    }
                ]
            },
            {
                "round_index": 2,
                "assistant": {
                    "role": "assistant",
                    "content": "main merged the fusion panel"
                }
            }
        ],
        "visible_internal_llm_tool_events": [
            {
                "event_type": "visible_internal_llm_tool_started",
                "main_node_id": "node-main-llm",
                "target_node_id": "node-panel-a",
                "tool_name": "fusion_review",
                "tool_call_id": "call_fusion",
                "tool_mode": "fusion",
                "execution_mode": "bounded_parallel_panel"
            },
            {
                "event_type": "visible_internal_llm_tool_completed",
                "main_node_id": "node-main-llm",
                "target_node_id": "node-panel-a",
                "tool_name": "fusion_review",
                "tool_call_id": "call_fusion",
                "tool_mode": "fusion",
                "execution_mode": "bounded_parallel_panel",
                "node_id": "node-panel-a",
                "node_alias": "Risk Panel",
                "node_type": "llm",
                "provider_route": {
                    "model": "risk-v1"
                },
                "input_payload": {
                    "user_prompt": "review refund policy risk",
                    "model": "risk-v1"
                },
                "content": "panel A says strict",
                "debug_payload": {
                    "llm_rounds": [
                        {
                            "round_index": 0,
                            "assistant": {
                                "content": "risk result"
                            }
                        }
                    ]
                }
            },
            {
                "event_type": "visible_internal_llm_tool_completed",
                "main_node_id": "node-main-llm",
                "target_node_id": "node-panel-a",
                "tool_name": "fusion_review",
                "tool_call_id": "call_fusion",
                "tool_mode": "fusion",
                "execution_mode": "bounded_parallel_panel",
                "node_id": "node-panel-b",
                "node_alias": "Support Panel",
                "node_type": "llm",
                "provider_route": {
                    "model": "support-v1"
                },
                "content": "panel B says flexible",
                "debug_payload_ref": "artifact-panel-b"
            }
        ]
    });

    let traces = collect_visible_internal_llm_tool_route_traces(&debug_payload);

    assert_eq!(traces.len(), 1);
    let summary = traces[0].inline_summary_payload();
    assert_eq!(summary["route_kind"], json!("fusion"));
    assert_eq!(summary["branch_count"], json!(2));
    assert_eq!(
        summary["branch_summaries"][0]["node_id"],
        json!("node-panel-a")
    );
    assert_eq!(
        summary["branch_summaries"][0]["node_alias"],
        json!("Risk Panel")
    );
    assert_eq!(
        summary["branch_summaries"][0]["route_model"],
        json!("risk-v1")
    );
    assert_eq!(
        summary["branch_summaries"][0]["output_summary"]["preview"],
        json!("panel A says strict")
    );
    assert!(!summary.to_string().contains("risk result"));

    let detail = traces[0].detail_payload();
    assert_eq!(detail["route_kind"], json!("fusion"));
    assert_eq!(detail["fan_in"]["mode"], json!("bounded_parallel_panel"));
    assert_eq!(detail["fan_in"]["branch_count"], json!(2));
    assert_eq!(detail["fan_in"]["returned_to_main"], json!(true));
    assert_eq!(detail["fan_in"]["main_resume"], json!(true));
    assert_eq!(
        detail["branch_traces"][0]["input_payload"]["user_prompt"],
        json!("review refund policy risk")
    );
    assert_eq!(
        detail["branch_traces"][0]["debug_payload"]["llm_rounds"][0]["assistant"]["content"],
        json!("risk result")
    );
    assert_eq!(
        detail["branch_traces"][0]["output_payload"]["text"],
        json!("panel A says strict")
    );
    assert_eq!(
        detail["branch_traces"][0]["output_payload"]["provider_route"]["model"],
        json!("risk-v1")
    );
    assert_eq!(
        detail["branch_traces"][1]["debug_payload_ref"],
        json!("artifact-panel-b")
    );
}

#[test]
fn fusion_trace_projects_historical_summary_llm_detail_from_debug_context() {
    let debug_payload = json!({
        "visible_internal_llm_tool_events": [
            {
                "event_type": "visible_internal_llm_tool_started",
                "main_node_id": "node-main-llm",
                "target_node_id": "node-panel-a",
                "tool_name": "fusion_review",
                "tool_call_id": "call_fusion",
                "tool_mode": "fusion",
                "execution_mode": "bounded_parallel_panel"
            },
            {
                "event_type": "visible_internal_llm_tool_completed",
                "main_node_id": "node-main-llm",
                "target_node_id": "node-panel-a",
                "tool_name": "fusion_review",
                "tool_call_id": "call_fusion",
                "tool_mode": "fusion",
                "execution_mode": "bounded_parallel_panel",
                "node_id": "node-judge",
                "node_alias": "LLM5",
                "node_type": "llm",
                "provider_route": {
                    "model": "gpt-5.4-mini",
                    "provider_code": "fixture_provider"
                },
                "metrics_payload": {
                    "usage": {
                        "input_tokens": 5513,
                        "output_tokens": 2455,
                        "total_tokens": 7968
                    }
                },
                "debug_payload": {
                    "llm_context": {
                        "effective_system": "You are the fusion judge.",
                        "provider_messages": [
                            {
                                "role": "user",
                                "content": "Merge panel answers."
                            }
                        ]
                    },
                    "assistant_message": {
                        "role": "assistant",
                        "content": "judge merged answer"
                    }
                },
                "content": "judge merged answer"
            }
        ]
    });

    let traces = collect_visible_internal_llm_tool_route_traces(&debug_payload);

    assert_eq!(traces.len(), 1);
    let detail = traces[0].detail_payload();
    let branch_trace = &detail["branch_traces"][0];
    assert_eq!(branch_trace["node_alias"], json!("LLM5"));
    assert_eq!(
        branch_trace["input_payload"]["prompt_messages"][0]["role"],
        json!("system")
    );
    assert_eq!(
        branch_trace["input_payload"]["prompt_messages"][0]["content"],
        json!("You are the fusion judge.")
    );
    assert_eq!(
        branch_trace["input_payload"]["prompt_messages"][1]["content"],
        json!("Merge panel answers.")
    );
    assert_eq!(
        branch_trace["output_payload"]["text"],
        json!("judge merged answer")
    );
    assert_eq!(
        branch_trace["metrics_payload"]["usage"]["total_tokens"],
        json!(7968)
    );
}
