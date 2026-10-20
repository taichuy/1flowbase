import { describe, test, expect } from 'vitest';
import { mockViewportSize, runtimeApi } from './floating-windows.fixtures';
import { App as AntdApp } from 'antd';
import { AppProviders } from '../../../../../app/AppProviders';
import { ApplicationLogsPage } from '../../../pages/ApplicationLogsPage';
import {
  applicationRunsPage,
  conversationMessagesPage,
  sampleRunDetail,
  sampleRunOverview
} from './floating-windows.support';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';

describe('ApplicationLogsPage - floating windows shell', () => {
  test('drags and resizes floating run detail window', async () => {
    mockViewportSize(1280, 900);

    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const detailWindow = await screen.findByTestId(
      'application-logs-floating-run-detail'
    );
    expect(detailWindow).toHaveStyle({
      left: '888px',
      top: '112px',
      width: '360px',
      height: '720px'
    });

    fireEvent.mouseDown(within(detailWindow).getByText('运行详情'), {
      button: 0,
      clientX: 980,
      clientY: 130
    });
    fireEvent.mouseMove(window, {
      clientX: 880,
      clientY: 190
    });
    fireEvent.mouseUp(window);

    expect(detailWindow).toHaveStyle({
      left: '788px',
      top: '172px'
    });

    fireEvent.mouseDown(
      within(detailWindow).getByRole('separator', {
        name: '从右侧调整运行详情宽度'
      }),
      {
        button: 0,
        clientX: 1148,
        clientY: 240
      }
    );
    fireEvent.mouseMove(window, {
      clientX: 1218,
      clientY: 240
    });
    fireEvent.mouseUp(window);

    expect(detailWindow).toHaveStyle({ width: '430px' });
    expect(
      window.localStorage.getItem(
        'applicationLogsFloatingWindowWidth:application-logs-floating-run-detail'
      )
    ).toBe('430');

    fireEvent.mouseDown(
      within(detailWindow).getByRole('separator', {
        name: '从左侧调整运行详情宽度'
      }),
      {
        button: 0,
        clientX: 788,
        clientY: 240
      }
    );
    fireEvent.mouseMove(window, {
      clientX: 728,
      clientY: 240
    });
    fireEvent.mouseUp(window);

    expect(detailWindow).toHaveStyle({
      left: '728px',
      width: '490px'
    });
    expect(
      window.localStorage.getItem(
        'applicationLogsFloatingWindowWidth:application-logs-floating-run-detail'
      )
    ).toBe('490');

    fireEvent.mouseDown(
      within(detailWindow).getByRole('separator', {
        name: '向下调整运行详情高度'
      }),
      {
        button: 0,
        clientX: 840,
        clientY: 892
      }
    );
    fireEvent.mouseMove(window, {
      clientX: 840,
      clientY: 820
    });
    fireEvent.mouseUp(window);

    expect(detailWindow).toHaveStyle({ height: '648px' });
    expect(
      await screen.findByTestId('debug-conversation-messages')
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '关闭运行详情' }));
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    expect(
      await screen.findByTestId('application-logs-floating-run-detail')
    ).toHaveStyle({
      left: '758px',
      width: '490px'
    });
  }, 20_000);

  test('opens a paginated-out run directly from the trace deep link', async () => {
    window.history.replaceState(
      {},
      '',
      '/applications/app-1/logs?run_id=run-deep&view=trace'
    );
    runtimeApi.fetchApplicationRuns.mockResolvedValue(applicationRunsPage([]));
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([])
    );

    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );

    expect(
      await screen.findByTestId('application-logs-floating-run-detail')
    ).toBeInTheDocument();
    const traceWindow = await screen.findByTestId(
      'application-logs-floating-conversation-log'
    );
    expect(
      within(traceWindow).getByRole('tab', { name: '追踪' })
    ).toHaveAttribute('aria-selected', 'true');
    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRunTraceTree).toHaveBeenCalledWith(
        'app-1',
        'run-deep'
      );
    });
    expect(window.location.search).toBe('?run_id=run-deep&view=trace');

    fireEvent.click(
      within(traceWindow).getByRole('button', { name: '关闭对话日志' })
    );
    expect(window.location.search).toBe('?run_id=run-deep');
    expect(
      screen.getByTestId('application-logs-floating-run-detail')
    ).toBeInTheDocument();
  });

  test('syncs the URL when switching and closing run details', async () => {
    window.history.replaceState(
      {},
      '',
      '/applications/app-1/logs?run_id=run-deep&view=trace'
    );
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([])
    );

    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );

    await screen.findByTestId('application-logs-floating-run-detail');
    fireEvent.click(
      await screen.findByRole('button', { name: '查看运行详情' })
    );

    expect(window.location.search).toBe('?run_id=run-1');
    expect(
      screen.queryByTestId('application-logs-floating-conversation-log')
    ).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '关闭运行详情' }));
    expect(window.location.search).toBe('');
  });
  test('collects four backend-associated calls in the original detail and opens each original trace', async () => {
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
      ...sampleRunOverview(),
      log_conversation_id: 'conversation-1'
    });
    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-1',
          run_mode: 'published_api_run',
          status: 'succeeded',
          title: '任务一',
          target_node_id: null,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:01Z',
          log_conversation_id: 'conversation-1',
          log_task_run_id: 'run-1',
          invocation_count: 4
        },
        {
          id: 'run-5',
          run_mode: 'published_api_run',
          status: 'succeeded',
          title: '任务二',
          target_node_id: null,
          started_at: '2026-04-17T09:01:00Z',
          finished_at: '2026-04-17T09:01:01Z',
          created_at: '2026-04-17T09:01:00Z',
          updated_at: '2026-04-17T09:01:01Z',
          log_conversation_id: 'conversation-1',
          log_task_run_id: 'run-5',
          invocation_count: 1
        }
      ])
    );
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage(
        [1, 2, 3, 4].map((n) => ({
          id: `message-${n}`,
          flow_run_id: `run-${n}`,
          role: 'assistant' as const,
          content: `调用 ${n}`,
          sequence: n
        }))
      )
    );
    runtimeApi.fetchApplicationLogConversationMessages.mockResolvedValue(
      conversationMessagesPage(
        [1, 2, 3, 4].map((n) => ({
          id: `message-${n}`,
          flow_run_id: `run-${n}`,
          role: 'assistant' as const,
          content: `调用 ${n}`,
          sequence: n
        }))
      )
    );
    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );
    expect(await screen.findByText('4 次调用')).toBeInTheDocument();
    expect(screen.getByText('任务二')).toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '会话日志' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('tab', { name: '原始运行日志' })
    ).not.toBeInTheDocument();
    fireEvent.click(
      within(screen.getByText('任务一').closest('tr')!).getByRole('button', {
        name: '查看运行详情'
      })
    );
    const links = await screen.findAllByRole('button', {
      name: '查看对话日志'
    });
    expect(links).toHaveLength(4);
    fireEvent.click(links[3]);
    await waitFor(() =>
      expect(runtimeApi.fetchApplicationRunOverview).toHaveBeenCalledWith(
        'app-1',
        'run-4'
      )
    );
    runtimeApi.fetchApplicationLogConversationMessages.mockResolvedValue(
      conversationMessagesPage([
        {
          id: 'session-task-1',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '所选任务的会话截止回复',
          sequence: 5
        }
      ])
    );
    fireEvent.click(screen.getByRole('button', { name: '返回当前任务' }));
    expect(await screen.findByText('调用 4')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看此会话' }));
    expect(
      await screen.findByText('所选任务的会话截止回复')
    ).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationLogConversationMessages
    ).toHaveBeenCalledWith('app-1', 'conversation-1', {
      aroundRunId: 'run-1',
      limit: 5
    });
    fireEvent.click(screen.getByRole('button', { name: '返回当前任务' }));
    expect(await screen.findByText('调用 4')).toBeInTheDocument();
  });

  test.each(['workflow_http', 'workflow_schedule'] as const)(
    'opens %s nodes without requiring conversation messages',
    async (invocation_source) => {
      runtimeApi.fetchApplicationRuns.mockResolvedValue(
        applicationRunsPage([
          {
            ...sampleRunDetail().flow_run,
            invocation_source,
            run_mode:
              invocation_source === 'workflow_http'
                ? 'workflow_http_run'
                : 'workflow_schedule_run'
          }
        ])
      );
      runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
        conversationMessagesPage([])
      );
      render(
        <AppProviders>
          <AntdApp>
            <ApplicationLogsPage
              applicationId="app-1"
              applicationType="workflow"
            />
          </AntdApp>
        </AppProviders>
      );
      fireEvent.click(
        await screen.findByRole('button', { name: '查看运行详情' })
      );
      const detail = await screen.findByTestId(
        'application-logs-floating-run-detail'
      );
      expect(
        within(detail).getByRole('tab', { name: '节点执行' })
      ).toHaveAttribute('aria-selected', 'true');
      fireEvent.click(
        await within(detail).findByRole('button', { name: /LLM.*llm/ })
      );
      expect(
        await within(detail).findByRole('region', { name: 'LLM 节点详情' })
      ).toBeInTheDocument();
      expect(
        runtimeApi.fetchApplicationRunConversationMessages
      ).not.toHaveBeenCalled();
      expect(
        screen.queryByTestId('application-logs-floating-conversation-log')
      ).not.toBeInTheDocument();
      fireEvent.click(
        within(detail).getByRole('button', { name: '关闭运行详情' })
      );
      expect(
        screen.queryByTestId('application-logs-floating-run-detail')
      ).not.toBeInTheDocument();
      expect(new URLSearchParams(window.location.search).has('run_id')).toBe(
        false
      );
    }
  );

  test.each(['', '&view=trace'])(
    'opens a workflow deep link in one node detail window (%s)',
    async (suffix) => {
      window.history.replaceState(
        {},
        '',
        `/applications/app-1/logs?run_id=run-1${suffix}`
      );
      runtimeApi.fetchApplicationRuns.mockResolvedValue(
        applicationRunsPage([])
      );
      render(
        <AppProviders>
          <AntdApp>
            <ApplicationLogsPage
              applicationId="app-1"
              applicationType="workflow"
            />
          </AntdApp>
        </AppProviders>
      );
      const detail = await screen.findByTestId(
        'application-logs-floating-run-detail'
      );
      expect(
        await within(detail).findByRole('button', { name: /LLM.*llm/ })
      ).toBeInTheDocument();
      expect(
        screen.queryByTestId('application-logs-floating-conversation-log')
      ).not.toBeInTheDocument();
      fireEvent.click(within(detail).getByRole('tab', { name: '详情' }));
      expect(within(detail).getByRole('tab', { name: '详情' })).toHaveAttribute(
        'aria-selected',
        'true'
      );
      await waitFor(() =>
        expect(runtimeApi.fetchApplicationRunOverview).toHaveBeenCalledWith(
          'app-1',
          'run-1'
        )
      );
      fireEvent.click(within(detail).getByRole('tab', { name: '节点执行' }));
      expect(new URLSearchParams(window.location.search).get('view')).toBe(
        'trace'
      );
      fireEvent.click(within(detail).getByRole('tab', { name: '详情' }));
      window.history.replaceState(
        {},
        '',
        '/applications/app-1/logs?run_id=run-1&view=trace'
      );
      fireEvent(window, new PopStateEvent('popstate'));
      expect(
        within(detail).getByRole('tab', { name: '节点执行' })
      ).toHaveAttribute('aria-selected', 'true');
      expect(
        runtimeApi.fetchApplicationRunConversationMessages
      ).not.toHaveBeenCalled();
    }
  );

  test('an agent flow without messages can still open its execution trace', async () => {
    window.history.replaceState(
      {},
      '',
      '/applications/app-1/logs?run_id=run-1'
    );
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([])
    );
    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage
            applicationId="app-1"
            applicationType="agent_flow"
          />
        </AntdApp>
      </AppProviders>
    );
    fireEvent.click(
      await screen.findByRole('button', { name: '查看执行轨迹' })
    );
    const trace = await screen.findByTestId(
      'application-logs-floating-conversation-log'
    );
    expect(within(trace).getByRole('tab', { name: '追踪' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(
      await within(trace).findByRole('button', { name: /LLM.*llm/ })
    ).toBeInTheDocument();
    expect(new URLSearchParams(window.location.search).get('view')).toBe(
      'trace'
    );
  });
});
