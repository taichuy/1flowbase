import { describe, test, expect, vi } from 'vitest';
import './embedded-agent-assistant.fixtures';
import { AppProviders } from '../../../../app/AppProviders';
import { EmbeddedAgentAssistant } from '../../components/embedded-assistant/EmbeddedAgentAssistant';
import { act, screen, waitFor } from '@testing-library/react';
import {
  attachConsoleAssistantRunWebSocket,
  createConsoleAssistantConversation,
  domEvents,
  findReadyHistoryButton,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantSettings,
  listConsoleAssistantConversations,
  renderSettled,
  subscribeConsoleAssistantConversationsWebSocket
} from './assistant-fixture';
import { i18nText } from '../../../../shared/i18n/text';

describe('EmbeddedAgentAssistant', () => {
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );

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
    await domEvents.click(
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

    await renderSettled(
      <AppProviders>
        <EmbeddedAgentAssistant />
      </AppProviders>
    );
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
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
});
