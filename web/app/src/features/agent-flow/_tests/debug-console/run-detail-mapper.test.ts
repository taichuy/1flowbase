import { describe, expect, test } from 'vitest';

import type { FlowDebugRunDetail } from '../../api/runtime';
import {
  extractAssistantOutputText,
  mapRunDetailToConversation,
  mapRunDetailToTrace
} from '../../lib/debug-console/run-detail-mapper';

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

  test('uses node input payload view for trace display when backend provides one', () => {
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
          __runtime_debug_artifact: true,
          artifact_ref: 'artifact-start-input',
          preview: '{"history":[{"role":"system","content":"hidden"}]}'
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
    });
  });
});
