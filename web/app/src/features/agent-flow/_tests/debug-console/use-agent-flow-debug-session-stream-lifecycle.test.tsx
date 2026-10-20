import { describe, test, expect, vi } from 'vitest';
import {
  act,
  answerTextDelta,
  createDefaultAgentFlowDocument,
  createQueryClient,
  createStreamRunDetail,
  createWrapper,
  renderHook,
  runtimeApi,
  useAgentFlowDebugSession
} from './use-agent-flow-debug-session-stream.fixtures';

describe('useAgentFlowDebugSession streaming', () => {
  test('keeps stream error state when a pending text delta flush exists', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent(answerTextDelta('partial answer'));
          throw new Error('stream down');
        }
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
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.status).toBe('failed');
      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          role: 'assistant',
          runId: 'flow-run-stream',
          status: 'failed',
          content: 'stream down'
        })
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('keeps newer trace summary when a node event follows a pending text delta', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent(answerTextDelta('partial answer'));
          handlers.onEvent({
            type: 'node_started',
            node_run_id: 'node-run-llm',
            node_id: 'node-llm',
            node_type: 'llm',
            title: 'LLM',
            input_payload: {}
          });
        }
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
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          content: 'partial answer',
          traceSummary: [
            expect.objectContaining({
              nodeId: 'node-llm',
              nodeAlias: 'LLM',
              status: 'running'
            })
          ]
        })
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('does not restore a pending text delta after clearing the session', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent(answerTextDelta('partial answer'));
        }
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
        result.current.clearSession();
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.status).toBe('idle');
      expect(result.current.messages).toEqual([]);
    } finally {
      vi.useRealTimers();
    }
  });

  test('aborts and ignores active stream events after clearing the session', async () => {
    const queryClient = createQueryClient();
    const abortController = new AbortController();
    const abortSpy = vi.spyOn(abortController, 'abort');
    let handlers: runtimeApi.FlowDebugRunStreamHandlers | null = null;
    let resolveStream: (() => void) | null = null;
    const streamSettled = new Promise<void>((resolve) => {
      resolveStream = resolve;
    });

    vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
      async (_applicationId, _input, _csrfToken, nextHandlers) => {
        handlers = nextHandlers;
        nextHandlers.getAbortController?.(abortController);
        await streamSettled;
      }
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

    let submitPromise: Promise<unknown> | null = null;
    await act(async () => {
      submitPromise = result.current.submitPrompt('请总结退款政策');
      await Promise.resolve();
    });

    await act(async () => {
      result.current.clearSession();
    });

    expect(abortSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      handlers?.onEvent({
        type: 'flow_started',
        run_id: 'flow-run-stale',
        status: 'running'
      });
      resolveStream?.();
      await submitPromise;
    });

    expect(result.current.status).toBe('idle');
    expect(result.current.messages).toEqual([]);
  });

  test('does not return to running when resetting variables with a pending text delta', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent(answerTextDelta('partial answer'));
        }
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
        result.current.resetVariableCache();
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.status).toBe('idle');
    } finally {
      vi.useRealTimers();
    }
  });

  test('aborts and ignores active stream events after resetting variables', async () => {
    const queryClient = createQueryClient();
    const abortController = new AbortController();
    const abortSpy = vi.spyOn(abortController, 'abort');
    let handlers: runtimeApi.FlowDebugRunStreamHandlers | null = null;
    let resolveStream: (() => void) | null = null;
    const streamSettled = new Promise<void>((resolve) => {
      resolveStream = resolve;
    });

    vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
      async (_applicationId, _input, _csrfToken, nextHandlers) => {
        handlers = nextHandlers;
        nextHandlers.getAbortController?.(abortController);
        await streamSettled;
      }
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

    let submitPromise: Promise<unknown> | null = null;
    await act(async () => {
      submitPromise = result.current.submitPrompt('请总结退款政策');
      await Promise.resolve();
    });

    await act(async () => {
      result.current.resetVariableCache();
    });

    expect(abortSpy).toHaveBeenCalledTimes(1);

    await act(async () => {
      handlers?.onEvent({
        type: 'node_started',
        node_run_id: 'node-run-stale',
        node_id: 'node-llm',
        node_type: 'llm',
        title: 'LLM',
        input_payload: {}
      });
      resolveStream?.();
      await submitPromise;
    });

    expect(result.current.status).toBe('idle');
    expect(result.current.traceItems).toEqual([]);
  });

  test('submits even when crypto.randomUUID is unavailable', async () => {
    const descriptor = Object.getOwnPropertyDescriptor(crypto, 'randomUUID');
    Object.defineProperty(crypto, 'randomUUID', {
      configurable: true,
      value: undefined
    });
    const queryClient = createQueryClient();
    const startFlowDebugRunStreamSpy = vi
      .spyOn(runtimeApi, 'startFlowDebugRunStream')
      .mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent({
            type: 'flow_finished',
            run_id: 'flow-run-stream',
            status: 'succeeded',
            output: { answer: '你好' }
          });
        }
      );
    vi.spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot').mockResolvedValue(
      createStreamRunDetail('flow-run-stream', '你好')
    );
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });

    try {
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
        await result.current.submitPrompt('你好？');
      });

      expect(startFlowDebugRunStreamSpy).toHaveBeenCalled();
      expect(result.current.messages[0]).toEqual(
        expect.objectContaining({
          role: 'user',
          content: '你好？'
        })
      );
      expect(
        result.current.runContext.fields.find((field) => field.key === 'query')
          ?.value
      ).toBe('');
      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          role: 'assistant',
          content: '你好',
          status: 'completed'
        })
      );
    } finally {
      if (descriptor) {
        Object.defineProperty(crypto, 'randomUUID', descriptor);
      }
    }
  });

  test('streams debug run events into assistant content and trace state', async () => {
    const queryClient = createQueryClient();
    const invalidateQueriesSpy = vi.spyOn(queryClient, 'invalidateQueries');
    const startFlowDebugRunStreamSpy = vi
      .spyOn(runtimeApi, 'startFlowDebugRunStream')
      .mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent({
            type: 'node_started',
            node_run_id: 'node-run-start',
            node_id: 'node-start',
            node_type: 'start',
            title: 'Start',
            input_payload: {}
          });
          handlers.onEvent({
            type: 'node_finished',
            node_run_id: 'node-run-start',
            node_id: 'node-start',
            status: 'succeeded',
            output_payload: { query: '请总结退款政策' },
            error_payload: null,
            metrics_payload: {},
            started_at: '2026-04-25T10:00:00Z',
            finished_at: '2026-04-25T10:00:00Z'
          });
          handlers.onEvent({
            type: 'node_started',
            node_run_id: 'node-run-llm',
            node_id: 'node-llm',
            node_type: 'llm',
            title: 'LLM',
            input_payload: { user_prompt: '请总结退款政策' }
          });
          handlers.onEvent({
            type: 'text_delta',
            node_run_id: 'node-run-llm',
            node_id: 'node-llm',
            text: '退款'
          });
          handlers.onEvent(answerTextDelta('退款'));
          handlers.onEvent({
            type: 'text_delta',
            node_run_id: 'node-run-llm',
            node_id: 'node-llm',
            text: '政策摘要'
          });
          handlers.onEvent(answerTextDelta('政策摘要'));
          handlers.onEvent({
            type: 'node_finished',
            node_run_id: 'node-run-llm',
            node_id: 'node-llm',
            status: 'succeeded',
            output_payload: { text: '退款政策摘要' },
            error_payload: null,
            metrics_payload: { total_tokens: 128 },
            debug_payload: {
              assistant_message: {
                role: 'assistant',
                content: '退款政策摘要'
              },
              provider_route: {
                provider_code: 'openai_compatible'
              }
            },
            started_at: '2026-04-25T10:00:01Z',
            finished_at: '2026-04-25T10:00:02Z'
          });
          handlers.onEvent({
            type: 'flow_finished',
            run_id: 'flow-run-stream',
            status: 'succeeded',
            output: { answer: '退款政策摘要' }
          });
          handlers.onCompleted?.();
        }
      );
    const startFlowDebugRunSpy = vi.spyOn(runtimeApi, 'startFlowDebugRun');
    const fetchApplicationRunDebugSnapshotSpy = vi
      .spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot')
      .mockResolvedValue(createStreamRunDetail('flow-run-stream'));
    vi.spyOn(runtimeApi, 'fetchDebugVariableSnapshot')
      .mockResolvedValueOnce({ variable_cache: {} })
      .mockResolvedValue({
        variable_cache: {
          'node-start': {
            query: '请总结退款政策'
          },
          'node-llm': {
            text: '退款政策摘要'
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

    expect(startFlowDebugRunStreamSpy).toHaveBeenCalledWith(
      'app-1',
      {
        document,
        debug_session_id: expect.stringMatching(/^app-1:draft-1:/),
        mcp_instance_ids: [],
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
      'csrf-123',
      expect.objectContaining({
        onEvent: expect.any(Function)
      })
    );
    expect(startFlowDebugRunSpy).not.toHaveBeenCalled();
    expect(fetchApplicationRunDebugSnapshotSpy).toHaveBeenCalledWith(
      'app-1',
      'flow-run-stream'
    );
    expect(result.current.status).toBe('completed');
    expect(result.current.messages.at(-1)).toEqual(
      expect.objectContaining({
        role: 'assistant',
        runId: 'flow-run-stream',
        status: 'completed',
        content: '退款政策摘要'
      })
    );
    expect(result.current.traceItems).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          nodeId: 'node-llm',
          nodeAlias: 'LLM',
          status: 'succeeded'
        }),
        expect.objectContaining({
          nodeId: 'node-llm',
          inputPayload: {
            user_prompt: '请总结退款政策'
          },
          debugPayload: expect.objectContaining({
            provider_events: [
              {
                type: 'text_delta',
                text: '退款'
              },
              {
                type: 'text_delta',
                text: '政策摘要'
              }
            ],
            assistant_message: {
              role: 'assistant',
              content: '退款政策摘要'
            }
          })
        })
      ])
    );
    expect(result.current.getNodePreviewVariableCache()).toEqual(
      expect.objectContaining({
        'node-start': expect.objectContaining({
          query: '请总结退款政策'
        }),
        'node-llm': expect.objectContaining({
          text: '退款政策摘要'
        })
      })
    );
    expect(window.localStorage.length).toBe(0);
    expect(
      result.current.getNodePreviewVariableCache()['node-llm']
    ).not.toHaveProperty('user_prompt');
    expect(
      runtimeApi.buildNodeDebugPreviewPlan(
        document,
        'node-answer',
        result.current.getNodePreviewVariableCache()
      )
    ).toEqual({
      input_payload: {
        'node-llm': {
          text: '退款政策摘要'
        }
      },
      missing_fields: []
    });

    await act(async () => {
      await result.current.submitPrompt();
    });

    expect(startFlowDebugRunStreamSpy).toHaveBeenLastCalledWith(
      'app-1',
      {
        document,
        debug_session_id: expect.stringMatching(/^app-1:draft-1:/),
        mcp_instance_ids: [],
        input_payload: {
          'node-start': {
            files: [],
            history: [],
            max_output_tokens: null,
            model: '',
            protocol_context: null,
            query: '',
            reasoning_effort: '',
            system: '',
            tool_choice: {},
            tools: []
          }
        }
      },
      'csrf-123',
      expect.objectContaining({
        onEvent: expect.any(Function)
      })
    );
    expect(invalidateQueriesSpy).toHaveBeenCalledWith({
      queryKey: ['applications', 'app-1', 'runtime']
    });
  });

  test('marks streamed debug run cancelled after flow cancelled event', async () => {
    const queryClient = createQueryClient();
    const startFlowDebugRunStreamSpy = vi
      .spyOn(runtimeApi, 'startFlowDebugRunStream')
      .mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'flow-run-stream',
            status: 'running'
          });
          handlers.onEvent(answerTextDelta('partial answer'));
          handlers.onEvent({
            type: 'flow_cancelled',
            run_id: 'flow-run-stream',
            status: 'cancelled'
          });
          handlers.onCompleted?.();
        }
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

    expect(startFlowDebugRunStreamSpy).toHaveBeenCalled();
    expect(result.current.status).toBe('cancelled');
    expect(result.current.messages.at(-1)).toEqual(
      expect.objectContaining({
        role: 'assistant',
        runId: 'flow-run-stream',
        status: 'cancelled',
        content: 'partial answer'
      })
    );
  });

  test('marks streamed debug run waiting after explicit waiting event', async () => {
    const queryClient = createQueryClient();
    vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
      async (_applicationId, _input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_started',
          run_id: 'flow-run-waiting',
          status: 'running'
        });
        handlers.onEvent({
          type: 'waiting_human',
          run_id: 'flow-run-waiting',
          node_run_id: 'node-run-human',
          node_id: 'node-human',
          status: 'waiting_human',
          event_id: 'flow-run-waiting:2',
          sequence: 2
        });
        handlers.onCompleted?.();
      }
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
      await result.current.submitPrompt('请人工确认');
    });

    expect(result.current.status).toBe('waiting_human');
    expect(result.current.messages.at(-1)).toEqual(
      expect.objectContaining({
        role: 'assistant',
        runId: 'flow-run-waiting',
        status: 'waiting_human'
      })
    );
  });
});
