import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const {
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  startConsoleAssistantRunWebSocket,
  startConsoleAssistantRunStream,
  updateConsoleAssistantSettings
} = vi.hoisted(() => ({
  cancelConsoleFlowRun: vi.fn(),
  createConsoleAssistantConversation: vi.fn(),
  getConsoleAssistantConversationMessages: vi.fn(),
  getConsoleAssistantLegacySnapshotMessages: vi.fn(),
  getConsoleAssistantSettings: vi.fn(),
  listConsoleAssistantConversations: vi.fn(),
  startConsoleAssistantRunWebSocket: vi.fn(),
  startConsoleAssistantRunStream: vi.fn(),
  updateConsoleAssistantSettings: vi.fn()
}));

vi.mock('@1flowbase/api-client', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@1flowbase/api-client')>();
  return {
    ...actual,
    cancelConsoleFlowRun,
    createConsoleAssistantConversation,
    getConsoleAssistantConversationMessages,
    getConsoleAssistantLegacySnapshotMessages,
    getConsoleAssistantSettings,
    listConsoleAssistantConversations,
    startConsoleAssistantRunWebSocket,
    startConsoleAssistantRunStream,
    updateConsoleAssistantSettings
  };
});

import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import * as runtimeApi from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

interface WindowDimensionSpy {
  mockRestore(): void;
  mockReturnValue(value: number): void;
}

