import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import * as runtimeApi from '../../api/runtime';
import { useAgentFlowDebugSession } from '../../hooks/runtime/useAgentFlowDebugSession';
import {
  applyDebugStreamEventToAssistantMessage,
  applyDebugStreamEventToTrace,
  reconcileSnapshotTraceWithLiveEvents
} from '../../lib/debug-console/stream-events';
import { collectLlmToolCallbacks } from '../../components/debug-console/conversation/llm-tool-callbacks';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

function createQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
}

function createWrapper(queryClient: QueryClient) {
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    );
  };
}

function createStreamRunDetail(runId: string, answer = '退款政策摘要') {
  return {
    flow_run: {
      id: runId,
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run' as const,
      status: 'succeeded',
      target_node_id: null,
      input_payload: {
        'node-start': { query: '请总结退款政策' }
      },
      output_payload: { answer },
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-25T10:00:00Z',
      finished_at: '2026-04-25T10:00:02Z',
      created_at: '2026-04-25T10:00:00Z'
    },
    node_runs: [
      {
        id: 'node-run-start',
        flow_run_id: runId,
        node_id: 'node-start',
        node_type: 'start',
        node_alias: 'Start',
        status: 'succeeded',
        input_payload: {},
        output_payload: { query: '请总结退款政策' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T10:00:00Z',
        finished_at: '2026-04-25T10:00:00Z'
      },
      {
        id: 'node-run-llm',
        flow_run_id: runId,
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: { user_prompt: '请总结退款政策' },
        output_payload: { text: answer },
        error_payload: null,
        metrics_payload: { total_tokens: 128 },
        started_at: '2026-04-25T10:00:01Z',
        finished_at: '2026-04-25T10:00:02Z'
      },
      {
        id: 'node-run-answer',
        flow_run_id: runId,
        node_id: 'node-answer',
        node_type: 'answer',
        node_alias: 'Answer',
        status: 'succeeded',
        input_payload: { answer_template: answer },
        output_payload: { answer },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T10:00:02Z',
        finished_at: '2026-04-25T10:00:02Z'
      }
    ],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}

function answerTextDelta(
  text: string
): Extract<runtimeApi.FlowDebugRunStreamEvent, { type: 'text_delta' }> {
  return {
    type: 'text_delta',
    node_run_id: null,
    node_id: 'node-answer',
    text,
    presentation: {
      kind: 'answer',
      answer_node_id: 'node-answer',
      segment_index: 0,
      source_node_id: 'node-llm',
      source_node_run_id: 'node-run-llm',
      source_output_key: 'text'
    }
  };
}

function answerReasoningDelta(
  text: string
): Extract<runtimeApi.FlowDebugRunStreamEvent, { type: 'reasoning_delta' }> {
  return {
    ...answerTextDelta(text),
    type: 'reasoning_delta'
  };
}

beforeEach(() => {
  window.localStorage.clear();
  resetAuthStore();
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['application.view.all', 'application.edit.own']
    }
  });
});

afterEach(() => {
  vi.restoreAllMocks();
});

export {
  act,
  answerReasoningDelta,
  answerTextDelta,
  applyDebugStreamEventToAssistantMessage,
  applyDebugStreamEventToTrace,
  collectLlmToolCallbacks,
  createDefaultAgentFlowDocument,
  createQueryClient,
  createStreamRunDetail,
  createWrapper,
  expect,
  reconcileSnapshotTraceWithLiveEvents,
  renderHook,
  runtimeApi,
  test,
  useAgentFlowDebugSession,
  vi
};
