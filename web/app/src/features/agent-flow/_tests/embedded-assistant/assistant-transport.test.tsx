import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeAll, beforeEach, describe, expect, test, vi } from 'vitest';

const {
  attachConsoleAssistantRunWebSocket,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantRunActivity,
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream,
  subscribeConsoleAssistantConversationsWebSocket,
  updateConsoleAssistantSettings
} = vi.hoisted(() => ({
  attachConsoleAssistantRunWebSocket: vi.fn(),
  cancelConsoleFlowRun: vi.fn(),
  createConsoleAssistantConversation: vi.fn(),
  getConsoleAssistantConversationMessages: vi.fn(),
  getConsoleAssistantLegacySnapshotMessages: vi.fn(),
  getConsoleAssistantRunActivity: vi.fn(),
  getConsoleAssistantSettings: vi.fn(),
  listConsoleAssistantConversations: vi.fn(),
  startConsoleAssistantRunWebSocket: vi.fn(),
  startConsoleAssistantRunStream: vi.fn(),
  subscribeConsoleAssistantConversationsWebSocket: vi.fn(),
  updateConsoleAssistantSettings: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();
  return {
    ...actual,
    attachConsoleAssistantRunWebSocket,
    cancelConsoleFlowRun,
    createConsoleAssistantConversation,
    getConsoleAssistantConversationMessages,
    getConsoleAssistantLegacySnapshotMessages,
    getConsoleAssistantRunActivity,
    getConsoleAssistantSettings,
    listConsoleAssistantConversations,
    startConsoleAssistantRunWebSocket,
    startConsoleAssistantRunStream,
    subscribeConsoleAssistantConversationsWebSocket,
    updateConsoleAssistantSettings
  };
});

vi.mock('@monaco-editor/react', () => ({
  default: ({ value }: { value: string }) => (
    <pre data-testid="mock-json-editor">{value}</pre>
  )
}));

vi.mock('../../../../shared/code-block/monaco-runtime', () => ({
  loadMonacoEditorModule: () => import('@monaco-editor/react')
}));

import { loadApplicationI18nResources } from '../../../../shared/i18n/app-i18n';
import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import * as runtimeApi from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

interface WindowDimensionSpy {
  mockRestore(): void;
  mockReturnValue(value: number): void;
}

const ASSISTANT_WINDOW_SIZE_STORAGE_KEY =
  '1flowbase.embedded_assistant.window_size';

describe('EmbeddedAgentAssistant', () => {
  beforeAll(() => loadApplicationI18nResources());
  let innerHeightSpy: WindowDimensionSpy | undefined;
  let innerWidthSpy: WindowDimensionSpy | undefined;

  beforeEach(() => {
    window.localStorage.removeItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY);
    innerHeightSpy = vi
      .spyOn(window, 'innerHeight', 'get')
      .mockReturnValue(900);
    innerWidthSpy = vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1280);
    vi.stubGlobal(
      'IntersectionObserver',
      class IntersectionObserver {
        disconnect() {}
        observe() {}
        takeRecords() {
          return [];
        }
        unobserve() {}
      }
    );
    resetAuthStore();
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-token',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'root',
        name: 'Root',
        nickname: 'Root',
        email: 'root@example.com',
        phone: null,
        avatar_url: null,
        introduction: '',
        effective_display_role: 'root',
        permissions: []
      }
    });
    getConsoleAssistantSettings.mockReset();
    getConsoleAssistantSettings.mockResolvedValue({
      preference: {
        application_id: 'flow-1',
        mcp_instance_ids: [],
        enabled_client_tools: ['get_client_context', 'refresh_client_view']
      },
      published_agent_flows: [
        { application_id: 'flow-1', name: 'Support Flow' }
      ],
      enabled_mcp_instances: [],
      page_reference_max_bytes: 65_536,
      page_reference_max_count: 5,
      page_reference_max_total_bytes: 65_536,
      run_capabilities: {
        model_selection_enabled: true,
        reasoning_effort_enabled: true,
        models: [
          {
            id: 'gpt-5.4',
            name: 'GPT-5.4',
            context_window: 100000,
            reasoning_efforts: ['low', 'high'],
            default_reasoning_effort: 'high'
          }
        ]
      }
    });
    updateConsoleAssistantSettings.mockReset();
    createConsoleAssistantConversation.mockReset();
    createConsoleAssistantConversation.mockResolvedValue({
      conversation_id: 'conversation-new',
      application_id: 'flow-1',
      created_at: '2026-08-07T00:00:00Z',
      updated_at: '2026-08-07T00:00:00Z'
    });
    getConsoleAssistantConversationMessages.mockReset();
    getConsoleAssistantConversationMessages.mockResolvedValue([]);
    getConsoleAssistantLegacySnapshotMessages.mockReset();
    getConsoleAssistantLegacySnapshotMessages.mockResolvedValue([
      {
        id: 'legacy-run:user',
        flow_run_id: 'legacy-run',
        role: 'user',
        content: 'Legacy question',
        created_at: '2026-08-06T00:00:00Z'
      }
    ]);
    getConsoleAssistantRunActivity.mockReset();
    getConsoleAssistantRunActivity.mockResolvedValue({
      status: 'running',
      started_at: '2026-08-16T00:00:00Z',
      finished_at: null,
      duration_ms: null,
      items: [],
      trace_events: [],
      has_more: false,
      next_sequence: null
    });
    listConsoleAssistantConversations.mockReset();
    listConsoleAssistantConversations.mockResolvedValue({
      items: [
        {
          conversation_id: 'conversation-1',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-1',
          latest_flow_run_status: 'succeeded',
          title: 'First conversation',
          created_at: '2026-08-07T00:00:00Z',
          updated_at: '2026-08-07T00:00:00Z'
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });
    cancelConsoleFlowRun.mockReset();
    cancelConsoleFlowRun.mockResolvedValue(undefined);
    attachConsoleAssistantRunWebSocket.mockReset();
    attachConsoleAssistantRunWebSocket.mockResolvedValue(undefined);
    startConsoleAssistantRunWebSocket.mockReset();
    startConsoleAssistantRunStream.mockReset();
    subscribeConsoleAssistantConversationsWebSocket.mockReset();
    subscribeConsoleAssistantConversationsWebSocket.mockImplementation(
      async (applicationId, _csrfToken, handlers) => {
        const controller = new AbortController();
        handlers.getAbortController?.(controller);
        handlers.onSnapshot(
          await listConsoleAssistantConversations(applicationId, {
            page: 1,
            pageSize: 20
          })
        );
      }
    );
  });

  afterEach(() => {
    window.localStorage.removeItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY);
    innerHeightSpy?.mockRestore();
    innerHeightSpy = undefined;
    innerWidthSpy?.mockRestore();
    innerWidthSpy = undefined;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  test('issue 1601 drives context from AI Gateway snapshots instead of Provider usage', async () => {
    vi.spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot').mockResolvedValue({
      flow_run: { status: 'succeeded' },
      node_runs: [],
      checkpoints: [],
      callback_tasks: [],
      events: [],
      context_snapshot: {
        type: 'context_snapshot',
        event_id: 'event-context-terminal',
        run_id: 'run-context',
        node_run_id: 'node-run-llm',
        node_id: 'node-llm',
        sequence: 9,
        input_tokens: 321,
        effective_context_window: 100000,
        remaining_tokens: 99679,
        measurement: {
          method: 'generic_estimate',
          accuracy: 'estimated',
          coverage: 'complete',
          unknown_block_count: 0
        },
        created_at: '2026-08-06T00:00:00Z'
      }
    } as never);
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-context',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'context_snapshot',
          run_id: 'run-context',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          input_tokens: 100,
          effective_context_window: 100000,
          remaining_tokens: 99900,
          measurement: {
            method: 'generic_estimate',
            accuracy: 'estimated',
            coverage: 'complete',
            unknown_block_count: 0
          }
        });
        handlers.onEvent({
          type: 'usage_snapshot',
          run_id: 'run-context',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          usage: {
            input_tokens: 80000,
            output_tokens: 50,
            total_tokens: 80050
          }
        });
        handlers.onEvent({
          type: 'node_finished',
          run_id: 'run-context',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          status: 'succeeded',
          metrics_payload: {
            usage: {
              input_tokens: 90000,
              output_tokens: 50,
              total_tokens: 90050
            }
          }
        });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-context',
          status: 'succeeded',
          output: { answer: 'Context updated' }
        });
      }
    );

    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    const initialContextProgress = document.querySelector(
      '.assistant-panel-plugin__context-progress'
    );
    expect(initialContextProgress).toBeInTheDocument();
    fireEvent.mouseEnter(initialContextProgress as HTMLElement);
    expect(await screen.findByText('剩余：100%')).toBeInTheDocument();
    expect(
      await screen.findByText('最大上下文 100K，已用 0')
    ).toBeInTheDocument();
    fireEvent.mouseLeave(initialContextProgress as HTMLElement);
    fireEvent.change(composer, { target: { value: 'Measure context' } });
    fireEvent.click(sendButton);

    const contextProgress = await waitFor(() => {
      const element = document.querySelector(
        '.assistant-panel-plugin__context-progress'
      );
      expect(element).toBeInTheDocument();
      return element as HTMLElement;
    });
    fireEvent.mouseEnter(contextProgress);

    expect(
      await screen.findByText('最大上下文 100K，已用 321')
    ).toBeInTheDocument();
    expect(await screen.findByText('剩余：99.7%')).toBeInTheDocument();
    expect(contextProgress.querySelector('.ant-progress')).toHaveAttribute(
      'aria-valuenow',
      '1'
    );
  });

  test('issue 1601 does not poll snapshots while WebSocket is healthy and calibrates once at terminal', async () => {
    const snapshot = vi
      .spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot')
      .mockRejectedValue(new Error('optional calibration unavailable'));
    let finishWebSocket: (() => void) | null = null;
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-live',
          status: 'queued'
        });
        await new Promise<void>((resolve) => {
          finishWebSocket = () => {
            handlers.onEvent({
              type: 'flow_finished',
              run_id: 'run-live',
              status: 'succeeded',
              output: { answer: 'done' }
            });
            resolve();
          };
        });
      }
    );

    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    fireEvent.change(composer, { target: { value: 'Keep streaming' } });
    fireEvent.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );
    expect(snapshot).not.toHaveBeenCalled();

    await act(async () => {
      finishWebSocket?.();
    });
    await waitFor(() =>
      expect(snapshot).toHaveBeenCalledWith('flow-1', 'run-live')
    );
    expect(snapshot).toHaveBeenCalledTimes(1);
  });

  test('issue 1601 falls back to Assistant SSE only before any WebSocket event', async () => {
    startConsoleAssistantRunWebSocket.mockRejectedValue(
      new Error('WebSocket unavailable')
    );
    startConsoleAssistantRunStream.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-fallback',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-fallback',
          status: 'succeeded',
          output: { answer: 'SSE fallback reply' }
        });
      }
    );

    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await waitFor(() =>
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1)
    );
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    fireEvent.change(composer, { target: { value: 'Fallback please' } });
    fireEvent.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunStream).toHaveBeenCalledWith(
        {
          application_id: 'flow-1',
          conversation_id: 'conversation-new',
          query: 'Fallback please'
        },
        'csrf-token',
        expect.any(Object)
      )
    );
  });

  test('issue 1601 stops a stalled handshake before a run id exists', async () => {
    const abort = vi.fn();
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) =>
        new Promise<void>((_resolve, reject) => {
          const abortController = new AbortController();
          abortController.signal.addEventListener('abort', () => {
            abort();
            reject(new DOMException('Aborted', 'AbortError'));
          });
          handlers.getAbortController?.(abortController);
        })
    );

    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    fireEvent.change(composer, { target: { value: 'Stall' } });
    fireEvent.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.terminate_debugging_run')
      })
    );

    await waitFor(() => expect(abort).toHaveBeenCalledTimes(1));
    expect(startConsoleAssistantRunStream).not.toHaveBeenCalled();
    await waitFor(() => expect(composer).toBeEnabled());
  });

  test('AC-001 closes only the transport and reattaches the accepted run when reopened', async () => {
    const abort = vi.fn();
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) =>
        new Promise<void>((_resolve, reject) => {
          const abortController = new AbortController();
          abortController.signal.addEventListener('abort', () => {
            abort();
            reject(new DOMException('Aborted', 'AbortError'));
          });
          handlers.getAbortController?.(abortController);
          handlers.onEvent({
            type: 'flow_accepted',
            run_id: 'run-close',
            status: 'queued'
          });
        })
    );

    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    fireEvent.change(composer, { target: { value: 'Close me' } });
    fireEvent.click(sendButton);
    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );

    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.close', {
          value1: i18nText('appShell', 'auto.assistant')
        })
      })
    );

    await waitFor(() => expect(abort).toHaveBeenCalledTimes(1));
    expect(cancelConsoleFlowRun).not.toHaveBeenCalled();
    expect(
      screen.queryByTestId('embedded-agent-assistant-preview')
    ).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await waitFor(() =>
      expect(attachConsoleAssistantRunWebSocket).toHaveBeenCalledWith(
        'flow-1',
        'run-close',
        'csrf-token',
        expect.any(Object),
        expect.objectContaining({ onControl: expect.any(Function) })
      )
    );
  });

});
