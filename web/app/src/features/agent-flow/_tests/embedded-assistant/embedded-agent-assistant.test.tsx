import {
  setupEmbeddedAssistantFixture,
  renderSettled,
  domEvents,
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
  ASSISTANT_WINDOW_SIZE_STORAGE_KEY,
  innerWidthSpy,
  findReadyHistoryButton
} from './assistant-fixture';
import { act, screen, waitFor } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import { AssistantRunTimeline } from '../../components/embedded-assistant/AssistantRunActivityPanel';
import * as runtimeApi from '../../api/runtime';
import { i18nText } from '../../../../shared/i18n/text';

describe('EmbeddedAgentAssistant', () => {
  setupEmbeddedAssistantFixture();

  test('AC-001 opens the Agent Flow Preview console instead of a hand-written drawer', async () => {
    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );

    const trigger = screen.getByRole('button', {
      name: i18nText('appShell', 'auto.assistant')
    });
    expect(trigger).toHaveTextContent('AI');

    await domEvents.click(trigger);

    await waitFor(() => {
      expect(getConsoleAssistantSettings).toHaveBeenCalledTimes(1);
    });
    await waitFor(
      () =>
        expect(
          document.querySelector('.agent-flow-editor__debug-console')
        ).toBeInTheDocument(),
      { timeout: 10_000 }
    );
    expect(
      screen.getByTestId('debug-conversation-messages')
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
    await domEvents.click(settings);
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
    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));

    const history = await findReadyHistoryButton();
    await domEvents.click(history);
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

    await domEvents.click(await screen.findByText('First conversation'));
    await waitFor(() =>
      expect(getConsoleAssistantConversationMessages).toHaveBeenCalledWith(
        'flow-1',
        'conversation-1'
      )
    );
    expect(
      screen.getByTestId('embedded-agent-assistant-preview')
    ).toHaveTextContent('div#failed-run');

    await domEvents.click(history);
    await domEvents.click(screen.getByRole('button', {
        name: new RegExp(
          i18nText('appShell', 'auto.assistant_new_conversation'),
          'u'
        )
      }));
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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await domEvents.click(await findReadyHistoryButton());

    expect(
      await screen.findByLabelText(
        i18nText('appShell', 'auto.assistant_status_cancelled')
      )
    ).toHaveAttribute('data-assistant-run-status', 'cancelled');
    await domEvents.click(await screen.findByText('Cancelled conversation'));

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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await domEvents.click(await findReadyHistoryButton());
    await domEvents.click(await screen.findByText('Cancelled without output'));

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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await domEvents.click(await findReadyHistoryButton());
    await domEvents.click(await screen.findByText('Active conversation'));

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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await domEvents.click(await findReadyHistoryButton());

    expect(await screen.findByLabelText('运行中')).toBeInTheDocument();
    expect(screen.getByLabelText('等待操作')).toBeInTheDocument();
    expect(screen.getByLabelText('运行失败')).toBeInTheDocument();
    expect(
      screen
        .queryByText('Completed conversation')
        ?.closest('.ant-conversations-item')
        ?.querySelector('[data-assistant-run-status]') ?? null
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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    const history = await findReadyHistoryButton();
    await domEvents.click(history);

    await waitFor(() => expect(emitConversation).toEqual(expect.any(Function)));
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
    await domEvents.click(history);
    expect(controller.signal.aborted).toBe(true);
  });

  test('AC-005 assigns each horizontal resize edge to its nearest pane', async () => {
    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));

    const history = await findReadyHistoryButton();
    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    const initialLeft = Number.parseFloat(assistantWindow.style.left);
    const initialWidth = Number.parseFloat(assistantWindow.style.width);

    await domEvents.click(history);

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
    await domEvents.mouseDown(resizeHandle, { clientX: 280 });
    await domEvents.mouseMove(window, { clientX: 340 });
    expect(historySidebar).toHaveStyle({ width: '340px' });
    expect(Number.parseFloat(assistantWindow.style.left)).toBe(expandedLeft);
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(expandedWidth);
    await domEvents.mouseUp(window);
    expect(historySidebar).toHaveAttribute('data-resizing', 'false');

    const conversationWidth =
      Number.parseFloat(assistantWindow.style.width) - 340 - 12;
    const leftResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} left`
    });
    await domEvents.mouseDown(leftResizeHandle, { clientX: expandedLeft });
    await domEvents.mouseMove(window, { clientX: expandedLeft - 60 });
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
    await domEvents.mouseUp(window);

    innerWidthSpy?.mockReturnValue(2_000);
    const rightResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} right`
    });
    const rightEdge = expandedLeft + expandedWidth;
    await domEvents.mouseDown(rightResizeHandle, { clientX: rightEdge });
    await domEvents.mouseMove(window, { clientX: rightEdge + 60 });
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
    await domEvents.mouseUp(window);

    await domEvents.click(history);
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
    const view = await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));

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
    await domEvents.mouseDown(rightResizeHandle, { clientX: initialRight });
    await domEvents.mouseMove(window, { clientX: initialRight + 80 });
    await domEvents.mouseUp(window);
    const bottomResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} bottom`
    });
    await domEvents.mouseDown(bottomResizeHandle, { clientY: initialBottom });
    await domEvents.mouseMove(window, { clientY: initialBottom - 100 });
    await domEvents.mouseUp(window);

    expect(
      JSON.parse(
        window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
      )
    ).toEqual({
      conversationWidth: initialWidth + 80,
      windowHeight: initialHeight - 100
    });

    const history = await findReadyHistoryButton();
    await domEvents.click(history);
    const historySidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    const expandedLeft = Number.parseFloat(assistantWindow.style.left);
    const leftResizeHandle = screen.getByRole('separator', {
      name: `${i18nText('appShell', 'auto.assistant')} left`
    });
    await domEvents.mouseDown(leftResizeHandle, { clientX: expandedLeft });
    await domEvents.mouseMove(window, { clientX: expandedLeft - 60 });
    await domEvents.mouseUp(window);
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
    await domEvents.mouseDown(divider, { clientX: 340 });
    await domEvents.mouseMove(window, { clientX: 380 });
    await domEvents.mouseUp(window);
    expect(historySidebar).toHaveStyle({ width: '380px' });
    expect(
      JSON.parse(
        window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
      )
    ).toEqual({
      conversationWidth: initialWidth + 40,
      windowHeight: initialHeight - 100
    });

    view.unmount();
    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
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
    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));

    const history = await findReadyHistoryButton();
    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    ) as HTMLElement;
    const initialLeft = assistantWindow.style.left;
    await domEvents.click(history);

    expect(
      await screen.findByTestId('embedded-agent-assistant-history')
    ).toBeInTheDocument();
    expect(document.querySelector('.ant-drawer')).not.toBeInTheDocument();
    expect(assistantWindow.style.left).toBe(initialLeft);
  });

  test('AC-005 switches an open history sidebar to the full assistant view after a narrow resize', async () => {
    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await domEvents.click(await findReadyHistoryButton());
    await screen.findByTestId('embedded-agent-assistant-history');

    innerWidthSpy?.mockReturnValue(640);
    await domEvents.fire(window, new Event('resize'));

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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await domEvents.click(await findReadyHistoryButton());
    await domEvents.click(await screen.findByText('Legacy snapshot'));

    await waitFor(() =>
      expect(getConsoleAssistantLegacySnapshotMessages).toHaveBeenCalledWith(
        'flow-1',
        'legacy-run'
      )
    );

    await domEvents.click(await screen.findByRole('button', {
        name: i18nText('appShell', 'auto.assistant_continue_legacy_snapshot')
      }));

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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    await domEvents.change(composer, { target: { value: 'Keep this conversation' } });
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.send_debug_message')
      }));
    await waitFor(() =>
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledTimes(1)
    );
    createConsoleAssistantConversation.mockClear();

    expect(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.clear_preview')
      })
    ).toBeDisabled();
    await domEvents.click(await findReadyHistoryButton());
    await domEvents.click(await screen.findByText('First conversation'));
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
    await renderSettled(
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

    await domEvents.click(trigger);

    expect(trigger).toHaveAttribute('aria-pressed', 'true');
    expect(trigger.closest('.embedded-agent-assistant-trigger')).toHaveClass(
      'ant-menu-item-selected'
    );

    await domEvents.click(trigger);

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

  test('#2018 AC-003 keeps the configured published application when opened from another application page', async () => {
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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant pageKey="/applications/current-draft/orchestration" />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));

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
    await domEvents.change(composer, { target: { value: 'Summarize this' } });
    await domEvents.click(sendButton);

    await waitFor(() => {
      expect(startConsoleAssistantRunWebSocket).toHaveBeenCalledWith(
        {
          application_id: 'flow-1',
          conversation_id: 'conversation-new',
          query: 'Summarize this'
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
          status: 'queued',
          event_id: 'run-cancelled-live:1',
          sequence: 1
        });
        handlers.onEvent({
          type: 'reasoning_delta',
          run_id: 'run-cancelled-live',
          node_id: 'node-answer',
          text: 'Temporary reasoning',
          event_id: 'run-cancelled-live:2',
          sequence: 2,
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
          event_id: 'run-cancelled-live:3',
          sequence: 3,
          presentation: {
            kind: 'answer',
            answer_node_id: 'node-answer',
            segment_index: 0
          }
        });
        handlers.onEvent({
          type: 'flow_cancelled',
          run_id: 'run-cancelled-live',
          status: 'cancelled',
          event_id: 'run-cancelled-live:4',
          sequence: 4
        });
      }
    );

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    const sendButton = screen.getByRole('button', {
      name: i18nText('agentFlow', 'auto.send_debug_message')
    });
    await waitFor(() => expect(sendButton).toBeEnabled());
    await domEvents.change(composer, { target: { value: 'Start then cancel' } });
    await domEvents.click(sendButton);

    expect(
      await screen.findByText('Public partial answer')
    ).toBeInTheDocument();
    expect(screen.queryByText('Temporary reasoning')).not.toBeInTheDocument();
    await domEvents.click(screen.getByText(
        `${i18nText('appShell', 'auto.assistant_activity_duration_unknown')} · ${i18nText('appShell', 'auto.assistant_status_cancelled')}`
      ));
    await domEvents.click(await screen.findByText(i18nText('agentFlow', 'auto.think')));
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
          sequence_start: 1,
          sequence_end: 1,
          created_at: '2026-08-16T00:00:00Z',
          text: '失败前思考'
        },
        {
          kind: 'error',
          event_id: 'run-failed:2',
          sequence_start: 2,
          sequence_end: 2,
          created_at: '2026-08-16T00:00:01Z',
          error: 'Provider rejected the request'
        }
      ],
      trace_events: [],
      has_more: false,
      next_sequence: null
    });

    await renderSettled(
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

    await renderSettled(
      <AppProviders>
        <div id="reference-target" data-testid="reference-target">
          <span data-testid="reference-target-child">退款失败</span>
          <button data-testid="reference-target-action">重试</button>
        </div>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      }));
    await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      }));

    const child = screen.getByTestId('reference-target-child');
    await domEvents.mouseMove(child);
    expect(
      document.querySelector('[data-testid="assistant-page-reference-outline"]')
    ).toBeInTheDocument();
    await domEvents.click(child);

    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      }));
    await domEvents.click(screen.getByTestId('reference-target-action'));

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
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    await domEvents.click(screen.getAllByRole('button', {
        name: i18nText('appShell', 'auto.assistant_remove_page_reference')
      })[0]);
    expect(
      screen.getAllByTestId('assistant-page-reference-draft')
    ).toHaveLength(1);

    await domEvents.click(screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      }));
    await domEvents.click(child);
    await domEvents.change(composer, {
      target: { value: '为什么这个区域显示失败？' }
    });
    await domEvents.click(screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.send_debug_message')
      }));

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
