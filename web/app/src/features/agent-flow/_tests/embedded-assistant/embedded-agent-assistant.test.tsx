import {
  act,
  fireEvent,
  render,
  screen,
  within,
  waitFor
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

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

import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import { AssistantRunTimeline } from '../../components/embedded-assistant/AssistantRunActivityPanel';
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

    fireEvent.click(trigger);

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

    await waitFor(() => expect(settings).toBeEnabled());
    fireEvent.click(settings);
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
    expect(
      screen.getByRole('checkbox', {
        name: i18nText('appShell', 'auto.assistant_client_context_tool')
      })
    ).toBeChecked();
    expect(
      screen.getByRole('checkbox', {
        name: i18nText('appShell', 'auto.assistant_client_refresh_tool')
      })
    ).toBeChecked();
  });

  test('AC-005 opens Conversations history, restores a selection, and creates a new conversation', async () => {
    getConsoleAssistantConversationMessages.mockResolvedValueOnce([
      {
        id: 'run-1:user',
        flow_run_id: 'run-1',
        role: 'user',
        content: 'Why did this fail?',
        page_references: [
          {
            page_url: 'http://console.test/logs',
            page_title: 'Logs',
            outer_html: '<div id="failed-run">Failed</div>'
          }
        ],
        created_at: '2026-08-07T00:00:00Z'
      }
    ]);
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
    expect(
      subscribeConsoleAssistantConversationsWebSocket
    ).toHaveBeenCalledWith(
      'flow-1',
      'csrf-token',
      expect.objectContaining({
        getAbortController: expect.any(Function),
        onConversation: expect.any(Function),
        onSnapshot: expect.any(Function)
      })
    );

    fireEvent.click(await screen.findByText('First conversation'));
    await waitFor(() =>
      expect(getConsoleAssistantConversationMessages).toHaveBeenCalledWith(
        'flow-1',
        'conversation-1'
      )
    );
    expect(
      screen.getByTestId('embedded-agent-assistant-preview')
    ).toHaveTextContent('div#failed-run');

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

  test('AC-005 restores a cancelled conversation with its backend partial answer and status', async () => {
    listConsoleAssistantConversations.mockResolvedValueOnce({
      items: [
        {
          conversation_id: 'conversation-cancelled',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-cancelled',
          latest_flow_run_status: 'cancelled',
          title: 'Cancelled conversation',
          created_at: '2026-08-07T00:00:00Z',
          updated_at: '2026-08-07T00:00:00Z'
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });
    getConsoleAssistantConversationMessages.mockResolvedValueOnce([
      {
        id: 'run-cancelled:user',
        flow_run_id: 'run-cancelled',
        role: 'user',
        content: 'Summarize this',
        status: 'cancelled',
        page_references: [],
        created_at: '2026-08-07T00:00:00Z'
      },
      {
        id: 'run-cancelled:assistant',
        flow_run_id: 'run-cancelled',
        role: 'assistant',
        content: 'Public partial answer',
        status: 'cancelled',
        page_references: [],
        created_at: '2026-08-07T00:00:01Z'
      }
    ]);

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

    expect(
      await screen.findByLabelText(
        i18nText('appShell', 'auto.assistant_status_cancelled')
      )
    ).toHaveAttribute('data-assistant-run-status', 'cancelled');
    fireEvent.click(await screen.findByText('Cancelled conversation'));

    expect(
      await screen.findByText('Public partial answer')
    ).toBeInTheDocument();
    expect(
      screen.queryByText(i18nText('agentFlow', 'auto.stopped'))
    ).not.toBeInTheDocument();
  });

  test('AC-005 shows cancelled-without-output only when history has no assistant partial', async () => {
    listConsoleAssistantConversations.mockResolvedValueOnce({
      items: [
        {
          conversation_id: 'conversation-cancelled-empty',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-cancelled-empty',
          latest_flow_run_status: 'cancelled',
          title: 'Cancelled without output',
          created_at: '2026-08-07T00:00:00Z',
          updated_at: '2026-08-07T00:00:00Z'
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });
    getConsoleAssistantConversationMessages.mockResolvedValueOnce([
      {
        id: 'run-cancelled-empty:user',
        flow_run_id: 'run-cancelled-empty',
        role: 'user',
        content: 'Stop before answering',
        status: 'cancelled',
        page_references: [],
        created_at: '2026-08-07T00:00:00Z'
      }
    ]);

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
    fireEvent.click(await screen.findByText('Cancelled without output'));

    expect(
      await screen.findByText(i18nText('agentFlow', 'auto.stopped'))
    ).toBeInTheDocument();
  });

  test('AC-003 restores and attaches a historical conversation whose latest run is active', async () => {
    listConsoleAssistantConversations.mockResolvedValueOnce({
      items: [
        {
          conversation_id: 'conversation-active',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-active',
          latest_flow_run_status: 'running',
          title: 'Active conversation',
          created_at: '2026-08-07T00:00:00Z',
          updated_at: '2026-08-07T00:00:00Z'
        }
      ],
      total: 1,
      page: 1,
      page_size: 20
    });
    getConsoleAssistantConversationMessages.mockResolvedValueOnce([
      {
        id: 'run-active:user',
        flow_run_id: 'run-active',
        role: 'user',
        content: 'Continue in the background',
        page_references: [],
        created_at: '2026-08-07T00:00:00Z'
      }
    ]);

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
    fireEvent.click(await screen.findByText('Active conversation'));

    await waitFor(() =>
      expect(attachConsoleAssistantRunWebSocket).toHaveBeenCalledWith(
        'flow-1',
        'run-active',
        'csrf-token',
        expect.any(Object),
        expect.objectContaining({ onControl: expect.any(Function) })
      )
    );
    expect(
      await screen.findByText('Continue in the background')
    ).toBeInTheDocument();
  });

  test('AC-001 shows conversation-scoped run status in the history sidebar', async () => {
    listConsoleAssistantConversations.mockResolvedValueOnce({
      items: [
        {
          conversation_id: 'conversation-running',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-running',
          latest_flow_run_status: 'running',
          title: 'Running conversation',
          created_at: '2026-08-15T00:00:00Z',
          updated_at: '2026-08-15T00:00:00Z'
        },
        {
          conversation_id: 'conversation-waiting',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-waiting',
          latest_flow_run_status: 'waiting_human',
          title: 'Waiting conversation',
          created_at: '2026-08-15T00:00:00Z',
          updated_at: '2026-08-15T00:00:00Z'
        },
        {
          conversation_id: 'conversation-failed',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-failed',
          latest_flow_run_status: 'failed',
          title: 'Failed conversation',
          created_at: '2026-08-15T00:00:00Z',
          updated_at: '2026-08-15T00:00:00Z'
        },
        {
          conversation_id: 'conversation-completed',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-completed',
          latest_flow_run_status: 'succeeded',
          title: 'Completed conversation',
          created_at: '2026-08-15T00:00:00Z',
          updated_at: '2026-08-15T00:00:00Z'
        }
      ],
      total: 4,
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

    expect(await screen.findByLabelText('运行中')).toBeInTheDocument();
    expect(screen.getByLabelText('等待操作')).toBeInTheDocument();
    expect(screen.getByLabelText('运行失败')).toBeInTheDocument();
    expect(
      screen
        .getByText('Completed conversation')
        .closest('.ant-conversations-item')
        ?.querySelector('[data-assistant-run-status]')
    ).toBeNull();
  });

  test('AC-002 follows application conversation events and aborts on history close', async () => {
    const controller = new AbortController();
    let emitConversation:
      | ((
          item: {
            conversation_id: string;
            legacy_flow_run_id: null;
            latest_flow_run_id: string;
            latest_flow_run_status: string;
            title: string;
            created_at: string;
            updated_at: string;
          },
          eventType: 'conversation.created' | 'conversation.updated'
        ) => void)
      | undefined;
    subscribeConsoleAssistantConversationsWebSocket.mockImplementation(
      async (_applicationId, _csrfToken, handlers) => {
        handlers.getAbortController?.(controller);
        handlers.onSnapshot({
          items: [],
          total: 0,
          page: 1,
          page_size: 20
        });
        emitConversation = handlers.onConversation;
      }
    );
    const setIntervalSpy = vi.spyOn(window, 'setInterval');

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

    await waitFor(() => expect(emitConversation).toBeDefined());
    await act(async () =>
      emitConversation?.(
        {
          conversation_id: 'conversation-background',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-background',
          latest_flow_run_status: 'running',
          title: 'Server generated title',
          created_at: '2026-08-15T00:00:00Z',
          updated_at: '2026-08-15T00:00:01Z'
        },
        'conversation.created'
      )
    );
    expect(
      await screen.findByText('Server generated title')
    ).toBeInTheDocument();
    expect(screen.getByLabelText('运行中')).toBeInTheDocument();
    expect(attachConsoleAssistantRunWebSocket).not.toHaveBeenCalled();
    expect(setIntervalSpy).not.toHaveBeenCalledWith(
      expect.any(Function),
      3_000
    );

    await act(async () =>
      emitConversation?.(
        {
          conversation_id: 'conversation-background',
          legacy_flow_run_id: null,
          latest_flow_run_id: 'run-background',
          latest_flow_run_status: 'succeeded',
          title: 'Final server title',
          created_at: '2026-08-15T00:00:00Z',
          updated_at: '2026-08-15T00:00:02Z'
        },
        'conversation.updated'
      )
    );
    await waitFor(() =>
      expect(screen.queryByLabelText('运行中')).not.toBeInTheDocument()
    );
    expect(screen.getByText('Final server title')).toBeInTheDocument();
    expect(controller.signal.aborted).toBe(false);
    fireEvent.click(history);
    expect(controller.signal.aborted).toBe(true);
  });

  test('AC-005 assigns each horizontal resize edge to its nearest pane', async () => {
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
    const expandedLeft = Number.parseFloat(assistantWindow.style.left);
    const expandedWidth = Number.parseFloat(assistantWindow.style.width);

    const resizeHandle = screen.getByTestId(
      'embedded-agent-assistant-history-resize'
    );
    fireEvent.mouseDown(resizeHandle, { clientX: 280 });
    fireEvent.mouseMove(window, { clientX: 340 });
    expect(historySidebar).toHaveStyle({ width: '340px' });
    expect(Number.parseFloat(assistantWindow.style.left)).toBe(expandedLeft);
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(expandedWidth);
    fireEvent.mouseUp(window);
    expect(historySidebar).toHaveAttribute('data-resizing', 'false');

    const conversationWidth =
      Number.parseFloat(assistantWindow.style.width) - 340 - 12;
    const leftResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} left`
    });
    fireEvent.mouseDown(leftResizeHandle, { clientX: expandedLeft });
    fireEvent.mouseMove(window, { clientX: expandedLeft - 60 });
    expect(historySidebar).toHaveStyle({ width: '400px' });
    expect(Number.parseFloat(assistantWindow.style.left)).toBe(
      expandedLeft - 60
    );
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(
      expandedWidth + 60
    );
    expect(Number.parseFloat(assistantWindow.style.width) - 400 - 12).toBe(
      conversationWidth
    );
    fireEvent.mouseUp(window);

    innerWidthSpy?.mockReturnValue(2_000);
    const rightResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} right`
    });
    const rightEdge = expandedLeft + expandedWidth;
    fireEvent.mouseDown(rightResizeHandle, { clientX: rightEdge });
    fireEvent.mouseMove(window, { clientX: rightEdge + 60 });
    expect(historySidebar).toHaveStyle({ width: '400px' });
    expect(Number.parseFloat(assistantWindow.style.left)).toBe(
      expandedLeft - 60
    );
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(
      expandedWidth + 120
    );
    expect(Number.parseFloat(assistantWindow.style.width) - 400 - 12).toBe(
      conversationWidth + 60
    );
    fireEvent.mouseUp(window);

    fireEvent.click(history);
    await waitFor(() =>
      expect(
        screen.queryByTestId('embedded-agent-assistant-history')
      ).not.toBeInTheDocument()
    );
    expect(Number.parseFloat(assistantWindow.style.left)).toBe(initialLeft);
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(
      initialWidth + 60
    );
  });

  test('caches only the main conversation width and assistant window height', async () => {
    innerWidthSpy?.mockReturnValue(2_000);
    const firstRender = render(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );

    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    const initialWidth = Number.parseFloat(assistantWindow.style.width);
    const initialHeight = Number.parseFloat(assistantWindow.style.height);
    const initialRight =
      Number.parseFloat(assistantWindow.style.left) + initialWidth;
    const initialBottom =
      Number.parseFloat(assistantWindow.style.top) + initialHeight;
    const rightResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} right`
    });
    fireEvent.mouseDown(rightResizeHandle, { clientX: initialRight });
    fireEvent.mouseMove(window, { clientX: initialRight + 80 });
    fireEvent.mouseUp(window);
    const bottomResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} bottom`
    });
    fireEvent.mouseDown(bottomResizeHandle, { clientY: initialBottom });
    fireEvent.mouseMove(window, { clientY: initialBottom - 100 });
    fireEvent.mouseUp(window);

    expect(
      JSON.parse(
        window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
      )
    ).toEqual({
      conversationWidth: initialWidth + 80,
      windowHeight: initialHeight - 100
    });

    const history = await screen.findByRole('button', {
      name: i18nText('appShell', 'auto.assistant_history')
    });
    fireEvent.click(history);
    const historySidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    const expandedLeft = Number.parseFloat(assistantWindow.style.left);
    const leftResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} left`
    });
    fireEvent.mouseDown(leftResizeHandle, { clientX: expandedLeft });
    fireEvent.mouseMove(window, { clientX: expandedLeft - 60 });
    fireEvent.mouseUp(window);
    expect(historySidebar).toHaveStyle({ width: '340px' });
    expect(
      JSON.parse(
        window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
      )
    ).toEqual({
      conversationWidth: initialWidth + 80,
      windowHeight: initialHeight - 100
    });

    const divider = screen.getByTestId(
      'embedded-agent-assistant-history-resize'
    );
    fireEvent.mouseDown(divider, { clientX: 340 });
    fireEvent.mouseMove(window, { clientX: 380 });
    fireEvent.mouseUp(window);
    expect(historySidebar).toHaveStyle({ width: '380px' });
    expect(
      JSON.parse(
        window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
      )
    ).toEqual({
      conversationWidth: initialWidth + 40,
      windowHeight: initialHeight - 100
    });

    firstRender.unmount();
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
    const restoredWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    expect(Number.parseFloat(restoredWindow.style.width)).toBe(
      initialWidth + 40
    );
    expect(Number.parseFloat(restoredWindow.style.height)).toBe(
      initialHeight - 100
    );
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

  test('AC-005 switches an open history sidebar to the full assistant view after a narrow resize', async () => {
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
    await screen.findByTestId('embedded-agent-assistant-history');

    innerWidthSpy?.mockReturnValue(640);
    fireEvent(window, new Event('resize'));

    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    );
    await waitFor(() =>
      expect(
        assistantWindow.querySelector(
          '.embedded-agent-assistant-preview__layout'
        )
      ).toHaveAttribute('data-history-full', 'true')
    );
    expect(
      assistantWindow.querySelector(
        '.embedded-agent-assistant-preview__conversation'
      )
    ).toHaveAttribute('hidden');
  });

  test('AC-004 explicitly continues a legacy snapshot without mutating it', async () => {
    listConsoleAssistantConversations.mockResolvedValueOnce({
      items: [
        {
          conversation_id: null,
          legacy_flow_run_id: 'legacy-run',
          latest_flow_run_id: 'legacy-run',
          latest_flow_run_status: 'succeeded',
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

  test('AC-006 keeps a live run in the background while another conversation is selected', async () => {
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
    fireEvent.click(await screen.findByText('First conversation'));
    await waitFor(() =>
      expect(getConsoleAssistantConversationMessages).toHaveBeenCalledWith(
        'flow-1',
        'conversation-1'
      )
    );
    expect(cancelConsoleFlowRun).not.toHaveBeenCalled();

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

    fireEvent.click(trigger);

    expect(trigger).toHaveAttribute('aria-pressed', 'true');
    expect(trigger.closest('.embedded-agent-assistant-trigger')).toHaveClass(
      'ant-menu-item-selected'
    );

    fireEvent.click(trigger);

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
    await waitFor(() => expect(sendButton).toBeEnabled());
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

  test('AC-002 AC-003 keeps the streamed public partial answer when the run is cancelled', async () => {
    vi.spyOn(runtimeApi, 'fetchApplicationRunDebugSnapshot').mockRejectedValue(
      new Error('terminal snapshot is intentionally unavailable')
    );
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-cancelled-live',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'reasoning_delta',
          run_id: 'run-cancelled-live',
          node_id: 'node-answer',
          text: 'Temporary reasoning',
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-cancelled-live',
          node_id: 'node-answer',
          text: 'Public partial answer',
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'flow_cancelled',
          run_id: 'run-cancelled-live',
          status: 'cancelled'
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
    fireEvent.change(composer, { target: { value: 'Start then cancel' } });
    fireEvent.click(sendButton);

    expect(
      await screen.findByText('Public partial answer')
    ).toBeInTheDocument();
    expect(screen.queryByText('Temporary reasoning')).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByText(
        `${i18nText('appShell', 'auto.assistant_activity_duration_unknown')} · ${i18nText('appShell', 'auto.assistant_status_cancelled')}`
      )
    );
    expect(await screen.findByText('Temporary reasoning')).toBeInTheDocument();
    expect(
      screen.queryByText(i18nText('agentFlow', 'auto.stopped'))
    ).not.toBeInTheDocument();
    await waitFor(() => expect(composer).toBeEnabled());
  });

  test('AC-006 keeps a formal terminal error outside the process when failure has no output', async () => {
    getConsoleAssistantRunActivity.mockResolvedValue({
      status: 'failed',
      started_at: '2026-08-16T00:00:00Z',
      finished_at: '2026-08-16T00:00:01Z',
      duration_ms: 1000,
      items: [
        {
          kind: 'reasoning',
          event_id: 'run-failed:1',
          sequence: 1,
          created_at: '2026-08-16T00:00:00Z',
          text: '失败前思考'
        },
        {
          kind: 'error',
          event_id: 'run-failed:2',
          sequence: 2,
          created_at: '2026-08-16T00:00:01Z',
          error: 'Provider rejected the request'
        }
      ],
      trace_events: [],
      has_more: false,
      next_sequence: null
    });

    render(
      <AppProviders>
        <AssistantRunTimeline
          applicationId="flow-1"
          message={{
            id: 'run-failed:assistant',
            role: 'assistant',
            content: '',
            status: 'failed',
            runId: 'run-failed',
            rawOutput: null,
            traceSummary: [],
            presentation: 'answer'
          }}
        />
      </AppProviders>
    );

    expect(
      await screen.findByText('Provider rejected the request')
    ).toBeInTheDocument();
    expect(screen.queryByText('失败前思考')).not.toBeInTheDocument();
  });

  test('AC-006 keeps ordered assistant activity inline and restores every node card in the sidebar', async () => {
    let finishRun: (() => void) | undefined;
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-activity',
          status: 'queued',
          event_id: 'run-activity:1',
          sequence: 1
        });
        handlers.onEvent({
          type: 'node_started',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          node_type: 'llm',
          title: 'Research node',
          event_id: 'run-activity:2',
          sequence: 2
        });
        handlers.onEvent({
          type: 'reasoning_delta',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          text: '先检查配置',
          event_id: 'run-activity:3',
          sequence: 3,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'assistant_tool_call_started',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-1',
            name: '1flowbase_mcp_list',
            arguments: { path: '/后台设置' }
          },
          event_id: 'run-activity:4',
          sequence: 4
        });
        handlers.onEvent({
          type: 'assistant_tool_call_finished',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-1',
            name: '1flowbase_mcp_list',
            arguments: { path: '/后台设置' }
          },
          tool_result: { count: 2 },
          duration_ms: 4,
          event_id: 'run-activity:5',
          sequence: 5
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-activity',
          node_id: 'node-answer',
          text: '阶段结果一',
          event_id: 'run-activity:6',
          sequence: 6,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'reasoning_delta',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          text: '继续检查数据',
          event_id: 'run-activity:7',
          sequence: 7,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 1
          }
        });
        handlers.onEvent({
          type: 'assistant_tool_call_started',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-2',
            name: '1flowbase_mcp_get',
            arguments: { group_id: 'group-123' }
          },
          event_id: 'run-activity:8',
          sequence: 8
        });
        handlers.onEvent({
          type: 'assistant_tool_call_finished',
          run_id: 'run-activity',
          node_run_id: 'node-run-llm',
          node_id: 'node-llm',
          tool_call: {
            id: 'call-2',
            name: '1flowbase_mcp_get',
            arguments: { group_id: 'group-123' }
          },
          tool_result: { id: 'item-1' },
          duration_ms: 3,
          event_id: 'run-activity:9',
          sequence: 9
        });
        handlers.onEvent({
          type: 'text_delta',
          run_id: 'run-activity',
          node_id: 'node-answer',
          text: '阶段结果二',
          event_id: 'run-activity:10',
          sequence: 10,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 1
          }
        });
        await new Promise<void>((resolve) => {
          finishRun = () => {
            handlers.onEvent({
              type: 'node_finished',
              run_id: 'run-activity',
              node_run_id: 'node-run-llm',
              node_id: 'node-llm',
              status: 'succeeded',
              event_id: 'run-activity:11',
              sequence: 11
            });
            handlers.onEvent({
              type: 'node_started',
              run_id: 'run-activity',
              node_run_id: 'node-run-answer',
              node_id: 'node-answer',
              node_type: 'answer',
              title: 'Answer node',
              event_id: 'run-activity:12',
              sequence: 12
            });
            handlers.onEvent({
              type: 'node_finished',
              run_id: 'run-activity',
              node_run_id: 'node-run-answer',
              node_id: 'node-answer',
              status: 'succeeded',
              event_id: 'run-activity:13',
              sequence: 13
            });
            handlers.onEvent({
              type: 'flow_finished',
              run_id: 'run-activity',
              status: 'succeeded',
              output: { answer: '阶段结果一阶段结果二' },
              event_id: 'run-activity:14',
              sequence: 14
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
    fireEvent.change(composer, { target: { value: 'Inspect activity order' } });
    fireEvent.click(sendButton);

    const firstReasoning = await screen.findByText('先检查配置');
    const firstTool = screen.getByText('1flowbase_mcp_list (/后台设置)');
    const firstOutput = screen.getByText('阶段结果一');
    const secondReasoning = screen.getByText('继续检查数据');
    const secondTool = screen.getByText('1flowbase_mcp_get (group-123)');
    const secondOutput = screen.getByText('阶段结果二');
    expect(firstReasoning.compareDocumentPosition(firstTool)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(firstTool.compareDocumentPosition(firstOutput)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(firstOutput.compareDocumentPosition(secondReasoning)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(secondReasoning.compareDocumentPosition(secondTool)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(secondTool.compareDocumentPosition(secondOutput)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    fireEvent.click(firstTool);
    expect(
      screen.getByText(i18nText('agentFlow', 'auto.input'))
    ).toBeInTheDocument();
    expect(screen.getByText(/"path": "\/后台设置"/)).toBeInTheDocument();
    expect(
      screen.queryAllByText(i18nText('agentFlow', 'auto.think'))
    ).toHaveLength(2);

    await act(async () => {
      finishRun?.();
    });
    await waitFor(() =>
      expect(screen.queryByText('先检查配置')).not.toBeInTheDocument()
    );
    expect(screen.getByText('阶段结果二')).toBeInTheDocument();
    const duration = screen.getByText(
      i18nText('appShell', 'auto.assistant_activity_duration_unknown')
    );
    fireEvent.click(duration);
    expect(await screen.findByText('先检查配置')).toBeInTheDocument();
    expect(screen.getByText('阶段结果一')).toBeInTheDocument();
    expect(
      screen.queryByLabelText(i18nText('agentFlow', 'auto.workflow'))
    ).not.toBeInTheDocument();

    const activityButtons = screen.getAllByRole('button', {
      name: i18nText('appShell', 'auto.assistant_activity')
    });
    fireEvent.click(activityButtons.at(-1) as HTMLElement);

    const sidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    expect(sidebar).toHaveAttribute(
      'aria-label',
      i18nText('appShell', 'auto.assistant_activity')
    );
    expect(within(sidebar).getByText('Research node')).toBeInTheDocument();
    expect(
      within(sidebar).getByText(i18nText('agentFlow', 'auto.reply_directly'))
    ).toBeInTheDocument();
    expect(
      within(sidebar).getByLabelText(i18nText('agentFlow', 'auto.workflow'))
    ).toBeInTheDocument();
    expect(within(sidebar).queryByText('先检查配置')).not.toBeInTheDocument();
    expect(within(sidebar).queryByText('继续检查数据')).not.toBeInTheDocument();
  });

  test('AC-007 replays the same run activity projection for restored history', async () => {
    getConsoleAssistantConversationMessages.mockResolvedValueOnce([
      {
        id: 'run-history:assistant',
        flow_run_id: 'run-history',
        role: 'assistant',
        content: '历史最终回答',
        status: 'succeeded',
        page_references: [],
        created_at: '2026-08-15T00:00:00Z'
      }
    ]);
    getConsoleAssistantRunActivity.mockResolvedValue({
      status: 'succeeded',
      started_at: '2026-08-15T00:00:00Z',
      finished_at: '2026-08-15T00:00:03Z',
      duration_ms: 3000,
      items: [
        {
          kind: 'reasoning',
          event_id: 'run-history:3',
          sequence: 3,
          created_at: '2026-08-15T00:00:01Z',
          text: '历史思考'
        },
        {
          kind: 'output',
          event_id: 'run-history:4',
          sequence: 4,
          created_at: '2026-08-15T00:00:02Z',
          text: '历史最终回答',
          segment_index: 0
        }
      ],
      trace_events: [
        {
          event_id: 'run-history:1',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'node_started',
          sequence: 1,
          created_at: '2026-08-15T00:00:00Z',
          payload: {
            node_id: 'node-history',
            node_type: 'llm',
            title: 'History node'
          },
          delta_index: null,
          content_type: null,
          text: null
        },
        {
          event_id: 'run-history:3',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'reasoning_delta',
          sequence: 3,
          created_at: '2026-08-15T00:00:01Z',
          payload: {
            node_id: 'node-history',
            text: '历史思考',
            presentation: {
              kind: 'answer',
              answer_node_id: 'node-answer',
              segment_index: 0
            }
          },
          delta_index: 3,
          content_type: 'reasoning',
          text: '历史思考'
        },
        {
          event_id: 'run-history:4',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'text_delta',
          sequence: 4,
          created_at: '2026-08-15T00:00:02Z',
          payload: {
            node_id: 'node-answer',
            text: '历史最终回答',
            presentation: {
              kind: 'answer',
              answer_node_id: 'node-answer',
              segment_index: 0
            }
          },
          delta_index: 4,
          content_type: 'text',
          text: '历史最终回答'
        },
        {
          event_id: 'run-history:5',
          run_id: 'run-history',
          node_run_id: 'node-run-history',
          event_type: 'node_finished',
          sequence: 5,
          created_at: '2026-08-15T00:00:03Z',
          payload: {
            node_id: 'node-history',
            status: 'succeeded'
          },
          delta_index: null,
          content_type: null,
          text: null
        }
      ],
      has_more: false,
      next_sequence: null
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
    fireEvent.click(await screen.findByText('First conversation'));
    expect(await screen.findByText('历史最终回答')).toBeInTheDocument();
    expect(screen.queryByText('历史思考')).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByText(
        i18nText('appShell', 'auto.assistant_activity_duration_seconds', {
          value1: 3
        })
      )
    );
    const historyReasoning = await screen.findByText('历史思考');
    expect(
      historyReasoning.compareDocumentPosition(screen.getByText('历史最终回答'))
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    const activityButtons = screen.getAllByRole('button', {
      name: i18nText('appShell', 'auto.assistant_activity')
    });
    fireEvent.click(activityButtons.at(-1) as HTMLElement);

    const sidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    expect(within(sidebar).getByText('History node')).toBeInTheDocument();
    expect(within(sidebar).queryByText('历史思考')).not.toBeInTheDocument();
    expect(getConsoleAssistantRunActivity).toHaveBeenCalledWith(
      'flow-1',
      'run-history',
      { pageSize: 500 }
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
    await waitFor(() => expect(sendButton).toBeEnabled());
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

  test('AC-001 through AC-008 selects, removes, and submits multiple page references', async () => {
    startConsoleAssistantRunWebSocket.mockImplementation(
      async (_input, _csrfToken, handlers) => {
        handlers.onEvent({
          type: 'flow_accepted',
          run_id: 'run-page-reference',
          status: 'queued'
        });
        handlers.onEvent({
          type: 'flow_finished',
          run_id: 'run-page-reference',
          status: 'succeeded',
          output: { answer: '已分析引用区域' }
        });
      }
    );

    render(
      <AppProviders>
        <div id="reference-target" data-testid="reference-target">
          <span data-testid="reference-target-child">退款失败</span>
          <button data-testid="reference-target-action">重试</button>
        </div>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      })
    );

    const child = screen.getByTestId('reference-target-child');
    fireEvent.mouseMove(child);
    expect(
      document.querySelector('[data-testid="assistant-page-reference-outline"]')
    ).toBeInTheDocument();
    fireEvent.click(child);

    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      })
    );
    fireEvent.click(screen.getByTestId('reference-target-action'));

    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    );
    expect(assistantWindow).toHaveTextContent('span');
    const composer = screen.getByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const draftReferences = screen.getAllByTestId(
      'assistant-page-reference-draft'
    );
    expect(
      draftReferences[0].compareDocumentPosition(composer) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBeTruthy();
    fireEvent.click(
      screen.getAllByRole('button', {
        name: i18nText('appShell', 'auto.assistant_remove_page_reference')
      })[0]
    );
    expect(
      screen.getAllByTestId('assistant-page-reference-draft')
    ).toHaveLength(1);

    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      })
    );
    fireEvent.click(child);
    fireEvent.change(composer, {
      target: { value: '为什么这个区域显示失败？' }
    });
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.send_debug_message')
      })
    );

    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledWith(
        expect.objectContaining({
          query: '为什么这个区域显示失败？',
          page_references: [
            expect.objectContaining({
              outer_html:
                '<button data-testid="reference-target-action">重试</button>'
            }),
            expect.objectContaining({
              outer_html:
                '<span data-testid="reference-target-child">退款失败</span>'
            })
          ]
        }),
        'csrf-token',
        expect.any(Object),
        expect.any(Object)
      )
    );
    expect(assistantWindow).toHaveTextContent('span');
    expect(assistantWindow).not.toHaveTextContent(
      '<span data-testid="reference-target-child"'
    );
  });
});
