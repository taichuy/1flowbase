import { seedAccessFixture } from '../../../../test/fixtures/console-access';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import * as runtimeApi from '../../api/runtime';
import { useAgentFlowDebugSession } from '../../hooks/runtime/useAgentFlowDebugSession';
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

function createRunningRunDetail() {
  return {
    flow_run: {
      id: 'flow-run-live',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run' as const,
      status: 'running',
      target_node_id: null,
      input_payload: {
        'node-start': { query: '请总结退款政策' }
      },
      output_payload: {},
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-25T12:00:00Z',
      finished_at: null,
      created_at: '2026-04-25T12:00:00Z'
    },
    node_runs: [
      {
        id: 'node-run-start',
        flow_run_id: 'flow-run-live',
        node_id: 'node-start',
        node_type: 'start',
        node_alias: 'Start',
        status: 'succeeded',
        input_payload: {},
        output_payload: { query: '请总结退款政策' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T12:00:00Z',
        finished_at: '2026-04-25T12:00:00Z'
      },
      {
        id: 'node-run-llm',
        flow_run_id: 'flow-run-live',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'running',
        input_payload: { user_prompt: '请总结退款政策' },
        output_payload: {},
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T12:00:01Z',
        finished_at: null
      }
    ],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}

function createSucceededRunDetail() {
  return {
    flow_run: {
      ...createRunningRunDetail().flow_run,
      status: 'succeeded',
      output_payload: { answer: '退款政策摘要' },
      finished_at: '2026-04-25T12:00:02Z'
    },
    node_runs: [
      {
        id: 'node-run-start',
        flow_run_id: 'flow-run-live',
        node_id: 'node-start',
        node_type: 'start',
        node_alias: 'Start',
        status: 'succeeded',
        input_payload: {},
        output_payload: { query: '请总结退款政策' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T12:00:00Z',
        finished_at: '2026-04-25T12:00:00Z'
      },
      {
        id: 'node-run-llm',
        flow_run_id: 'flow-run-live',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: { user_prompt: '请总结退款政策' },
        output_payload: { text: '退款政策摘要' },
        error_payload: null,
        metrics_payload: { total_tokens: 128 },
        started_at: '2026-04-25T12:00:01Z',
        finished_at: '2026-04-25T12:00:02Z'
      },
      {
        id: 'node-run-answer',
        flow_run_id: 'flow-run-live',
        node_id: 'node-answer',
        node_type: 'answer',
        node_alias: 'Answer',
        status: 'succeeded',
        input_payload: { answer_template: '退款政策摘要' },
        output_payload: { answer: '退款政策摘要' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T12:00:02Z',
        finished_at: '2026-04-25T12:00:02Z'
      }
    ],
    checkpoints: [],
    callback_tasks: [],
    events: []
  };
}

function createCancelledRunDetail() {
  return {
    flow_run: {
      ...createRunningRunDetail().flow_run,
      status: 'cancelled',
      finished_at: '2026-04-25T12:00:01Z'
    },
    node_runs: createRunningRunDetail().node_runs,
    checkpoints: [],
    callback_tasks: [],
    events: [
      {
        id: 'event-cancel',
        flow_run_id: 'flow-run-live',
        node_run_id: null,
        sequence: 2,
        event_type: 'flow_run_cancelled',
        payload: { reason: 'manual_stop' },
        created_at: '2026-04-25T12:00:01Z'
      }
    ]
  };
}

beforeEach(() => {
  vi.useFakeTimers();
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
      policy_fingerprint: 'fixture-policy'
    }
  });
  seedAccessFixture(['application.view.all', 'application.edit.own']);
});

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('debug console live runtime', () => {
  test('polls run detail after start until terminal status', async () => {
    const queryClient = createQueryClient();
    const invalidateQueriesSpy = vi.spyOn(queryClient, 'invalidateQueries');
    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(createRunningRunDetail());
    const fetchApplicationRunDebugSnapshotSpy = vi
      .spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot')
      .mockResolvedValue(createSucceededRunDetail());
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await act(async () => {
      await result.current.submitPrompt('请总结退款政策');
    });

    expect(result.current.status).toBe('running');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });

    expect(fetchApplicationRunDebugSnapshotSpy).toHaveBeenCalledWith(
      'app-1',
      'flow-run-live'
    );
    expect(result.current.status).toBe('completed');
    expect(result.current.messages.at(-1)?.content).toBe('退款政策摘要');
    expect(invalidateQueriesSpy).toHaveBeenCalled();
  });

  test('sends cancel request when stopping a live run and updates session to cancelled', async () => {
    const queryClient = createQueryClient();
    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(createRunningRunDetail());
    const fetchApplicationRunDebugSnapshotSpy = vi.spyOn(
      runtimeApi,
      'fetchApplicationRunDebugSnapshot'
    );
    const cancelFlowDebugRunSpy = vi
      .spyOn(runtimeApi, 'cancelFlowDebugRun')
      .mockResolvedValue(createCancelledRunDetail());
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await act(async () => {
      await result.current.submitPrompt('请总结退款政策');
    });

    await act(async () => {
      await result.current.stopRun();
    });

    expect(cancelFlowDebugRunSpy).toHaveBeenCalledWith(
      'app-1',
      'flow-run-live',
      'csrf-123'
    );
    expect(result.current.status).toBe('cancelled');

    await act(async () => {
      await vi.advanceTimersByTimeAsync(250);
    });

    expect(fetchApplicationRunDebugSnapshotSpy).not.toHaveBeenCalled();
    expect(result.current.messages.at(-1)?.status).toBe('cancelled');
  });

  test('guards duplicate stop requests while cancellation is in flight', async () => {
    const queryClient = createQueryClient();
    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(createRunningRunDetail());
    const cancelFlowDebugRunSpy = vi
      .spyOn(runtimeApi, 'cancelFlowDebugRun')
      .mockImplementation(
        () =>
          new Promise((resolve) => {
            setTimeout(() => resolve(createCancelledRunDetail()), 50);
          })
      );
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await act(async () => {
      await result.current.submitPrompt('请总结退款政策');
    });

    expect(result.current.status).toBe('running');

    act(() => {
      void result.current.stopRun();
      void result.current.stopRun();
    });

    expect(result.current.stopping).toBe(true);
    expect(cancelFlowDebugRunSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(50);
    });

    expect(result.current.stopping).toBe(false);
    expect(result.current.status).toBe('cancelled');
  });
});