describe('EmbeddedAgentAssistant', () => {
  let innerHeightSpy: WindowDimensionSpy | undefined;
  let innerWidthSpy: WindowDimensionSpy | undefined;

  beforeEach(() => {
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
        mcp_instance_ids: []
      },
      published_agent_flows: [
        { application_id: 'flow-1', name: 'Support Flow' }
      ],
      enabled_mcp_instances: [],
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
    listConsoleAssistantConversations.mockReset();
    listConsoleAssistantConversations.mockResolvedValue({
      items: [
        {
          conversation_id: 'conversation-1',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-1',
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
    startConsoleAssistantRunWebSocket.mockReset();
    startConsoleAssistantRunStream.mockReset();
  });

  afterEach(() => {
    innerHeightSpy?.mockRestore();
    innerHeightSpy = undefined;
    innerWidthSpy?.mockRestore();
    innerWidthSpy = undefined;
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  test('AC-001 opens the Agent Flow Preview console instead of a hand-written drawer', async () => {
    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );

    const trigger = screen.getByRole('button', {
      name: i18nText('appShell', 'auto.assistant')
    });
    expect(trigger).toHaveTextContent('AI');

    await act(async () => {
      fireEvent.click(trigger);
    });

    await waitFor(() => {
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1);
    });
    expect(
      document.querySelector('.agent-flow-editor__debug-console')
    ).toBeInTheDocument();
    expect(document.querySelector('.ant-drawer')).not.toBeInTheDocument();
    expect(
      screen.getByTestId('embedded-agent-assistant-preview')
    ).toBeInTheDocument();
    expect(
      screen
        .getAllByRole('separator')
        .map((element) => element.getAttribute('aria-label'))
    ).toEqual(expect.arrayContaining([expect.any(String)]));
    expect(
      screen.getByRole('button', { name: /GPT-5\.4/u })
    ).toBeInTheDocument();

    const settings = screen.getByRole('button', {
      name: i18nText('appShell', 'auto.assistant_settings')
    });
    expect(settings).toHaveTextContent('');
    expect(settings.querySelector('.anticon-setting')).toBeInTheDocument();

    await waitFor(() => expect(settings).not.toBeDisabled());
    await act(async () => {
      fireEvent.click(settings);
    });
    expect(
      await screen.findByText(i18nText('appShell', 'auto.assistant_settings'))
    ).toBeInTheDocument();
    const settingsModalLayer = document.querySelector(
      '.ant-modal-wrap'
    ) as HTMLElement;
    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    expect(Number(settingsModalLayer.style.zIndex)).toBeGreaterThan(
      Number(assistantWindow.style.zIndex)
    );
  });

  test('AC-005 opens Conversations history, restores a selection, and creates a new conversation', async () => {
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

    const history = await screen.findByRole('button', {
      name: i18nText('appShell', 'auto.assistant_history')
    });
    fireEvent.click(history);
    expect(await screen.findByText('First conversation')).toBeInTheDocument();
    expect(listConsoleAssistantConversations).toHaveBeenCalledWith('flow-1', {
      page: 1,
      pageSize: 20
    });

    fireEvent.click(screen.getByText('First conversation'));
    await waitFor(() =>
      expect(getConsoleAssistantConversationMessages).toHaveBeenCalledWith(
        'flow-1',
        'conversation-1'
      )
    );

    fireEvent.click(history);
    fireEvent.click(
      screen.getByRole('button', {
        name: new RegExp(
          i18nText('appShell', 'auto.assistant_new_conversation'),
          'u'
        )
      })
    );
    await waitFor(() =>
      expect(createConsoleAssistantConversation).toHaveBeenCalledWith(
        { application_id: 'flow-1' },
        'csrf-token'
      )
    );
  });

  test('AC-005 keeps history in a collapsible left sidebar inside the assistant window', async () => {
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

    const history = await screen.findByRole('button', {
      name: i18nText('appShell', 'auto.assistant_history')
    });
    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    const initialLeft = Number.parseFloat(assistantWindow.style.left);
    const initialWidth = Number.parseFloat(assistantWindow.style.width);

    fireEvent.click(history);

    const historySidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    await waitFor(() =>
      expect(Number.parseFloat(assistantWindow.style.left)).toBeLessThan(
        initialLeft
      )
    );
    expect(Number.parseFloat(assistantWindow.style.width)).toBeGreaterThan(
      initialWidth
    );
    expect(historySidebar).toHaveAttribute(
      'aria-label',
      i18nText('appShell', 'auto.assistant_history')
    );
    expect(document.querySelector('.ant-drawer')).not.toBeInTheDocument();
    expect(
      historySidebar.compareDocumentPosition(
        document.querySelector('.agent-flow-editor__debug-console') as Node
      ) & Node.DOCUMENT_POSITION_FOLLOWING
    ).not.toBe(0);

    const resizeHandle = screen.getByTestId(
      'embedded-agent-assistant-history-resize'
    );
    fireEvent.mouseDown(resizeHandle, { clientX: 280 });
    fireEvent.mouseMove(window, { clientX: 340 });
    expect(historySidebar).toHaveStyle({ width: '340px' });
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(
      initialWidth + 340 + 12
    );
    fireEvent.mouseUp(window);
    expect(historySidebar).toHaveAttribute('data-resizing', 'false');

    fireEvent.click(history);
    await waitFor(() =>
      expect(
        screen.queryByTestId('embedded-agent-assistant-history')
      ).not.toBeInTheDocument()
    );
    expect(Number.parseFloat(assistantWindow.style.left)).toBe(initialLeft);
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(initialWidth);
  });

  test('AC-005 uses the assistant-owned history view without moving it on narrow screens', async () => {
    innerWidthSpy?.mockReturnValue(640);
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

    const history = await screen.findByRole('button', {
      name: i18nText('appShell', 'auto.assistant_history')
    });
    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    const initialLeft = assistantWindow.style.left;
    fireEvent.click(history);

    expect(
      await screen.findByTestId('embedded-agent-assistant-history')
    ).toBeInTheDocument();
    expect(document.querySelector('.ant-drawer')).not.toBeInTheDocument();
    expect(assistantWindow.style.left).toBe(initialLeft);
  });

  test('AC-004 explicitly continues a legacy snapshot without mutating it', async () => {
    listConsoleAssistantConversations.mockResolvedValueOnce({
      items: [
        {
          conversation_id: null,
          legacy_flow_run_id: 'legacy-run',
          latest_flow_run_id: 'legacy-run',
          title: 'Legacy snapshot',
          created_at: '2026-08-06T00:00:00Z',
          updated_at: '2026-08-06T00:00:00Z'
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });

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
    fireEvent.click(
      await screen.findByRole('button', {
        name: i18nText('appShell', 'auto.assistant_history')
      })
    );
    fireEvent.click(await screen.findByText('Legacy snapshot'));

    await waitFor(() =>
      expect(getConsoleAssistantLegacySnapshotMessages).toHaveBeenCalledWith(
        'flow-1',
        'legacy-run'
      )
    );

    fireEvent.click(
      await screen.findByRole('button', {
        name: i18nText('appShell', 'auto.assistant_continue_legacy_snapshot')
      })
    );

    await waitFor(() =>
      expect(createConsoleAssistantConversation).toHaveBeenCalledWith(
        {
          application_id: 'flow-1',
          seed_legacy_flow_run_id: 'legacy-run'
        },
        'csrf-token'
      )
    );
  });

  test('AC-006 prevents a live run from switching or clearing the current conversation', async () => {
    let finishWebSocket: (() => void) | null = null;
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-live-history',
          status: 'queued'
        });
        await new Promise<void>((resolve) => {
          finishWebSocket = () => {
            handlers.onEvent({
              type: 'flow_finished',
              run_id: 'run-live-history',
              status: 'succeeded',
              output: { answer: 'Finished without switching conversations.' }
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
    fireEvent.change(composer, { target: { value: 'Keep this conversation' } });
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.send_debug_message')
      })
    );
    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );
    createConsoleAssistantConversation.mockClear();

    expect(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.clear_preview')
      })
    ).toBeDisabled();
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_history')
      })
    );
    fireEvent.click(
      await screen.findByRole('button', {
        name: new RegExp(
          i18nText('appShell', 'auto.assistant_new_conversation'),
          'u'
        )
      })
    );
    expect(createConsoleAssistantConversation).not.toHaveBeenCalled();

    fireEvent.click(screen.getByText('First conversation'));
    expect(getConsoleAssistantConversationMessages).not.toHaveBeenCalled();

    await act(async () => {
      finishWebSocket?.();
    });
  });

  test('AC-005 toggles the AI preview and its trigger highlight together', async () => {
    render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );

    const trigger = screen.getByRole('button', {
      name: i18nText('appShell', 'auto.assistant')
    });
    expect(trigger).toHaveClass('app-shell-design-block');
    expect(trigger.closest('.app-shell-design-menu')).toBeInTheDocument();
    expect(trigger).toHaveAttribute('aria-pressed', 'false');

    await act(async () => {
      fireEvent.click(trigger);
    });

    expect(trigger).toHaveAttribute('aria-pressed', 'true');
    expect(trigger.closest('.embedded-agent-assistant-trigger')).toHaveClass(
      'ant-menu-item-selected'
    );

    await act(async () => {
      fireEvent.click(trigger);
    });

    await waitFor(() => {
      expect(trigger).toHaveAttribute('aria-pressed', 'false');
      expect(
        screen.queryByTestId('embedded-agent-assistant-preview')
      ).not.toBeInTheDocument();
    });
    expect(
      trigger.closest('.embedded-agent-assistant-trigger')
    ).not.toHaveClass('ant-menu-item-selected');
  });

  test('AC-004 projects primary Assistant WebSocket events through the Preview conversation', async () => {
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-1',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-1',
          node_id: 'node-answer',
          text: 'Assistant reply',
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'flow_incomplete',
          run_id: 'run-1',
          status: 'incomplete',
          reason: 'output_limit',
          output: { answer: 'Assistant reply' }
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

    await waitFor(() => {
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1);
    });

    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).not.toBeDisabled());
    fireEvent.change(composer, { target: { value: 'Summarize this' } });
    fireEvent.click(sendButton);

    await waitFor(() => {
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledWith(
        {
          application_id: 'flow-1',
          conversation_id: 'conversation-new',
          query: 'Summarize this',
          history: []
        },
        'csrf-token',
        expect.any(Object),
        expect.objectContaining({ onControl: expect.any(Function) })
      );
    });
    expect(startConsoleAssistantRunStream).not.toHaveBeenCalled();
    expect(await screen.findByText('Assistant reply')).toBeInTheDocument();
  });

  test('AC-006 loads a truncated workflow payload from the selected assistant flow', async () => {
    const loadArtifacts = vi
      .spyOn(runtimeApi, 'fetchRuntimeDebugArtifacts')
      .mockResolvedValue({
        artifacts: [
          {
            artifact_ref: 'artifact-input',
            content_type: 'application/json',
            value: { tools: [{ name: 'full tool definition' }] }
          }
        ]
      });
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-artifact',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'node_started',
          run_id: 'run-artifact',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          node_type: 'llm',
          title: 'LLM',
          input_payload: {
            tools: {
              __runtime_debug_artifact: true,
              is_truncated: true,
              original_size_bytes: 8192,
              preview_size_bytes: 256,
              content_type: 'application/json',
              artifact_ref: 'artifact-input',
              preview: '[{\"name\":\"preview tool\"}]'
            }
          }
        });
        handlers.onEvent({
          type: 'node_finished',
          run_id: 'run-artifact',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          status: 'succeeded',
          debug_payload: {
            llm_rounds: [
              {
                round_index: 0,
                assistant: {
                  tool_calls: [
                    { id: 'call-tools', name: 'list_available_tools' }
                  ]
                }
              }
            ]
          }
        });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-artifact',
          status: 'succeeded',
          output: { answer: 'Tool list ready' }
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
    await waitFor(() => expect(sendButton).not.toBeDisabled());
    fireEvent.change(composer, { target: { value: 'List available tools' } });
    fireEvent.click(sendButton);

    const inputPayload = await screen.findByRole('button', {
      name: i18nText('agentFlow', 'auto.input')
    });
    fireEvent.click(inputPayload);

    const loadFullValue = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.load_full_value')
    });
    expect(loadFullValue).toBeEnabled();
    fireEvent.click(loadFullValue);

    await waitFor(() =>
      expect(loadArtifacts).toHaveBeenCalledWith('flow-1', ['artifact-input'])
    );
    await waitFor(() =>
      expect(screen.getByLabelText('输入 JSON')).toHaveTextContent(
        'full tool definition'
      )
    );
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
    await waitFor(() => expect(sendButton).not.toBeDisabled());
    const initialContextProgress = document.querySelector(
      '.embedded-agent-assistant-preview__context-progress'
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
        '.embedded-agent-assistant-preview__context-progress'
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
    await waitFor(() => expect(sendButton).not.toBeDisabled());
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
    await waitFor(() => expect(sendButton).not.toBeDisabled());
    fireEvent.change(composer, { target: { value: 'Fallback please' } });
    fireEvent.click(sendButton);

    await waitFor(() =>
      expect(startConsoleAssistantRunStream).toHaveBeenCalledWith(
        {
          application_id: 'flow-1',
          conversation_id: 'conversation-new',
          query: 'Fallback please',
          history: []
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
    await waitFor(() => expect(sendButton).not.toBeDisabled());
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
    await waitFor(() => expect(composer).not.toBeDisabled());
  });

  test('issue 1601 closes the transport and cancels an accepted run', async () => {
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
    await waitFor(() => expect(sendButton).not.toBeDisabled());
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
    await waitFor(() =>
      expect(cancelConsoleFlowRun).toHaveBeenCalledWith(
        'flow-1',
        'run-close',
        'csrf-token'
      )
    );
    expect(
      screen.queryByTestId('embedded-agent-assistant-preview')
    ).not.toBeInTheDocument();
  });
});
