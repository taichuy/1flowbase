import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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
  fetchApplicationRunConversationMessages: vi.fn()
}));

const debugConsoleState = vi.hoisted(() => ({
  latestMessages: [] as AgentFlowDebugMessage[]
}));

vi.mock('../api/runtime', () => runtimeApi);

vi.mock('../../agent-flow/components/debug-console/AgentFlowDebugConsole', () => ({
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
            <div>{message.content}</div>
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
}));

import { ApplicationRunDetailPanel } from '../components/logs/ApplicationRunDetailPanel';

type ConversationItemInput = {
  run_id?: string;
  detail_run_id?: string | null;
  can_open_detail?: boolean;
  role?: 'system' | 'user' | 'assistant' | null;
  content?: string | null;
  status: string;
  query?: string | null;
  answer?: string | null;
  is_current?: boolean;
};

function conversationPage(items: ConversationItemInput[]) {
  return {
    items: items.map((item) => ({
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
    page: {
      has_before: false,
      has_after: false,
      before_cursor: null,
      after_cursor: null
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
    debugConsoleState.latestMessages = [];
  });

  test('AC-001/AC-003 renders a UI-only bot message for waiting callback runs with no answer and keeps the detail action', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationPage([
        {
          status: 'waiting_callback',
          query: '> 要我按 A + B1 动手吗？',
          answer: null,
          can_open_detail: true
        }
      ])
    );
    const { onOpenMessageLog } = renderPanel({});

    expect(
      await screen.findByText('等待 Callback 回填中，暂时还没有输出。')
    ).toBeInTheDocument();
    expect(screen.getByText('> 要我按 A + B1 动手吗？')).toBeInTheDocument();

    const assistantMessage = screen.getByTestId('message-assistant');
    expect(assistantMessage).toHaveAttribute('data-can-open-detail', 'true');
    fireEvent.click(
      within(assistantMessage).getByRole('button', {
        name: 'open-assistant-run-1'
      })
    );

    await waitFor(() => {
      expect(onOpenMessageLog).toHaveBeenCalledWith(
        expect.objectContaining({
          role: 'assistant',
          content: '等待 Callback 回填中，暂时还没有输出。',
          detailRunId: 'run-1',
          canOpenDetail: true
        })
      );
    });
  });

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

  test('AC-003 keeps the fallback bot message closed when can_open_detail is false', async () => {
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

    expect(
      await screen.findByText('等待人工输入中，暂时还没有输出。')
    ).toBeInTheDocument();

    const assistantMessage = screen.getByTestId('message-assistant');
    expect(assistantMessage).toHaveAttribute('data-can-open-detail', 'false');
    expect(
      within(assistantMessage).queryByRole('button', {
        name: 'open-assistant-run-1'
      })
    ).not.toBeInTheDocument();
  });
});
