import { describe, test, expect, vi } from 'vitest';
import {
  ASSISTANT_WINDOW_SIZE_STORAGE_KEY,
  AppProviders,
  AssistantRunTimeline,
  EmbeddedAgentAssistant,
  act,
  cancelConsoleFlowRun,
  createConsoleAssistantConversation,
  domEvents,
  findReadyHistoryButton,
  getConsoleAssistantConversationMessages,
  getConsoleAssistantLegacySnapshotMessages,
  getConsoleAssistantRunActivity,
  getConsoleAssistantSettings,
  i18nText,
  innerWidthSpy,
  listConsoleAssistantConversations,
  renderSettled,
  runtimeApi,
  screen,
  startConsoleAssistantRunStream,
  startConsoleAssistantRunWebSocket,
  waitFor
} from './embedded-agent-assistant.fixtures';

describe('EmbeddedAgentAssistant', () => {
  test('resizes the history pane with focused arrow keys and persists the conversation width', async () => {
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
    const sidebar = await screen.findByTestId(
      'embedded-agent-assistant-history'
    );
    const divider = screen.getByTestId(
      'embedded-agent-assistant-history-resize'
    );
    const assistantWindow = screen.getByTestId(
      'embedded-agent-assistant-preview'
    );
    const initialWidth = Number.parseFloat(sidebar.style.width);
    const windowWidth = Number.parseFloat(assistantWindow.style.width);
    expect(divider).toHaveAttribute('tabindex', '0');
    await act(async () => {
      divider.focus();
    });
    expect(divider).toHaveFocus();
    await domEvents.fire(
      divider,
      new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true })
    );
    expect(sidebar).toHaveStyle({ width: `${initialWidth + 10}px` });
    expect(Number.parseFloat(assistantWindow.style.width)).toBe(windowWidth);
    const widerHistory = JSON.parse(
      window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
    );
    await domEvents.fire(
      divider,
      new KeyboardEvent('keydown', { key: 'ArrowLeft', bubbles: true })
    );
    expect(sidebar).toHaveStyle({ width: `${initialWidth}px` });
    const restoredHistory = JSON.parse(
      window.localStorage.getItem(ASSISTANT_WINDOW_SIZE_STORAGE_KEY) ?? '{}'
    );
    expect(restoredHistory.conversationWidth).toBe(
      widerHistory.conversationWidth + 10
    );
  });

  test('AC-005 assigns each horizontal resize edge to its nearest pane', async () => {
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
    await domEvents.click(
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
    await domEvents.click(
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await domEvents.click(await findReadyHistoryButton());
    await domEvents.click(await screen.findByText('Legacy snapshot'));

    await waitFor(() =>
      expect(getConsoleAssistantLegacySnapshotMessages).toHaveBeenCalledWith(
        'flow-1',
        'legacy-run'
      )
    );

    await domEvents.click(
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
    const composer = await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    await domEvents.change(composer, {
      target: { value: 'Keep this conversation' }
    });
    await domEvents.click(
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
    await domEvents.click(
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
    await domEvents.click(
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
    await domEvents.change(composer, {
      target: { value: 'Start then cancel' }
    });
    await domEvents.click(sendButton);

    expect(
      await screen.findByText('Public partial answer')
    ).toBeInTheDocument();
    expect(screen.queryByText('Temporary reasoning')).not.toBeInTheDocument();
    await domEvents.click(
      screen.getByText(
        `${i18nText('appShell', 'auto.assistant_activity_duration_unknown')} · ${i18nText('appShell', 'auto.assistant_status_cancelled')}`
      )
    );
    await domEvents.click(
      await screen.findByText(i18nText('agentFlow', 'auto.think'))
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
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant')
      })
    );
    await screen.findByPlaceholderText(
      i18nText('agentFlow', 'auto.chat_with_bots')
    );
    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      })
    );

    const child = screen.getByTestId('reference-target-child');
    await domEvents.mouseMove(child);
    expect(
      document.querySelector('[data-testid="assistant-page-reference-outline"]')
    ).toBeInTheDocument();
    await domEvents.click(child);

    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      })
    );
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
    await domEvents.click(
      screen.getAllByRole('button', {
        name: i18nText('appShell', 'auto.assistant_remove_page_reference')
      })[0]
    );
    expect(
      screen.getAllByTestId('assistant-page-reference-draft')
    ).toHaveLength(1);

    await domEvents.click(
      screen.getByRole('button', {
        name: i18nText('appShell', 'auto.assistant_select_page_content')
      })
    );
    await domEvents.click(child);
    await domEvents.change(composer, {
      target: { value: '为什么这个区域显示失败？' }
    });
    await domEvents.click(
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
