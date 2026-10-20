import { describe, test, expect, vi } from 'vitest';
import {
  answerReasoningDelta,
  answerTextDelta,
  createQueryClient,
  createStreamRunDetail,
  createWrapper,
  runtimeApi
} from './use-agent-flow-debug-session-stream.fixtures';
import { act, renderHook } from '@testing-library/react';
import {
  applyDebugStreamEventToAssistantMessage,
  applyDebugStreamEventToTrace,
  reconcileSnapshotTraceWithLiveEvents
} from '../../lib/debug-console/stream-events';
import { collectLlmToolCallbacks } from '../../components/debug-console/conversation/llm-tool-callbacks';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { useAgentFlowDebugSession } from '../../hooks/runtime/useAgentFlowDebugSession';

describe('useAgentFlowDebugSession streaming', () => {
  test('AC-004 retains a real-time tool lifecycle while its run snapshot is still behind', () => {
    const snapshotTrace = [
      {
        nodeRunId: 'node-run-llm',
        nodeId: 'node-llm',
        nodeAlias: 'LLM',
        nodeType: 'llm',
        status: 'running',
        startedAt: '2026-08-06T00:00:00Z',
        finishedAt: null,
        durationMs: null,
        inputPayload: {},
        outputPayload: {},
        errorPayload: null,
        metricsPayload: {},
        debugPayload: {}
      }
    ];
    const liveTrace = applyDebugStreamEventToTrace(snapshotTrace, {
      type: 'assistant_tool_call_started',
      node_run_id: 'node-run-llm',
      node_id: 'node-llm',
      tool_call: {
        id: 'call-weather',
        name: 'lookup_weather',
        arguments: { city: 'Shanghai' }
      }
    } as never);

    const reconciledTrace = reconcileSnapshotTraceWithLiveEvents(
      snapshotTrace,
      liveTrace
    );

    expect(
      collectLlmToolCallbacks(reconciledTrace[0]?.debugPayload)
    ).toMatchObject([
      {
        id: 'call-weather',
        name: 'lookup_weather',
        callbackStatus: 'waiting_callback'
      }
    ]);
  });

  test('AC-004 projects live assistant tool calls into the active LLM trace', () => {
    const initialTrace = [
      {
        nodeRunId: 'node-run-llm',
        nodeId: 'node-llm',
        nodeAlias: 'LLM',
        nodeType: 'llm',
        status: 'running',
        startedAt: '2026-08-06T00:00:00Z',
        finishedAt: null,
        durationMs: null,
        inputPayload: {},
        outputPayload: {},
        errorPayload: null,
        metricsPayload: {},
        debugPayload: {}
      }
    ];
    const startedTrace = applyDebugStreamEventToTrace(initialTrace, {
      type: 'assistant_tool_call_started',
      node_run_id: 'node-run-llm',
      node_id: 'node-llm',
      tool_call: {
        id: 'call-weather',
        name: 'lookup_weather',
        arguments: { city: 'Shanghai' }
      },
      call_usage: {
        input_tokens: 1200,
        output_tokens: 260,
        total_tokens: 1460
      }
    } as never);
    const completedTrace = applyDebugStreamEventToTrace(startedTrace, {
      type: 'assistant_tool_call_finished',
      node_run_id: 'node-run-llm',
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
    } as never);

    const assistantAfterToolStart = applyDebugStreamEventToAssistantMessage(
      {
        id: 'assistant-running',
        role: 'assistant',
        status: 'running',
        runId: 'run-1',
        content: '',
        rawOutput: null,
        traceSummary: []
      },
      {
        type: 'assistant_tool_call_started',
        node_run_id: 'node-run-llm',
        node_id: 'node-llm',
        tool_call: {
          id: 'call-weather',
          name: 'lookup_weather',
          arguments: { city: 'Shanghai' }
        }
      } as never,
      startedTrace
    );

    expect(
      collectLlmToolCallbacks(startedTrace[0]?.debugPayload)
    ).toMatchObject([
      {
        id: 'call-weather',
        name: 'lookup_weather',
        callbackStatus: 'waiting_callback',
        call_usage: {
          input_tokens: 1200,
          output_tokens: 260,
          total_tokens: 1460
        }
      }
    ]);
    expect(
      collectLlmToolCallbacks(completedTrace[0]?.debugPayload)
    ).toMatchObject([
      {
        id: 'call-weather',
        name: 'lookup_weather',
        callbackStatus: 'returned',
        executionStatus: 'succeeded',
        duration_ms: 42,
        parsedResult: {
          content: { temperature: 26 },
          is_error: false
        }
      }
    ]);
    expect(assistantAfterToolStart.traceSummary).toEqual(startedTrace);
  });

  test('AC-002 projects only canonical Answer deltas into assistant content', () => {
    const runningMessage = {
      id: 'assistant-running',
      role: 'assistant' as const,
      status: 'running' as const,
      runId: 'run-1',
      content: '',
      rawOutput: null,
      traceSummary: []
    };
    const providerProjected = applyDebugStreamEventToAssistantMessage(
      runningMessage,
      {
        type: 'text_delta',
        event_id: 'run-1:7',
        run_id: 'run-1',
        sequence: 7,
        node_run_id: 'node-run-llm',
        node_id: 'node-llm',
        text: '退款'
      },
      []
    );
    const answerProjected = applyDebugStreamEventToAssistantMessage(
      providerProjected,
      {
        type: 'text_delta',
        event_id: 'run-1:8',
        run_id: 'run-1',
        sequence: 8,
        node_run_id: null,
        node_id: 'node-answer',
        text: '退款',
        presentation: {
          kind: 'answer',
          answer_node_id: 'node-answer',
          segment_index: 0,
          source_node_id: 'node-llm',
          source_node_run_id: 'node-run-llm',
          source_output_key: 'text'
        }
      },
      []
    );
    const runningTrace = applyDebugStreamEventToTrace(
      [
        {
          nodeRunId: 'node-run-llm',
          nodeId: 'node-llm',
          nodeAlias: 'LLM',
          nodeType: 'llm',
          status: 'running',
          startedAt: '2026-07-31T00:00:00Z',
          finishedAt: null,
          durationMs: null,
          inputPayload: {},
          outputPayload: {},
          errorPayload: null,
          metricsPayload: {},
          debugPayload: {}
        }
      ],
      {
        type: 'text_delta',
        event_id: 'run-1:7',
        run_id: 'run-1',
        sequence: 7,
        node_run_id: 'node-run-llm',
        node_id: 'node-llm',
        text: '退款'
      }
    );
    const traceAfterPresentation = applyDebugStreamEventToTrace(runningTrace, {
      type: 'text_delta',
      event_id: 'run-1:8',
      run_id: 'run-1',
      sequence: 8,
      node_run_id: null,
      node_id: 'node-answer',
      text: '退款',
      presentation: {
        kind: 'answer',
        answer_node_id: 'node-answer',
        segment_index: 0,
        source_node_id: 'node-llm',
        source_node_run_id: 'node-run-llm',
        source_output_key: 'text'
      }
    });

    expect(providerProjected.content).toBe('');
    expect(answerProjected.content).toBe('退款');
    expect(runningTrace[0]?.debugPayload).toEqual({
      provider_events: [{ type: 'text_delta', text: '退款' }]
    });
    expect(traceAfterPresentation).toEqual(runningTrace);
  });

  test('handles flow accepted and batches text deltas without rebuilding variable cache per token', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      const requestAnimationFrameSpy = vi.spyOn(
        window,
        'requestAnimationFrame'
      );
      const startFlowDebugRunStreamSpy = vi
        .spyOn(runtimeApi, 'startFlowDebugRunStream')
        .mockImplementation(
          async (_applicationId, _input, _csrfToken, handlers) => {
            handlers.onEvent({
              type: 'flow_accepted',
              run_id: 'run-1',
              status: 'queued'
            });
            handlers.onEvent({
              type: 'flow_started',
              run_id: 'run-1',
              status: 'running'
            });
            handlers.onEvent(answerTextDelta('退'));
            handlers.onEvent(answerTextDelta('款'));
            handlers.onEvent({
              type: 'flow_finished',
              run_id: 'run-1',
              status: 'succeeded',
              output: { answer: '退款' }
            });
          }
        );
      vi.spyOn(
        runtimeApi,
        'fetchApplicationRunDebugSnapshot'
      ).mockResolvedValue(createStreamRunDetail('run-1', '退款'));
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
        const promise = result.current.submitPrompt('退款');
        await vi.runOnlyPendingTimersAsync();
        await promise;
      });

      expect(startFlowDebugRunStreamSpy).toHaveBeenCalled();
      expect(requestAnimationFrameSpy).toHaveBeenCalledTimes(1);
      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          runId: 'run-1',
          status: 'completed',
          content: '退款'
        })
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('streams reasoning and answer deltas into one Dify-style ordered content field', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(
        runtimeApi,
        'fetchApplicationRunDebugSnapshot'
      ).mockResolvedValue(createStreamRunDetail('run-envelope', '退款政策'));
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'run-reasoning',
            status: 'running'
          });
          handlers.onEvent(answerReasoningDelta('先分析'));
          handlers.onEvent(answerTextDelta('结果'));
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
        await result.current.submitPrompt('请分析退款政策');
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          runId: 'run-reasoning',
          status: 'running',
          content: '<think>先分析</think>结果'
        })
      );
      expect(result.current.messages.at(-1)).not.toHaveProperty(
        'reasoningContent'
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('deduplicates repeated envelope delta events by event_id', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'run-envelope',
            status: 'running',
            event_id: 'evt-run'
          });
          handlers.onEvent({
            ...answerTextDelta('退款'),
            run_id: 'run-envelope',
            sequence: 1
          });
          handlers.onEvent({
            ...answerTextDelta('退款'),
            run_id: 'run-envelope',
            sequence: 1,
            event_id: 'event-same-id'
          });
          handlers.onEvent({
            ...answerTextDelta('政策'),
            run_id: 'run-envelope',
            sequence: 2,
            event_id: 'event-same-id'
          });
          handlers.onEvent({
            type: 'flow_finished',
            run_id: 'run-envelope',
            status: 'succeeded',
            output: { answer: '退款政策' }
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
        await result.current.submitPrompt('退款');
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          content: '退款政策'
        })
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('ignores provider deltas that are not canonical Answer presentation', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'run-legacy',
            status: 'running'
          });
          handlers.onEvent({
            type: 'text_delta',
            node_id: 'node-llm',
            text: '只进入 trace'
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
        await result.current.submitPrompt('退款');
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.messages.at(-1)).toEqual(
        expect.objectContaining({
          content: ''
        })
      );
    } finally {
      vi.useRealTimers();
    }
  });

  test('does not overwrite trace rows when same node_id appears in different node_run_id', async () => {
    vi.useFakeTimers();

    try {
      const queryClient = createQueryClient();
      vi.spyOn(runtimeApi, 'startFlowDebugRunStream').mockImplementation(
        async (_applicationId, _input, _csrfToken, handlers) => {
          handlers.onEvent({
            type: 'flow_started',
            run_id: 'run-trace',
            status: 'running'
          });
          handlers.onEvent({
            type: 'node_started',
            node_run_id: 'node-run-1',
            node_id: 'node-llm',
            node_type: 'llm',
            title: 'LLM-1',
            input_payload: {}
          });
          handlers.onEvent({
            type: 'node_started',
            node_run_id: 'node-run-2',
            node_id: 'node-llm',
            node_type: 'llm',
            title: 'LLM-2',
            input_payload: {}
          });
          handlers.onEvent({
            type: 'node_finished',
            node_run_id: 'node-run-1',
            node_id: 'node-llm',
            status: 'succeeded',
            output_payload: {},
            error_payload: null,
            metrics_payload: {},
            started_at: '2026-01-01T00:00:00Z',
            finished_at: '2026-01-01T00:00:01Z'
          });
          handlers.onEvent({
            type: 'node_finished',
            node_run_id: 'node-run-2',
            node_id: 'node-llm',
            status: 'succeeded',
            output_payload: {},
            error_payload: null,
            metrics_payload: {},
            started_at: '2026-01-01T00:00:02Z',
            finished_at: '2026-01-01T00:00:03Z'
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
        await result.current.submitPrompt('退款');
        await vi.runOnlyPendingTimersAsync();
      });

      expect(result.current.traceItems).toEqual(
        expect.arrayContaining([
          expect.objectContaining({
            nodeRunId: 'node-run-1',
            nodeId: 'node-llm',
            status: 'succeeded'
          }),
          expect.objectContaining({
            nodeRunId: 'node-run-2',
            nodeId: 'node-llm',
            status: 'succeeded'
          })
        ])
      );
      expect(result.current.traceItems).toHaveLength(2);
    } finally {
      vi.useRealTimers();
    }
  });
});
