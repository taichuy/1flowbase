import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { AgentFlowDebugMessage } from '../../agent-flow/api/runtime';
import { appI18n } from '../../../shared/i18n/app-i18n';

const runtimeApi = vi.hoisted(() => ({
  applicationRunOverviewQueryKey: (applicationId: string, runId: string) => [
    'applications',
    applicationId,
    'runtime',
    'runs',
    runId,
    'overview'
  ],
  fetchApplicationRunOverview: vi.fn(),
  applicationRunConversationMessagesQueryKey: (
    applicationId: string,
    runId: string,
    input?: { limit?: number }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'conversation-messages',
      input?.limit ?? 'default'
    ] as const,
  fetchApplicationRunConversationMessages: vi.fn(),
  applicationLogConversationMessagesQueryKey: (
    applicationId: string,
    conversationId: string,
    input?: { limit?: number; aroundRunId?: string }
  ) =>
    [
      'applications',
      applicationId,
      'log-conversation',
      conversationId,
      input?.aroundRunId,
      input?.limit ?? 'default'
    ] as const,
  fetchApplicationLogConversationMessages: vi.fn()
}));

const debugConsoleState = vi.hoisted(() => ({
  latestMessages: [] as AgentFlowDebugMessage[]
}));

vi.mock('../api/runtime', () => runtimeApi);

const trajectoryApi = vi.hoisted(() => ({
  fetchApplicationLogRecord: vi.fn()
}));
vi.mock('../api/trajectory', () => trajectoryApi);

vi.mock(
  '../../agent-flow/components/debug-console/AgentFlowDebugConsole',
  () => ({
    AgentFlowDebugConsole: ({
      messages,
      onOpenMessageLog,
      onReachConversationTop,
      assistantMessageActions
    }: {
      messages: AgentFlowDebugMessage[];
      onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
      onReachConversationTop?: () => void;
      assistantMessageActions?: (message: AgentFlowDebugMessage) => ReactNode;
    }) => {
      debugConsoleState.latestMessages = messages;

      return (
        <section data-testid="debug-console">
          <button onClick={onReachConversationTop}>load history</button>
          {messages.map((message) => (
            <article
              data-can-open-detail={String(message.canOpenDetail)}
              data-testid={`message-${message.role}`}
              key={message.id}
            >
              <div data-testid="message-content">{message.content}</div>
              {message.role === 'assistant'
                ? assistantMessageActions?.(message)
                : null}
              {message.canOpenDetail !== false ? (
                <button
                  aria-label={`open-${message.role}-${message.runId ?? 'none'}`}
                  type="button"
                  onClick={() => onOpenMessageLog?.(message)}
                >
                  open
                </button>
              ) : null}
            </article>
          ))}
        </section>
      );
    }
  })
);

import { ApplicationRunDetailPanel } from '../components/logs/ApplicationRunDetailPanel';

type ConversationItemInput = {
  message_id?: string;
  run_id?: string;
  detail_run_id?: string | null;
  can_open_detail?: boolean;
  role?: 'system' | 'user' | 'assistant' | null;
  content?: string | null;
  status: string;
  query?: string | null;
  answer?: string | null;
  is_current?: boolean;
  output_source?:
    | 'provider_output_item'
    | 'projection_timeout'
    | 'waiting_callback'
    | 'persisted_answer'
    | 'error'
    | 'none';
  context_source?: 'client_request' | 'application_config' | 'effective_prompt';
  sequence?: number;
};

function conversationPage(
  items: ConversationItemInput[],
  page: {
    output_state?: unknown;
    has_before?: boolean;
    has_after?: boolean;
    before_cursor?: string | null;
    after_cursor?: string | null;
    newest_cursor?: string | null;
  } = {}
) {
  return {
    items: items.map((item) => ({
      message_id:
        item.message_id ?? `message-${item.status}-${item.query ?? ''}`,
      sequence: item.sequence,
      output_source: item.output_source,
      context_source: item.context_source,
      run_id: item.run_id ?? 'run-1',
      detail_run_id: item.detail_run_id ?? 'run-1',
      can_open_detail: item.can_open_detail,
      role: item.role ?? null,
      content: item.content ?? null,
      started_at: '2026-07-07T01:00:00Z',
      finished_at: null,
      status: item.status,
      query: item.query ?? null,
      model: null,
      answer: item.answer ?? null,
      is_current: item.is_current ?? true
    })),
    output_state: page.output_state ?? null,
    page: {
      has_before: page.has_before ?? false,
      has_after: page.has_after ?? false,
      before_cursor: page.before_cursor ?? null,
      after_cursor: page.after_cursor ?? null,
      newest_cursor: page.newest_cursor ?? null
    }
  };
}

