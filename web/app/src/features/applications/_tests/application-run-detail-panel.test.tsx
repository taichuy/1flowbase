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
    input?: { limit?: number }
  ) =>
    [
      'applications',
      applicationId,
      'log-conversation',
      conversationId,
      input?.limit ?? 'default'
    ] as const,
  fetchApplicationLogConversationMessages: vi.fn()
}));

const debugConsoleState = vi.hoisted(() => ({
  latestMessages: [] as AgentFlowDebugMessage[]
}));

vi.mock('../api/runtime', () => runtimeApi);

vi.mock(
  '../../agent-flow/components/debug-console/AgentFlowDebugConsole',
  () => ({
    AgentFlowDebugConsole: ({
      messages,
      onOpenMessageLog
    }: {
      messages: AgentFlowDebugMessage[];
      onOpenMessageLog?: (message: AgentFlowDebugMessage) => void;
    }) => {
      debugConsoleState.latestMessages = messages;

      return (
        <section data-testid="debug-console">
          {messages.map((message) => (
            <article
              data-can-open-detail={String(message.canOpenDetail)}
              data-testid={`message-${message.role}`}
              key={message.id}
            >
              <div data-testid="message-content">{message.content}</div>
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
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchApplicationLogConversationMessages.mockReset();
    debugConsoleState.latestMessages = [];
  });

  test('#2105 opens the complete series at its latest five turns and refetches on reopen', async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false, staleTime: Infinity } }
    });
    const page = (prefix: string) =>
      conversationPage(
        Array.from({ length: 5 }, (_, index) => ({
          message_id: `turn-${index}`,
          run_id: `run-${index}`,
          detail_run_id: `run-${index}`,
          status: 'succeeded',
          query: `${prefix} question ${index}`,
          answer: `${prefix} answer ${index}`
        }))
      );
    runtimeApi.fetchApplicationLogConversationMessages
      .mockResolvedValueOnce(page('first'))
      .mockResolvedValueOnce(page('latest'));
    const surface = (open: boolean) => (
      <QueryClientProvider client={client}>
        <App>
          <ApplicationRunDetailPanel
            applicationId="app-1"
            runId={open ? 'old-selected-run' : null}
            logConversationId="series-1"
            onClose={() => {}}
          />
        </App>
      </QueryClientProvider>
    );
    const view = render(surface(true));
    expect(await screen.findByText('first answer 4')).toBeInTheDocument();
    expect(screen.getAllByTestId('message-user')).toHaveLength(5);
    expect(screen.getAllByTestId('message-assistant')).toHaveLength(5);
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).toHaveBeenCalledWith('app-1', 'series-1', { limit: 5 });
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

  test.each([
    'waiting_callback',
    'waiting_human',
    'running',
    'failed',
    'cancelled'
  ])(
    '#2105 %s retains the user input without inventing an assistant answer',
    async (status) => {
      runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
        conversationPage([
          { status, query: 'Review this change', answer: null }
        ])
      );
      renderPanel({});
      expect(await screen.findByText('Review this change')).toBeInTheDocument();
      expect(screen.queryByTestId('message-assistant')).not.toBeInTheDocument();
    }
  );

  test('AC-002 does not synthesize a bot message for succeeded runs without an answer', async () => {
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
    expect(screen.queryByTestId('message-assistant')).not.toBeInTheDocument();
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

  test('#2105 a waiting turn preserves the input detail permission without a placeholder answer', async () => {
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
    expect(screen.queryByTestId('message-assistant')).not.toBeInTheDocument();
    expect(screen.getByTestId('message-user')).toHaveAttribute(
      'data-can-open-detail',
      'false'
    );
  });
});
