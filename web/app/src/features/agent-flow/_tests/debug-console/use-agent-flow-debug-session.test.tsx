import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { act, renderHook, waitFor } from '@testing-library/react';
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

function createSucceededRunDetail(): runtimeApi.FlowDebugRunDetail {
  return {
    flow_run: {
      id: 'flow-run-1',
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
      output_payload: {
        answer: '退款政策摘要'
      },
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-25T10:00:00Z',
      finished_at: '2026-04-25T10:00:02Z',
      created_at: '2026-04-25T10:00:00Z'
    },
    node_runs: [
      {
        id: 'node-run-start',
        flow_run_id: 'flow-run-1',
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
        flow_run_id: 'flow-run-1',
        node_id: 'node-llm',
        node_type: 'llm',
        node_alias: 'LLM',
        status: 'succeeded',
        input_payload: { user_prompt: '请总结退款政策' },
        output_payload: {
          text: '退款政策摘要',
          usage: { total_tokens: 128 },
          raw_response: { id: 'chatcmpl-1' }
        },
        error_payload: null,
        metrics_payload: { total_tokens: 128 },
        started_at: '2026-04-25T10:00:00Z',
        finished_at: '2026-04-25T10:00:01Z'
      },
      {
        id: 'node-run-answer',
        flow_run_id: 'flow-run-1',
        node_id: 'node-answer',
        node_type: 'answer',
        node_alias: 'Answer',
        status: 'succeeded',
        input_payload: { answer_template: '退款政策摘要' },
        output_payload: { answer: '退款政策摘要' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T10:00:01Z',
        finished_at: '2026-04-25T10:00:02Z'
      }
    ],
    checkpoints: [],
    callback_tasks: [],
    events: [
      {
        id: 'event-1',
        flow_run_id: 'flow-run-1',
        node_run_id: null,
        sequence: 1,
        event_type: 'flow.started',
        payload: {},
        created_at: '2026-04-25T10:00:00Z'
      }
    ]
  };
}

function createWaitingHumanRunDetail() {
  return {
    flow_run: {
      id: 'flow-run-2',
      application_id: 'app-1',
      flow_id: 'flow-1',
      draft_id: 'draft-1',
      compiled_plan_id: 'plan-1',
      run_mode: 'debug_flow_run' as const,
      status: 'waiting_human',
      target_node_id: null,
      input_payload: {
        'node-start': { query: '请人工审核退款申请' }
      },
      output_payload: {},
      error_payload: null,
      created_by: 'user-1',
      started_at: '2026-04-25T11:00:00Z',
      finished_at: null,
      created_at: '2026-04-25T11:00:00Z'
    },
    node_runs: [
      {
        id: 'node-run-start',
        flow_run_id: 'flow-run-2',
        node_id: 'node-start',
        node_type: 'start',
        node_alias: 'Start',
        status: 'succeeded',
        input_payload: {},
        output_payload: { query: '请人工审核退款申请' },
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T11:00:00Z',
        finished_at: '2026-04-25T11:00:00Z'
      },
      {
        id: 'node-run-human',
        flow_run_id: 'flow-run-2',
        node_id: 'node-human',
        node_type: 'human',
        node_alias: '人工审核',
        status: 'waiting_human',
        input_payload: {},
        output_payload: {},
        error_payload: null,
        metrics_payload: {},
        started_at: '2026-04-25T11:00:01Z',
        finished_at: null
      }
    ],
    checkpoints: [
      {
        id: 'checkpoint-1',
        flow_run_id: 'flow-run-2',
        node_run_id: 'node-run-human',
        status: 'waiting_human',
        reason: '等待人工审核',
        locator_payload: { node_id: 'node-human' },
        variable_snapshot: { ticket_id: 'ticket-1' },
        external_ref_payload: { prompt: '请确认是否批准退款' },
        created_at: '2026-04-25T11:00:01Z'
      }
    ],
    callback_tasks: [],
    events: []
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

describe('useAgentFlowDebugSession', () => {
  test('initializes a new draft with an empty query input', () => {
    const queryClient = createQueryClient();
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

    expect(result.current.runContext.remembered).toBe(false);
    expect(result.current.runContext.fields).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          key: 'query',
          value: ''
        })
      ])
    );
  });

  test('restores node preview variable cache from durable runtime snapshot', async () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const fetchSnapshotSpy = vi
      .spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValue({
        variable_cache: {
          'node-llm': {
            text: '沿用 durable 输出'
          }
        }
      });

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await waitFor(() => {
      expect(result.current.getNodePreviewVariableCache()).toEqual(
        expect.objectContaining({
          'node-llm': expect.objectContaining({
            text: '沿用 durable 输出'
          })
        })
      );
    });
    expect(result.current.variableGroups[0]).toEqual(
      expect.objectContaining({
        title: 'LLM',
        items: expect.arrayContaining([
          expect.objectContaining({
            key: 'node-llm.text',
            value: '沿用 durable 输出'
          })
        ])
      })
    );
    expect(fetchSnapshotSpy).toHaveBeenCalledWith('app-1');
  });

  test('restores durable variable cache from backend after remount without localStorage', async () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const fetchSnapshotSpy = vi
      .spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValueOnce({ variable_cache: {} })
      .mockResolvedValueOnce({
        variable_cache: {
          'node-llm': {
            text: '后端持久化缓存'
          }
        }
      });

    const view = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await waitFor(() => {
      expect(fetchSnapshotSpy).toHaveBeenCalledTimes(1);
    });
    view.unmount();

    const utils = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await waitFor(() => {
      expect(fetchSnapshotSpy).toHaveBeenCalledTimes(2);
    });

    await waitFor(() => {
      expect(fetchSnapshotSpy).toHaveBeenLastCalledWith('app-1');
      expect(utils.result.current.getNodePreviewVariableCache()).toEqual(
        expect.objectContaining({
          'node-llm': expect.objectContaining({
            text: '后端持久化缓存'
          })
        })
      );
    });
  });

  test('restores durable variable cache from backend latest snapshot', async () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const fetchSnapshotSpy = vi
      .spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValue({
        variable_cache: {
          'node-llm': {
            text: '刷新后沿用最新 run 输出'
          }
        }
      });

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await waitFor(() => {
      expect(fetchSnapshotSpy).toHaveBeenCalledWith('app-1');
      expect(result.current.getNodePreviewVariableCache()).toEqual(
        expect.objectContaining({
          'node-llm': expect.objectContaining({
            text: '刷新后沿用最新 run 输出'
          })
        })
      );
    });
  });

  test('ignores a delayed durable snapshot after resetting variable cache', async () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    let resolveSnapshot:
      | ((value: runtimeApi.DebugVariableSnapshot) => void)
      | null = null;
    vi.spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockReturnValueOnce(
        new Promise<runtimeApi.DebugVariableSnapshot>((resolve) => {
          resolveSnapshot = resolve;
        })
      )
      .mockResolvedValue({ variable_cache: {} });

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    act(() => {
      result.current.resetVariableCache();
    });
    await act(async () => {
      resolveSnapshot?.({
        variable_cache: {
          'node-llm': {
            text: '迟到输出'
          }
        }
      });
    });

    expect(
      result.current.getNodePreviewVariableCache()['node-llm']
    ).toBeUndefined();
  });

  test('creates user and assistant messages after a debug run succeeds', async () => {
    const queryClient = createQueryClient();
    const invalidateQueriesSpy = vi.spyOn(queryClient, 'invalidateQueries');
    const startFlowDebugRunSpy = vi
      .spyOn(runtimeApi, 'startFlowDebugRun')
      .mockResolvedValue(createSucceededRunDetail());
    const fetchSnapshotSpy = vi
      .spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValueOnce({ variable_cache: {} })
      .mockResolvedValue({
        variable_cache: {
          'node-start': {
            query: '请总结退款政策'
          },
          'node-llm': {
            text: '持久化表里的退款政策摘要'
          }
        }
      });
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

    await waitFor(() => {
      expect(result.current.status).toBe('completed');
    });

    expect(startFlowDebugRunSpy).toHaveBeenCalledWith(
      'app-1',
      {
        document,
        debug_session_id: expect.stringMatching(/^app-1:draft-1:/),
        input_payload: {
          'node-start': {
            files: [],
            history: [],
            max_output_tokens: null,
            model: '',
            protocol_context: null,
            query: '请总结退款政策',
            reasoning_effort: '',
            system: '',
            tool_choice: {},
            tools: []
          }
        }
      },
      'csrf-123'
    );
    expect(result.current.messages).toEqual([
      expect.objectContaining({
        role: 'user',
        content: '请总结退款政策'
      }),
      expect.objectContaining({
        role: 'assistant',
        status: 'completed',
        runId: 'flow-run-1',
        content: '退款政策摘要'
      })
    ]);
    expect(result.current.traceItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          nodeAlias: 'LLM',
          status: 'succeeded',
          durationMs: 1000
        })
      ])
    );
    expect(fetchSnapshotSpy).toHaveBeenCalledTimes(2);
    expect(fetchSnapshotSpy).toHaveBeenLastCalledWith('app-1');
    expect(result.current.variableGroups.map((group) => group.title)).toEqual(
      expect.arrayContaining(['LLM', 'Start'])
    );
    const startGroup = result.current.variableGroups.find(
      (group) => group.title === 'Start'
    );
    const startKeys = (startGroup?.items ?? []).map((item) => item.key);

    expect(startKeys).toContain('node-start.query');
    expect(startKeys).toContain('node-start.system');
    expect(startKeys).toContain('node-start.model');
    expect(startKeys).not.toContain('node-llm.user_prompt');
    expect(startKeys).not.toContain('node-answer.answer_template');
    expect(result.current.getNodePreviewVariableCache()).toEqual(
      expect.objectContaining({
        'node-start': expect.objectContaining({
          query: '请总结退款政策'
        }),
        'node-llm': expect.objectContaining({
          text: '持久化表里的退款政策摘要'
        })
      })
    );
    expect(
      result.current.getNodePreviewVariableCache()['node-llm']
    ).not.toHaveProperty('usage');
    expect(
      result.current.getNodePreviewVariableCache()['node-llm']
    ).not.toHaveProperty('raw_response');
    expect(
      result.current.getNodePreviewVariableCache()['node-llm']
    ).not.toHaveProperty('user_prompt');
    expect(
      result.current.getNodePreviewVariableCache()['node-answer']
    ).toBeUndefined();
    expect(window.localStorage.length).toBe(0);
    expect(invalidateQueriesSpy).toHaveBeenCalledWith({
      queryKey: ['applications', 'app-1', 'runtime']
    });
  });

  test('hydrates truncated output artifacts before rendering run results', async () => {
    const queryClient = createQueryClient();
    const detail = createSucceededRunDetail();
    detail.flow_run.output_payload = {
      answer: {
        __runtime_debug_artifact: true,
        is_truncated: true,
        original_size_bytes: 8192,
        preview_size_bytes: 256,
        content_type: 'text/plain',
        artifact_ref: 'artifact-answer',
        preview: '完整回答的预览'
      }
    };
    detail.node_runs[1]!.output_payload = {
      text: {
        __runtime_debug_artifact: true,
        is_truncated: true,
        original_size_bytes: 8192,
        preview_size_bytes: 256,
        content_type: 'text/plain',
        artifact_ref: 'artifact-llm-text',
        preview: '模型输出预览'
      },
      usage: { total_tokens: 128 }
    };
    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(detail);
    vi.spyOn(runtimeApi, 'fetchRuntimeDebugArtifact').mockImplementation(
      async (_applicationId, artifactRef) => {
        if (artifactRef === 'artifact-answer') {
          return '完整回答内容，不应该显示 artifact_ref 或预览截断值';
        }

        if (artifactRef === 'artifact-llm-text') {
          return '完整模型输出内容';
        }

        throw new Error(`unexpected artifact: ${artifactRef}`);
      }
    );
    vi.spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValueOnce({ variable_cache: {} })
      .mockResolvedValue({ variable_cache: {} });
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
      await result.current.submitPrompt('介绍一下你自己');
    });

    await waitFor(() => {
      expect(result.current.messages[1]).toEqual(
        expect.objectContaining({
          content: '完整回答内容，不应该显示 artifact_ref 或预览截断值'
        })
      );
    });
    expect(result.current.traceItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          outputPayload: expect.objectContaining({
            text: '完整模型输出内容'
          })
        })
      ])
    );
    expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledWith(
      'app-1',
      'artifact-answer'
    );
    expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledWith(
      'app-1',
      'artifact-llm-text'
    );
  });

  test('hydrates truncated start input field artifacts before projecting history and tools variables', async () => {
    const queryClient = createQueryClient();
    const detail = createSucceededRunDetail();
    detail.flow_run.input_payload = {
      'node-start': {
        query: '请总结退款政策',
        model: 'deepseek-chat',
        files: [],
        history: {
          __runtime_debug_artifact: true,
          artifact_scope: 'field',
          field_path: ['node-start', 'history'],
          is_truncated: true,
          original_size_bytes: 8192,
          preview_size_bytes: 256,
          content_type: 'application/json',
          artifact_ref: 'artifact-start-history',
          preview: '[{"role":"user","content":"之前'
        },
        tools: {
          __runtime_debug_artifact: true,
          artifact_scope: 'field',
          field_path: ['node-start', 'tools'],
          is_truncated: true,
          original_size_bytes: 8192,
          preview_size_bytes: 256,
          content_type: 'application/json',
          artifact_ref: 'artifact-start-tools',
          preview: '[{"name":"read'
        }
      }
    };
    detail.node_runs[0]!.input_payload = {
      query: '请总结退款政策',
      model: 'deepseek-chat',
      files: [],
      history: {
        __runtime_debug_artifact: true,
        artifact_scope: 'field',
        field_path: ['history'],
        is_truncated: true,
        original_size_bytes: 4096,
        preview_size_bytes: 128,
        content_type: 'application/json',
        artifact_ref: 'artifact-start-history',
        preview: '[{"role":"user","content":"之前'
      },
      tools: {
        __runtime_debug_artifact: true,
        artifact_scope: 'field',
        field_path: ['tools'],
        is_truncated: true,
        original_size_bytes: 4096,
        preview_size_bytes: 128,
        content_type: 'application/json',
        artifact_ref: 'artifact-start-tools',
        preview: '[{"name":"read'
      }
    };
    vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockRejectedValue(
      new Error('stream unavailable')
    );
    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(detail);
    vi.spyOn(runtimeApi, 'fetchRuntimeDebugArtifact').mockImplementation(
      async (_applicationId, artifactRef) => {
        if (artifactRef === 'artifact-start-history') {
          return [
            { role: 'user', content: '之前的问题' },
            { role: 'assistant', content: '之前的回答' }
          ];
        }

        if (artifactRef === 'artifact-start-tools') {
          return [{ name: 'read_file' }, { name: 'search' }];
        }

        throw new Error(`unexpected artifact: ${artifactRef}`);
      }
    );
    vi.spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValueOnce({ variable_cache: {} })
      .mockResolvedValue({ variable_cache: {} });
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

    await waitFor(() => {
      expect(result.current.getNodePreviewVariableCache()).toEqual(
        expect.objectContaining({
          'node-start': expect.objectContaining({
            query: '请总结退款政策',
            model: 'deepseek-chat',
            history: [
              { role: 'user', content: '之前的问题' },
              { role: 'assistant', content: '之前的回答' }
            ],
            tools: [{ name: 'read_file' }, { name: 'search' }],
            files: []
          })
        })
      );
    });
    expect(
      result.current.variableGroups
        .find((group) => group.title === 'Start')
        ?.items.find((item) => item.key === 'node-start.history')?.value
    ).toEqual([
      { role: 'user', content: '之前的问题' },
      { role: 'assistant', content: '之前的回答' }
    ]);
    expect(
      result.current.variableGroups
        .find((group) => group.title === 'Start')
        ?.items.find((item) => item.key === 'node-start.tools')?.value
    ).toEqual([{ name: 'read_file' }, { name: 'search' }]);
    expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledWith(
      'app-1',
      'artifact-start-history'
    );
    expect(runtimeApi.fetchRuntimeDebugArtifact).toHaveBeenCalledWith(
      'app-1',
      'artifact-start-tools'
    );
  });

  test('persists edited variable cache values across editor remounts', async () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(
      createSucceededRunDetail()
    );
    vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockRejectedValue(
      new Error('stream unavailable')
    );
    const upsertCacheSpy = vi
      .spyOn(runtimeApi, 'upsertDebugVariableCacheEntry')
      .mockResolvedValue({ ok: true });
    vi.spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValueOnce({ variable_cache: {} })
      .mockResolvedValue({
        variable_cache: {
          'node-llm': {
            text: '手动调试缓存'
          }
        }
      });

    const view = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await act(async () => {
      await view.result.current.submitPrompt('请总结退款政策');
    });

    act(() => {
      view.result.current.setVariableCacheValue(
        'node-llm.text',
        '手动调试缓存'
      );
    });

    expect(view.result.current.getNodePreviewVariableCache()).toEqual(
      expect.objectContaining({
        'node-llm': expect.objectContaining({
          text: '手动调试缓存'
        })
      })
    );
    expect(
      view.result.current.variableGroups
        .flatMap((group) => group.items)
        .find((item) => item.key === 'node-llm.text')?.value
    ).toBe('手动调试缓存');
    expect(upsertCacheSpy).toHaveBeenCalledWith(
      'app-1',
      {
        node_id: 'node-llm',
        variable_key: 'text',
        value: '手动调试缓存'
      },
      'csrf-123'
    );
    expect(window.localStorage.length).toBe(0);

    view.unmount();

    const utils = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    await waitFor(() => {
      expect(utils.result.current.getNodePreviewVariableCache()).toEqual(
        expect.objectContaining({
          'node-llm': expect.objectContaining({
            text: '手动调试缓存'
          })
        })
      );
    });
  });

  test('maps waiting_human runs to pending assistant state without fake output', async () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    vi.spyOn(runtimeApi, 'startFlowDebugRun').mockResolvedValue(
      createWaitingHumanRunDetail()
    );

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
      await result.current.submitPrompt('请人工审核退款申请');
    });

    await waitFor(() => {
      expect(result.current.status).toBe('waiting_human');
    });

    expect(result.current.messages).toHaveLength(2);
    expect(result.current.messages[1]).toEqual(
      expect.objectContaining({
        role: 'assistant',
        status: 'waiting_human',
        runId: 'flow-run-2',
        content: ''
      })
    );
    expect(result.current.messages[1]?.rawOutput).toBeNull();
    expect(result.current.traceItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-human',
          nodeAlias: '人工审核',
          status: 'waiting_human'
        })
      ])
    );
  });

  test('does not hydrate run context from local draft storage', () => {
    const queryClient = createQueryClient();
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    document.graph.nodes = document.graph.nodes.map((node) =>
      node.id === 'node-start'
        ? {
            ...node,
            config: {
              input_fields: [
                {
                  key: 'language',
                  label: '语言',
                  inputType: 'text',
                  valueType: 'string',
                  required: false
                },
                {
                  key: 'enable_search',
                  label: '启用搜索',
                  inputType: 'checkbox',
                  valueType: 'boolean',
                  required: false
                }
              ]
            }
          }
        : node
    );

    const { result } = renderHook(
      () =>
        useAgentFlowDebugSession({
          applicationId: 'app-1',
          draftId: 'draft-1',
          document
        }),
      { wrapper: createWrapper(queryClient) }
    );

    expect(result.current.runContext.remembered).toBe(false);
    expect(result.current.runContext.environmentLabel).toBe('draft');
    expect(result.current.runContext.fields).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          key: 'query',
          value: ''
        }),
        expect.objectContaining({
          key: 'language',
          value: 'Start language 调试值'
        }),
        expect.objectContaining({
          key: 'enable_search',
          value: true
        })
      ])
    );
  });
});