function renderPanel({
  children,
  onOpenMessageLog = vi.fn()
}: {
  children?: ReactNode;
  onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
}) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false
      }
    }
  });

  render(
    <QueryClientProvider client={queryClient}>
      <App>
        {children ?? (
          <ApplicationRunDetailPanel
            applicationId="app-1"
            runId="run-1"
            onClose={() => {}}
            onOpenMessageLog={onOpenMessageLog}
          />
        )}
      </App>
    </QueryClientProvider>
  );

  return { onOpenMessageLog };
}

describe('ApplicationRunDetailPanel', () => {
  beforeEach(async () => {
    await appI18n.changeLanguage('zh_Hans');
    runtimeApi.fetchApplicationRunOverview.mockReset();
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
      log_conversation_id: null
    });
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchApplicationLogConversationMessages.mockReset();
    trajectoryApi.fetchApplicationLogRecord.mockReset();
    debugConsoleState.latestMessages = [];
  });

  test('#2336 uses overview ownership without a list row and anchors the selected old run on reopen', async () => {
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
      log_conversation_id: 'series-1'
    });
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } }
    });
    const page = (prefix: string) =>
      conversationPage(
        Array.from({ length: 5 }, (_, index) => ({
          message_id: `turn-${index}`,
          run_id: index === 4 ? 'old-selected-run' : `run-${index}`,
          detail_run_id: index === 4 ? 'old-selected-run' : `run-${index}`,
          status: 'succeeded',
          query: `${prefix} question ${index}`,
          answer: `${prefix} answer ${index}`
        }))
      );
    let opens = 0;
    runtimeApi.fetchApplicationLogConversationMessages.mockImplementation(
      (_applicationId, _conversationId, input) => {
        opens += 1;
        return Promise.resolve(
          input?.aroundRunId === 'old-selected-run'
            ? page(opens === 1 ? 'first' : 'latest')
            : conversationPage([
                { status: 'succeeded', answer: 'future answer' }
              ])
        );
      }
    );
    const surface = (open: boolean) => (
      <QueryClientProvider client={client}>
        <App>
          <ApplicationRunDetailPanel
            applicationId="app-1"
            runId={open ? 'old-selected-run' : null}
            onClose={() => {}}
          />
        </App>
      </QueryClientProvider>
    );
    const view = render(surface(true));
    expect(await screen.findByText('first answer 4')).toBeInTheDocument();
    expect(screen.getAllByTestId('message-user')).toHaveLength(5);
    expect(screen.getAllByTestId('message-assistant')).toHaveLength(5);
    expect(screen.queryByText('future answer')).not.toBeInTheDocument();
    expect(debugConsoleState.latestMessages.at(-1)).toEqual(
      expect.objectContaining({
        runId: 'old-selected-run',
        content: 'first answer 4'
      })
    );
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).toHaveBeenCalledWith('app-1', 'series-1', {
      aroundRunId: 'old-selected-run',
      limit: 5
    });
    expect(runtimeApi.fetchApplicationRunOverview).toHaveBeenCalledWith(
      'app-1',
      'old-selected-run'
    );
    expect(
      runtimeApi.fetchApplicationRunConversationMessages
    ).not.toHaveBeenCalled();
    view.rerender(surface(false));
    view.rerender(surface(true));
    expect(await screen.findByText('latest answer 4')).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).toHaveBeenCalledTimes(2);
  });

  test('#2336 history paging keeps the selected cutoff and the current-task switch remains available', async () => {
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
      log_conversation_id: 'series-1'
    });
    runtimeApi.fetchApplicationLogConversationMessages.mockImplementation(
      (_applicationId, _conversationId, input) =>
        Promise.resolve(
          input?.before
            ? conversationPage([
                {
                  message_id: 'older',
                  run_id: 'older-run',
                  status: 'succeeded',
                  query: 'older question',
                  answer: 'older answer'
                }
              ])
            : conversationPage(
                [
                  {
                    message_id: 'selected',
                    status: 'succeeded',
                    query: 'selected question',
                    answer: 'selected final answer'
                  }
                ],
                {
                  has_before: true,
                  before_cursor: 'run-1'
                }
              )
        )
    );
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage([
        {
          message_id: 'current-only',
          status: 'succeeded',
          answer: 'current task answer'
        }
      ])
    );
    renderPanel({});
    expect(
      await screen.findByText('selected final answer')
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'load history' }));
    expect(await screen.findByText('older answer')).toBeInTheDocument();
    expect(screen.getByText('selected final answer')).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).toHaveBeenCalledWith('app-1', 'series-1', {
      aroundRunId: 'run-1',
      before: 'run-1',
      limit: 5
    });
    fireEvent.click(screen.getByRole('button', { name: '返回当前任务' }));
    expect(await screen.findByText('current task answer')).toBeInTheDocument();
    expect(screen.queryByText('older answer')).not.toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationRunConversationMessages
    ).toHaveBeenCalledWith('app-1', 'run-1', { limit: 5 });
  });

  test('#2336 active conversation refresh and catchup keep the cutoff while replacing streamed output', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
        log_conversation_id: 'series-1'
      });
      let refreshes = 0;
      const item = (
        message_id: string,
        content: string,
        status = 'running'
      ) => ({
        message_id,
        run_id: 'run-1',
        status,
        role: 'assistant' as const,
        content
      });
      runtimeApi.fetchApplicationLogConversationMessages.mockImplementation(
        (_applicationId, _conversationId, input) => {
          if (input?.after === 'draft-cursor') {
            return Promise.resolve(
              conversationPage(
                [
                  item('stream', 'final output', 'succeeded'),
                  item('between', 'output between pages', 'succeeded')
                ],
                { after_cursor: 'final-cursor', newest_cursor: 'final-cursor' }
              )
            );
          }
          refreshes += 1;
          return Promise.resolve(
            refreshes === 1
              ? conversationPage([item('stream', 'streamed draft')], {
                  newest_cursor: 'draft-cursor'
                })
              : conversationPage(
                  [item('stream', 'final output', 'succeeded')],
                  { newest_cursor: 'final-cursor' }
                )
          );
        }
      );
      renderPanel({});
      expect(await screen.findByText('streamed draft')).toBeInTheDocument();
      await act(async () => {
        await vi.advanceTimersByTimeAsync(1_500);
      });
      expect(await screen.findByText('final output')).toBeInTheDocument();
      expect(
        await screen.findByText('output between pages')
      ).toBeInTheDocument();
      expect(screen.queryByText('streamed draft')).not.toBeInTheDocument();
      expect(
        runtimeApi.fetchApplicationLogConversationMessages
      ).toHaveBeenCalledWith('app-1', 'series-1', {
        aroundRunId: 'run-1',
        after: 'draft-cursor',
        limit: 5
      });
      for (const call of runtimeApi.fetchApplicationLogConversationMessages.mock
        .calls) {
        expect(call[2]).toEqual(
          expect.objectContaining({ aroundRunId: 'run-1' })
        );
      }
      expect(
        runtimeApi.fetchApplicationRunConversationMessages
      ).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  test('#2336 different selected runs in one series do not share a conversation cache entry', async () => {
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
      log_conversation_id: 'series-1'
    });
    runtimeApi.fetchApplicationLogConversationMessages.mockImplementation(
      (_applicationId, _conversationId, input) =>
        Promise.resolve(
          conversationPage([
            { status: 'succeeded', answer: `final for ${input.aroundRunId}` }
          ])
        )
    );
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } }
    });
    const panel = (runId: string) => (
      <QueryClientProvider client={client}>
        <App>
          <ApplicationRunDetailPanel
            applicationId="app-1"
            runId={runId}
            onClose={() => {}}
          />
        </App>
      </QueryClientProvider>
    );
    const view = render(panel('old-run'));
    expect(await screen.findByText('final for old-run')).toBeInTheDocument();
    view.rerender(panel('new-run'));
    expect(await screen.findByText('final for new-run')).toBeInTheDocument();
    expect(screen.queryByText('final for old-run')).not.toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).toHaveBeenCalledWith('app-1', 'series-1', {
      aroundRunId: 'new-run',
      limit: 5
    });
  });

  test('#2336 imported records do not request native overview or conversations', async () => {
    trajectoryApi.fetchApplicationLogRecord.mockResolvedValue({
      native_run_id: null,
      available_views: ['client_trajectory'],
      messages: [{ sequence: 1, role: 'assistant', content: 'imported answer' }]
    });
    renderPanel({
      children: (
        <ApplicationRunDetailPanel
          applicationId="app-1"
          runId="record-1"
          recordId="record-1"
          onClose={() => {}}
        />
      )
    });
    expect(await screen.findByText('imported answer')).toBeInTheDocument();
    expect(runtimeApi.fetchApplicationRunOverview).not.toHaveBeenCalled();
    expect(
      runtimeApi.fetchApplicationRunConversationMessages
    ).not.toHaveBeenCalled();
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).not.toHaveBeenCalled();
  });

  test.each([
    'waiting_callback',
    'waiting_human',
    'running',
    'failed',
    'cancelled',
    'succeeded'
  ])(
    '%s exposes a status-only item and its detail entry without inventing an answer',
    async (status) => {
      runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
        conversationPage([
          { status, query: 'Review this change', answer: null }
        ])
      );
      const { onOpenMessageLog } = renderPanel({});
      expect(await screen.findByText('Review this change')).toBeInTheDocument();
      const statusItem = screen.getByTestId('message-assistant');
      expect(
        within(statusItem).getByTestId('message-content')
      ).toBeEmptyDOMElement();
      const statusMessage = debugConsoleState.latestMessages.find(
        (message) => message.role === 'assistant'
      );
      expect(statusMessage).toMatchObject({
        content: '',
        presentation: 'status',
        status: status === 'succeeded' ? 'completed' : status,
        detailRunId: 'run-1',
        canOpenDetail: true
      });
      fireEvent.click(within(statusItem).getByRole('button'));
      expect(onOpenMessageLog).toHaveBeenCalledWith(statusMessage);
    }
  );

  test('native user items get one status region per run, replaced when a real reply arrives', async () => {
    const queryClient = new QueryClient({
      defaultOptions: { queries: { retry: false } }
    });
    const nativeInputs = [
      {
        message_id: 'input-1',
        role: 'user' as const,
        content: 'first input',
        status: 'cancelled'
      },
      {
        message_id: 'input-2',
        role: 'user' as const,
        content: 'second input',
        status: 'cancelled'
      }
    ];
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage(nativeInputs)
    );
    renderPanel({
      children: (
        <QueryClientProvider client={queryClient}>
          <ApplicationRunDetailPanel
            applicationId="app-1"
            runId="run-1"
            onClose={() => {}}
          />
        </QueryClientProvider>
      )
    });
    expect(await screen.findByText('second input')).toBeInTheDocument();
    expect(screen.getAllByTestId('message-assistant')).toHaveLength(1);
    expect(
      within(screen.getByTestId('message-assistant')).getByTestId(
        'message-content'
      )
    ).toBeEmptyDOMElement();
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage([
        ...nativeInputs,
        {
          message_id: 'reply',
          role: 'assistant',
          content: 'actual recorded reply',
          status: 'succeeded'
        }
      ])
    );
    await act(async () => {
      await queryClient.invalidateQueries();
    });
    expect(
      await screen.findByText('actual recorded reply')
    ).toBeInTheDocument();
    expect(screen.getAllByTestId('message-assistant')).toHaveLength(1);
    expect(
      debugConsoleState.latestMessages.some((message) =>
        message.id.startsWith('conversation-status-')
      )
    ).toBe(false);
  });

  test.each([
    ['waiting_callback', 'waiting_callback'],
    ['Timeout', 'projection_timeout'],
    ['最后一次模型输出', 'projection_timeout']
  ] as const)(
    'renders backend projection %s with its log entry',
    async (answer, output_source) => {
      runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
        conversationPage([
          {
            status: 'waiting_callback',
            query: '继续',
            answer,
            output_source,
            can_open_detail: true
          }
        ])
      );
      const onOpenMessageLog = vi.fn();
      renderPanel({ onOpenMessageLog });
      expect(await screen.findByText(answer)).toBeInTheDocument();
      expect(screen.getAllByTestId('message-assistant')).toHaveLength(1);
      fireEvent.click(
        screen.getByRole('button', { name: 'open-assistant-run-1' })
      );
      expect(onOpenMessageLog).toHaveBeenCalledWith(
        expect.objectContaining({
          detailRunId: 'run-1',
          canOpenDetail: true,
          content: answer
        })
      );
    }
  );

  test('a succeeded run without an answer shows status without invented answer content', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage([
        {
          status: 'succeeded',
          query: '继续',
          answer: null,
          can_open_detail: true
        }
      ])
    );
    renderPanel({});

    expect(await screen.findByText('继续')).toBeInTheDocument();
    expect(
      screen.queryByText('运行中，暂时还没有输出。')
    ).not.toBeInTheDocument();
    expect(
      within(screen.getByTestId('message-assistant')).getByTestId(
        'message-content'
      )
    ).toBeEmptyDOMElement();
    expect(
      debugConsoleState.latestMessages.find(
        (message) => message.role === 'assistant'
      )
    ).toMatchObject({ status: 'completed', content: '' });
  });

  test('#2090 AC-001/AC-003 exposes the run system context beside the page with its source', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage(
        [
          {
            message_id: 'context-1',
            sequence: 1_000_000,
            status: 'succeeded',
            role: 'system',
            content: 'You are running inside the Codex app.',
            context_source: 'client_request',
            can_open_detail: false
          },
          {
            message_id: 'context-2',
            sequence: 1_000_001,
            status: 'succeeded',
            role: 'system',
            content: 'Use concise Chinese.',
            context_source: 'effective_prompt',
            can_open_detail: false
          },
          {
            message_id: 'message-running',
            sequence: 0,
            status: 'running',
            query: '继续',
            answer: null,
            can_open_detail: true
          }
        ],
        {
          output_state: {
            run_id: 'run-1',
            status: 'waiting_callback',
            call_kind: 'generate',
            request_kind: 'turn',
            output_source: 'provider_output_item',
            output_item_count: 3
          }
        }
      )
    );
    renderPanel({});

    expect(await screen.findByText('继续')).toBeInTheDocument();
    // The system context is the first turn of the conversation, labelled with
    // the layer it came from, instead of a separate collapsible panel.
    // The system prompt is the first turn of the conversation and carries no
    // extra label: a per-message source caption would mislead the reader.
    const systemMessages = await screen.findAllByTestId('message-system');
    expect(systemMessages).toHaveLength(2);
    const promptText = within(systemMessages[0]).getByTestId('message-content');
    expect(promptText).toHaveTextContent(
      'You are running inside the Codex app.'
    );
    expect(promptText).not.toHaveTextContent('系统上下文');
    expect(
      screen.queryByTestId('message-source-label')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('run-conversation-contexts')
    ).not.toBeInTheDocument();
  });

  test('#2105 prewarm remains empty instead of inventing a business answer', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage(
        [
          {
            status: 'succeeded',
            query: null,
            answer: null,
            can_open_detail: true,
            output_source: 'none'
          }
        ],
        {
          output_state: {
            run_id: 'run-1',
            status: 'succeeded',
            call_kind: 'generate',
            request_kind: 'prewarm',
            output_source: 'none',
            output_item_count: 0
          }
        }
      )
    );
    renderPanel({});

    await waitFor(() =>
      expect(
        runtimeApi.fetchApplicationRunConversationMessages
      ).toHaveBeenCalled()
    );
    expect(screen.queryByTestId('message-assistant')).not.toBeInTheDocument();
  });

  test('#2090 AC-004 keeps refreshing a waiting call whose page has no active item', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
        conversationPage(
          [
            {
              status: 'waiting_callback',
              query: 'start',
              answer: '等待 Callback 回填中，暂时还没有输出。',
              can_open_detail: true,
              output_source: 'none'
            }
          ],
          {
            output_state: {
              run_id: 'run-1',
              status: 'waiting_callback',
              call_kind: 'generate',
              request_kind: 'turn',
              output_source: 'none',
              output_item_count: 0
            }
          }
        )
      );
      renderPanel({});

      await screen.findByText('start');
      const initialCalls =
        runtimeApi.fetchApplicationRunConversationMessages.mock.calls.length;

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2_500);
      });

      expect(
        runtimeApi.fetchApplicationRunConversationMessages.mock.calls.length
      ).toBeGreaterThan(initialCalls);
    } finally {
      vi.useRealTimers();
    }
  });

  test('#2090 AC-004 catches up on more new items than one page holds', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const item = (sequence: number, content: string) => ({
        message_id: `message-${sequence}`,
        sequence,
        status: 'succeeded',
        role: 'assistant' as const,
        content,
        can_open_detail: false
      });
      const outputState = {
        run_id: 'run-1',
        status: 'running',
        call_kind: 'generate',
        request_kind: 'turn',
        output_source: 'provider_output_item',
        output_item_count: 2
      };
      let pollCount = 0;
      runtimeApi.fetchApplicationRunConversationMessages.mockImplementation(
        (...args: unknown[]) => {
          const input = args[2] as
            | { before?: string | null; after?: string | null }
            | undefined;
          if (input?.after === 'run-1:context:9') {
            return Promise.resolve(
              conversationPage([item(10, 'new-10'), item(11, 'new-11')], {
                output_state: outputState,
                newest_cursor: 'run-1:context:11'
              })
            );
          }
          pollCount += 1;
          if (pollCount === 1) {
            return Promise.resolve(
              conversationPage([item(5, 'old-5'), item(9, 'old-9')], {
                output_state: outputState,
                has_before: true,
                before_cursor: 'run-1:context:5',
                newest_cursor: 'run-1:context:9'
              })
            );
          }
          return Promise.resolve(
            conversationPage([item(10, 'new-10'), item(11, 'new-11')], {
              output_state: outputState,
              has_before: true,
              before_cursor: 'run-1:context:10',
              newest_cursor: 'run-1:context:11'
            })
          );
        }
      );
      renderPanel({});

      await screen.findByText('old-5');
      expect(screen.queryByText('new-11')).not.toBeInTheDocument();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(2_500);
      });

      await waitFor(() => {
        expect(
          runtimeApi.fetchApplicationRunConversationMessages
        ).toHaveBeenCalledWith(
          'app-1',
          'run-1',
          expect.objectContaining({ after: 'run-1:context:9' })
        );
      });
      expect(await screen.findByText('new-11')).toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  test('#2090 AC-004 replaces a refreshed item instead of keeping the stale copy', async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    try {
      const outputState = {
        run_id: 'run-1',
        status: 'running',
        call_kind: 'generate',
        request_kind: 'turn',
        output_source: 'provider_output_item',
        output_item_count: 1
      };
      let pollCount = 0;
      runtimeApi.fetchApplicationRunConversationMessages.mockImplementation(
        () => {
          pollCount += 1;
          return Promise.resolve(
            conversationPage(
              [
                {
                  message_id: 'message-1',
                  sequence: 1,
                  status: 'running',
                  role: 'assistant',
                  content: pollCount === 1 ? 'streaming draft' : 'final answer',
                  can_open_detail: false
                }
              ],
              { output_state: outputState, newest_cursor: 'run-1:context:1' }
            )
          );
        }
      );
      renderPanel({});

      expect(await screen.findByText('streaming draft')).toBeInTheDocument();

      await act(async () => {
        await vi.advanceTimersByTimeAsync(1_500);
      });

      expect(await screen.findByText('final answer')).toBeInTheDocument();
      expect(screen.queryByText('streaming draft')).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  test('a status-only item preserves the backend detail permission', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage([
        {
          status: 'waiting_human',
          query: '请人工审核',
          answer: null,
          can_open_detail: false
        }
      ])
    );
    renderPanel({});
    expect(await screen.findByText('请人工审核')).toBeInTheDocument();
    expect(
      within(screen.getByTestId('message-assistant')).getByTestId(
        'message-content'
      )
    ).toBeEmptyDOMElement();
    expect(screen.getByTestId('message-assistant')).toHaveAttribute(
      'data-can-open-detail',
      'false'
    );
    expect(screen.getByTestId('message-user')).toHaveAttribute(
      'data-can-open-detail',
      'false'
    );
  });
});
