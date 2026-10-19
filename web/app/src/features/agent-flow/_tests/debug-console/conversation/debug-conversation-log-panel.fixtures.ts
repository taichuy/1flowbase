import type { AgentFlowDebugMessage } from '../../../api/runtime';

export const assistantMessage: AgentFlowDebugMessage = {
  id: 'assistant-1',
  role: 'assistant',
  status: 'running',
  runId: 'run-1',
  compatibilityModeLabel: 'OpenAI Responses',
  content: '你好，我可以帮你。',
  rawOutput: {
    answer: '你好，我可以帮你。'
  },
  statistics: {
    total_tokens: 154,
    unique_node_count: 2,
    tool_callback_count: 0
  },
  traceSummary: [
    {
      nodeId: 'node-start',
      nodeRunId: 'node-run-start',
      nodeAlias: 'Start',
      nodeType: 'start',
      status: 'succeeded',
      startedAt: '2026-04-25T10:00:00Z',
      finishedAt: '2026-04-25T10:00:00Z',
      durationMs: 79,
      inputPayload: {
        query: '你好?'
      },
      outputPayload: {
        query: '你好?'
      },
      errorPayload: null,
      metricsPayload: {},
      debugPayload: {}
    },
    {
      nodeId: 'node-llm',
      nodeRunId: 'node-run-llm',
      nodeAlias: 'LLM',
      nodeType: 'llm',
      status: 'succeeded',
      startedAt: '2026-04-25T10:00:01Z',
      finishedAt: '2026-04-25T10:00:05Z',
      durationMs: 4257,
      inputPayload: {
        prompt: '你好?'
      },
      outputPayload: {
        answer: '你好，我可以帮你。'
      },
      errorPayload: null,
      metricsPayload: {
        total_tokens: 154
      },
      debugPayload: {
        provider: 'openai'
      }
    }
  ]
};

export const llmRoundAssistantMessage: AgentFlowDebugMessage = {
  ...assistantMessage,
  traceSummary: assistantMessage.traceSummary.map((item) =>
    item.nodeId === 'node-llm'
      ? {
          ...item,
          outputPayload: {
            answer: 'weather is clear'
          },
          debugPayload: {
            provider: 'openai',
            llm_rounds: [
              {
                round_index: 0,
                usage: {
                  input_tokens: 11,
                  input_cache_hit_tokens: 5,
                  output_tokens: 3,
                  total_tokens: 14
                },
                assistant: {
                  role: 'assistant',
                  content: 'need tool',
                  tool_calls: [
                    {
                      id: 'call_weather',
                      name: 'lookup_weather',
                      call_usage: {
                        input_tokens: 11,
                        input_cache_hit_tokens: 5,
                        output_tokens: 3,
                        total_tokens: 14
                      },
                      arguments: {
                        city: 'Shanghai'
                      }
                    }
                  ]
                },
                finish_reason: 'tool_call'
              },
              {
                round_index: 1,
                assistant: {
                  role: 'assistant',
                  content: 'need tool'
                },
                tool_results: [
                  {
                    role: 'tool',
                    tool_call_id: 'call_weather',
                    token_delta: 10,
                    duration_ms: 1234,
                    result_context_usage: {
                      input_tokens: 20,
                      input_cache_hit_tokens: 8,
                      output_tokens: 4,
                      total_tokens: 24
                    },
                    content: '{"temperature":21}'
                  }
                ]
              },
              {
                round_index: 2,
                usage: {
                  input_tokens: 20,
                  input_cache_hit_tokens: 8,
                  output_tokens: 4,
                  total_tokens: 24
                },
                assistant: {
                  role: 'assistant',
                  content: 'weather is clear'
                },
                finish_reason: 'stop'
              }
            ]
          }
        }
      : item
  )
};

export const truncatedLlmRoundsAssistantMessage: AgentFlowDebugMessage = {
  ...assistantMessage,
  traceSummary: assistantMessage.traceSummary.map((item) =>
    item.nodeId === 'node-llm'
      ? {
          ...item,
          outputPayload: {
            answer: 'weather is clear'
          },
          debugPayload: {
            provider: 'openai',
            llm_rounds: {
              __runtime_debug_artifact: true,
              artifact_ref: 'artifact-llm-rounds',
              is_truncated: true,
              original_size_bytes: 2000,
              preview_size_bytes: 120,
              content_type: 'application/json',
              preview: '["call_weather"]',
              tool_callbacks: [
                {
                  id: 'call_weather',
                  name: 'lookup_weather',
                  callback_status: 'returned',
                  execution_status: 'unknown',
                  request_round_index: 0,
                  result_round_index: 0,
                  call_usage: {
                    input_tokens: 11,
                    input_cache_hit_tokens: 5,
                    output_tokens: 3,
                    total_tokens: 14
                  },
                  result_context_usage: {
                    input_tokens: 20,
                    input_cache_hit_tokens: 8,
                    output_tokens: 4,
                    total_tokens: 24
                  },
                  token_delta: 10,
                  duration_ms: 1234,
                  artifact_ref: 'artifact-tool-call-weather'
                }
              ]
            }
          }
        }
      : item
  )
};

