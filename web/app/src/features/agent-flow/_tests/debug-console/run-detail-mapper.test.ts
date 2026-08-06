import { describe, expect, test } from 'vitest';

import type { FlowDebugRunDetail } from '../../api/runtime';
import {
  extractAssistantOutputText,
  mapRunDetailToConversation,
  mapRunDetailToTrace
} from '../../lib/debug-console/run-detail-mapper';
import { collectLlmToolCallbacks } from '../../components/debug-console/conversation/llm-tool-callbacks';

function baseDetail(): FlowDebugRunDetail {
  return {
    flow_run: {
      id: 'flow-run-1',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run',
      status: 'failed',
      target_node_id: null,
      input_payload: {},
      output_payload: {},
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-26T10:00:00Z',
      finished_at: '2026-04-26T10:00:01Z',
      created_at: '2026-04-26T10:00:00Z'
    },
    node_runs: [],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}

describe('run detail mapper', () => {
  test('prefers provider error message over structural error kind text', () => {
    const detail = baseDetail();
    detail.flow_run.output_payload = {
      text: null,
      error: {
        error_kind: 'provider_invalid_response',
        message: 'upstream unavailable: provider_runtime',
        protocol: 'openai_compatible'
      }
    };

    expect(extractAssistantOutputText(detail)).toBe(
      'upstream unavailable: provider_runtime'
    );
  });

  test('prefers answer or text output before metadata strings', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'succeeded';
    detail.flow_run.output_payload = {
      finish_reason: 'stop',
      answer: '退款政策摘要'
    };

    expect(extractAssistantOutputText(detail)).toBe('退款政策摘要');
  });

  test('uses runtime artifact preview instead of artifact metadata strings', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'succeeded';
    detail.flow_run.output_payload = {
      answer: {
        __runtime_debug_artifact: true,
        is_truncated: true,
        original_size_bytes: 8192,
        preview_size_bytes: 256,
        content_type: 'text/plain',
        artifact_ref: 'artifact-answer',
        preview: '截断预览内容'
      }
    };

    expect(extractAssistantOutputText(detail)).toBe('截断预览内容');
  });

  test('ignores root output artifact preview json and reads answer node output', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'succeeded';
    detail.flow_run.output_payload = {
      __runtime_debug_artifact: true,
      is_truncated: true,
      artifact_ref: 'artifact-flow-output',
      preview:
        '{"env":{},"sys":{"workflow_run_id":"flow-run-1"},"answer":"错误的整包 JSON 预览"}'
    };
    detail.node_runs = [
      {
        id: 'node-run-answer',
        flow_run_id: 'flow-run-1',
        node_id: 'node-answer',
        node_type: 'answer',
        node_alias: 'Answer',
        status: 'succeeded',
        input_payload: {},
        output_payload: {
          env: {},
          sys: { workflow_run_id: 'flow-run-1' },
          answer: '真正回答'
        },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-26T10:00:00Z',
        finished_at: '2026-04-26T10:00:01Z'
      }
    ];

    expect(extractAssistantOutputText(detail)).toBe('真正回答');
  });

  test('does not fall back to sys or env strings when answer is missing', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'succeeded';
    detail.flow_run.output_payload = {
      env: {},
      sys: {
        workflow_run_id: 'flow-run-1',
        application_id: 'app-1'
      }
    };

    expect(extractAssistantOutputText(detail)).toBe('');
  });

  test('uses provider text delta events while a run is still producing output', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'running';
    detail.events = [
      {
        id: 'event-1',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 1,
        event_type: 'text_delta',
        payload: { type: 'text_delta', delta: '退款' },
        created_at: '2026-04-26T10:00:00Z'
      },
      {
        id: 'event-2',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 2,
        event_type: 'text_delta',
        payload: { type: 'text_delta', delta: '政策摘要' },
        created_at: '2026-04-26T10:00:01Z'
      }
    ];

    expect(extractAssistantOutputText(detail)).toBe('退款政策摘要');
  });

  test('restores persisted reasoning and answer deltas as one ordered Dify-style content field', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'running';
    detail.events = [
      {
        id: 'event-1',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 1,
        event_type: 'reasoning_delta',
        payload: { type: 'reasoning_delta', text: '先分析' },
        created_at: '2026-04-26T10:00:00Z'
      },
      {
        id: 'event-2',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 2,
        event_type: 'text_delta',
        payload: { type: 'text_delta', text: '结果' },
        created_at: '2026-04-26T10:00:01Z'
      }
    ];

    expect(mapRunDetailToConversation(detail)).toEqual(
      expect.objectContaining({
        content: '<think>先分析</think>结果'
      })
    );
    expect(mapRunDetailToConversation(detail)).not.toHaveProperty(
      'reasoningContent'
    );
  });

  test('restores final output text when durable events only contain reasoning', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'succeeded';
    detail.flow_run.output_payload = {
      text: '<think>先分析</think>结果'
    };
    detail.events = [
      {
        id: 'event-1',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 1,
        event_type: 'reasoning_delta',
        payload: { type: 'reasoning_delta', text: '先分析' },
        created_at: '2026-04-26T10:00:00Z'
      }
    ];

    expect(mapRunDetailToConversation(detail)).toEqual(
      expect.objectContaining({
        content: '<think>先分析</think>结果'
      })
    );
  });

  test('maps trace debug payload separately from public output and metrics', () => {
    const detail = baseDetail();
    detail.node_runs = [
      {
        id: 'node-run-llm',
        flow_run_id: 'flow-run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: { prompt: 'hi' },
        output_payload: { text: 'hello' },
        error_payload: null,
        metrics_payload: { total_tokens: 8 },
        debug_payload: {
          response_ref: 'runtime_artifact:inline:response-1'
        },
        started_at: '2026-04-26T10:00:00Z',
        finished_at: '2026-04-26T10:00:01Z'
      }
    ];

    expect(mapRunDetailToTrace(detail)[0]).toEqual(
      expect.objectContaining({
        outputPayload: { text: 'hello' },
        metricsPayload: { total_tokens: 8 },
        debugPayload: {
          response_ref: 'runtime_artifact:inline:response-1'
        }
      })
    );
  });

  test('restores persisted assistant tool lifecycle events into the LLM trace', () => {
    const detail = baseDetail();
    detail.node_runs = [
      {
        id: 'node-run-llm',
        flow_run_id: 'flow-run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-26T10:00:00Z',
        finished_at: '2026-04-26T10:00:01Z'
      }
    ];
    detail.events = [
      {
        id: 'tool-started',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 3,
        event_type: 'assistant_tool_call_started',
        payload: {
          node_id: 'node-llm',
          tool_call: {
            id: 'call-weather',
            name: 'lookup_weather',
            arguments: { city: 'Shanghai' }
          }
        },
        created_at: '2026-04-26T10:00:00Z'
      },
      {
        id: 'tool-finished',
        flow_run_id: 'flow-run-1',
        node_run_id: 'node-run-llm',
        sequence: 4,
        event_type: 'assistant_tool_call_finished',
        payload: {
          node_id: 'node-llm',
          tool_call: {
            id: 'call-weather',
            name: 'lookup_weather',
            arguments: { city: 'Shanghai' }
          },
          tool_result: {
            tool_call_id: 'call-weather',
            name: 'lookup_weather',
            content: { temperature: 26 },
            is_error: false
          },
          duration_ms: 42
        },
        created_at: '2026-04-26T10:00:01Z'
      }
    ];

    expect(collectLlmToolCallbacks(mapRunDetailToTrace(detail)[0]?.debugPayload)).toMatchObject([
      {
        id: 'call-weather',
        callbackStatus: 'returned',
        executionStatus: 'succeeded',
        duration_ms: 42,
        parsedResult: { content: { temperature: 26 }, is_error: false }
      }
    ]);
  });

  test('uses node input payload truth for trace display when backend also provides a view', () => {
    const detail = baseDetail();
    detail.node_runs = [
      {
        id: 'node-run-start',
        flow_run_id: 'flow-run-1',
        node_id: 'node-start',
        node_type: 'start',
        node_alias: 'Start',
        status: 'succeeded',
        input_payload: {
          query: '总结退款政策',
          model: 'deepseek-chat',
          files: [],
          history: {
            __runtime_debug_artifact: true,
            artifact_scope: 'field',
            field_path: ['history'],
            artifact_ref: 'artifact-start-history',
            is_truncated: true,
            original_size_bytes: 4096,
            preview_size_bytes: 128,
            content_type: 'application/json',
            preview: '[{"role":"system","content":"hidden"}]'
          },
          sys: { workflow_run_id: 'flow-run-1' },
          env: {}
        },
        input_payload_view: {
          kind: 'start_input_summary',
          artifact_ref: 'artifact-start-input',
          is_truncated: true,
          preview: {
            query: '总结退款政策',
            model: 'deepseek-chat',
            files: [],
            history: ['...'],
            tools: ['...']
          }
        },
        output_payload: {},
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-26T10:00:00Z',
        finished_at: '2026-04-26T10:00:01Z'
      }
    ];

    expect(mapRunDetailToTrace(detail)[0].inputPayload).toEqual({
      query: '总结退款政策',
      model: 'deepseek-chat',
      files: [],
      history: {
        __runtime_debug_artifact: true,
        artifact_scope: 'field',
        field_path: ['history'],
        artifact_ref: 'artifact-start-history',
        is_truncated: true,
        original_size_bytes: 4096,
        preview_size_bytes: 128,
        content_type: 'application/json',
        preview: '[{"role":"system","content":"hidden"}]'
      },
      sys: { workflow_run_id: 'flow-run-1' },
      env: {}
    });
  });

  test('attaches waiting answer snapshot to the waiting node trace item', () => {
    const detail = baseDetail();
    detail.flow_run.status = 'waiting_callback';
    detail.flow_run.output_payload = {
      answer: 'LLM1 final\n----\n'
    };
    detail.answer_snapshot = {
      kind: 'answer',
      text: 'LLM1 final\n----\n',
      output_payload: {
        answer: 'LLM1 final\n----\n'
      },
      complete: false,
      materialized_from: 'waiting_prefix',
      answer_node_id: 'node-answer',
      answer_node_run_id: 'node-run-answer-snapshot',
      waiting_node_id: 'node-llm-2',
      waiting_node_run_id: 'node-run-llm-2'
    };
    detail.node_runs = [
      {
        id: 'node-run-llm-2',
        flow_run_id: 'flow-run-1',
        node_id: 'node-llm-2',
        node_type: 'llm',
        node_alias: 'LLM2',
        status: 'waiting_callback',
        input_payload: {
          prompt: 'continue'
        },
        output_payload: {
          tool_calls: []
        },
        error_payload: null,
        metrics_payload: {},
        debug_payload: {},
        started_at: '2026-04-26T10:00:00Z',
        finished_at: null
      }
    ];

    const traceItems = mapRunDetailToTrace(detail);

    expect(traceItems).toHaveLength(1);
    expect(traceItems[0]).toEqual(
      expect.objectContaining({
        nodeId: 'node-llm-2',
        nodeRunId: 'node-run-llm-2',
        answerSnapshot: expect.objectContaining({
          text: 'LLM1 final\n----\n',
          complete: false,
          materializedFrom: 'waiting_prefix',
          answerNodeId: 'node-answer',
          answerNodeRunId: 'node-run-answer-snapshot',
          waitingNodeId: 'node-llm-2',
          waitingNodeRunId: 'node-run-llm-2'
        })
      })
    );
    expect(traceItems.some((item) => item.nodeType === 'answer')).toBe(false);
  });
});