export const toolCallbackDetailPayload = {
  id: 'call_weather',
  name: 'lookup_weather',
  callback_status: 'returned',
  execution_status: 'unknown',
  call_usage: {
    input_tokens: 11,
    input_cache_hit_tokens: 5,
    output_tokens: 3,
    total_tokens: 14
  },
  result_context_usage: {
    input_tokens: 20,
    input_cache_hit_tokens: 8,
    output_tokens: 4,
    total_tokens: 24
  },
  token_delta: 10,
  duration_ms: 1234,
  request_payload: {
    id: 'call_weather',
    name: 'lookup_weather',
    call_usage: {
      input_tokens: 11,
      input_cache_hit_tokens: 5,
      output_tokens: 3,
      total_tokens: 14
    },
    arguments: {
      city: 'Shanghai'
    }
  },
  callback_payload: {
    role: 'tool',
    tool_call_id: 'call_weather',
    token_delta: 10,
    duration_ms: 1234,
    result_context_usage: {
      input_tokens: 20,
      input_cache_hit_tokens: 8,
      output_tokens: 4,
      total_tokens: 24
    },
    content: '{"temperature":21}',
    adapter_trace_id: 'trace-weather-1'
  },
  parsed_result: {
    tool_call_id: 'call_weather',
    content: '{"temperature":21}'
  },
  request_round_index: 0,
  result_round_index: 0
};

export const multiLlmRunAssistantMessage: AgentFlowDebugMessage = {
  ...assistantMessage,
  traceSummary: [
    assistantMessage.traceSummary[0],
    {
      ...assistantMessage.traceSummary[1],
      nodeRunId: 'node-run-llm-1',
      status: 'succeeded',
      durationMs: 5400,
      outputPayload: {
        usage: { total_tokens: 8035 },
        tool_calls: [{ id: 'call_weather', name: 'lookup_weather' }]
      },
      debugPayload: {
        llm_rounds: [
          {
            round_index: 0,
            assistant: {
              role: 'assistant',
              content: 'need weather',
              tool_calls: [
                {
                  id: 'call_weather',
                  name: 'lookup_weather',
                  arguments: { city: 'Shanghai' }
                }
              ]
            },
            finish_reason: 'tool_call'
          }
        ]
      }
    },
    {
      ...assistantMessage.traceSummary[1],
      nodeRunId: 'node-run-llm-2',
      status: 'succeeded',
      durationMs: 6900,
      outputPayload: {
        usage: { total_tokens: 8259 },
        tool_calls: [{ id: 'call_policy', name: 'read_policy' }]
      },
      debugPayload: {
        llm_rounds: [
          {
            round_index: 0,
            assistant: { role: 'assistant', content: 'continue' },
            tool_results: [
              {
                role: 'tool',
                tool_call_id: 'call_weather',
                content: '{"temperature":21}'
              }
            ]
          },
          {
            round_index: 1,
            assistant: {
              role: 'assistant',
              content: 'need policy',
              tool_calls: [
                {
                  id: 'call_policy',
                  name: 'read_policy',
                  arguments: { path: '.memory/user-memory.md' }
                }
              ]
            },
            finish_reason: 'tool_call'
          }
        ]
      }
    },
    {
      ...assistantMessage.traceSummary[1],
      nodeRunId: 'node-run-llm-3',
      status: 'succeeded',
      durationMs: 8500,
      outputPayload: {
        answer: 'weather is clear'
      },
      debugPayload: {
        llm_rounds: [
          {
            round_index: 0,
            assistant: { role: 'assistant', content: 'finish' },
            tool_results: [
              {
                role: 'tool',
                tool_call_id: 'call_policy',
                content: 'memory loaded'
              }
            ]
          },
          {
            round_index: 1,
            assistant: {
              role: 'assistant',
              content: 'weather is clear'
            },
            finish_reason: 'stop'
          }
        ]
      }
    }
  ]
};

export const fusionSummaryOnlyAssistantMessage: AgentFlowDebugMessage = {
  ...assistantMessage,
  traceSummary: [
    {
      ...assistantMessage.traceSummary[1],
      nodeId: 'node-main-llm',
      nodeRunId: 'node-run-main-llm',
      nodeAlias: 'LLM',
      debugPayload: {
        llm_rounds: [
          {
            round_index: 0,
            assistant: {
              role: 'assistant',
              tool_calls: [
                {
                  id: 'call_fusion',
                  name: 'fusion_review'
                }
              ]
            }
          }
        ],
        visible_internal_llm_tool_trace: [
          {
            kind: 'visible_internal_llm_tool_trace',
            preview_kind: 'visible_internal_llm_tool_trace',
            route_kind: 'fusion',
            tool_call_id: 'call_fusion',
            tool_name: 'fusion_review',
            status: 'succeeded',
            branch_summaries: [
              {
                node_id: 'node-panel-a',
                node_alias: 'LLM2',
                node_type: 'llm',
                status: 'succeeded',
                output_summary: {
                  kind: 'text',
                  preview: 'summary only',
                  char_count: 12,
                  truncated: false
                }
              }
            ]
          }
        ]
      }
    }
  ]
};

export const fusionHistoricalBranchDetailAssistantMessage: AgentFlowDebugMessage = {
  ...assistantMessage,
  traceSummary: [
    {
      ...assistantMessage.traceSummary[1],
      nodeId: 'node-main-llm',
      nodeRunId: 'node-run-main-llm',
      nodeAlias: 'LLM',
      debugPayload: {
        llm_rounds: [
          {
            round_index: 0,
            assistant: {
              role: 'assistant',
              tool_calls: [
                {
                  id: 'call_fusion',
                  name: 'fusion_review'
                }
              ]
            }
          }
        ],
        visible_internal_llm_tool_trace: [
          {
            kind: 'visible_internal_llm_tool_trace',
            preview_kind: 'visible_internal_llm_tool_trace',
            route_kind: 'fusion',
            tool_call_id: 'call_fusion',
            tool_name: 'fusion_review',
            status: 'succeeded',
            branch_count: 1,
            branch_traces: [
              {
                node_id: 'node-judge',
                node_alias: 'LLM5',
                node_type: 'llm',
                status: 'succeeded',
                route_model: 'gpt-5.4-mini',
                input_payload: {
                  prompt_messages: [
                    {
                      role: 'system',
                      content: 'You are the fusion judge.'
                    },
                    {
                      role: 'user',
                      content: 'Merge panel answers.'
                    }
                  ]
                },
                output_payload: {
                  text: 'judge merged answer'
                },
                metrics_payload: {
                  usage: {
                    input_tokens: 5513,
                    output_tokens: 2455,
                    total_tokens: 7968
                  }
                },
                debug_payload: {
                  assistant_message: {
                    role: 'assistant',
                    content: 'judge merged answer'
                  }
                },
                output_summary: {
                  kind: 'text',
                  preview: 'judge merged answer',
                  char_count: 19,
                  truncated: false
                }
              }
            ]
          }
        ]
      }
    }
  ]
};

export const answerSnapshotAssistantMessage: AgentFlowDebugMessage = {
  ...assistantMessage,
  status: 'waiting_callback',
  content: '',
  rawOutput: {
    answer: 'LLM1 final\n----\n'
  },
  traceSummary: [
    {
      nodeId: 'node-llm-2',
      nodeRunId: 'node-run-llm-2',
      nodeAlias: 'LLM2',
      nodeType: 'llm',
      status: 'waiting_callback',
      startedAt: '2026-04-25T10:00:01Z',
      finishedAt: null,
      durationMs: null,
      inputPayload: {
        prompt: 'continue'
      },
      outputPayload: {
        tool_calls: []
      },
      errorPayload: null,
      metricsPayload: {},
      debugPayload: {},
      answerSnapshot: {
        kind: 'answer',
        text: 'LLM1 final\n----\n',
        outputPayload: {
          answer: 'LLM1 final\n----\n'
        },
        complete: false,
        materializedFrom: 'waiting_prefix',
        answerNodeId: 'node-answer',
        answerNodeRunId: 'node-run-answer-snapshot',
        waitingNodeId: 'node-llm-2',
        waitingNodeRunId: 'node-run-llm-2'
      }
    }
  ]
};
