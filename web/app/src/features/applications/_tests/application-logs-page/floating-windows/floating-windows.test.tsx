import { describe, test, expect } from 'vitest';
import {
  AntdApp,
  AppProviders,
  ApplicationLogsPage,
  act,
  applicationRunsPage,
  conversationMessagesPage,
  fireEvent,
  lastElement,
  render,
  runtimeApi,
  screen,
  trajectoryApi,
  waitFor,
  within
} from './floating-windows.fixtures';

describe('ApplicationLogsPage - floating windows shell', () => {
  test('uses the task invocation count for both the summary and count column', async () => {
    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );

    const taskRow = (await screen.findByText('公开 API 退款总结')).closest(
      'tr'
    )!;
    expect(
      screen.getByRole('columnheader', { name: '请求次数' })
    ).toBeInTheDocument();
    expect(within(taskRow).getByText('3 次调用')).toBeInTheDocument();
    expect(within(taskRow).getAllByRole('cell', { name: '3' })).toHaveLength(2);
    expect(
      within(taskRow).queryByRole('cell', { name: '2' })
    ).not.toBeInTheDocument();
  });

  test('opens run detail and conversation log as floating windows', async () => {
    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    expect(screen.getByText('公开 API 退款总结')).toBeInTheDocument();
    expect(screen.getByText('customer-42')).toBeInTheDocument();
    expect(screen.getByText('应用 API Key · key-1')).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', {
        name: '协议'
      })
    ).toBeInTheDocument();
    expect(screen.getByText('OpenAI Responses')).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', {
        name: 'expand_id'
      })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '总 tokens' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '真实节点数' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '请求次数' })
    ).toBeInTheDocument();
    expect(screen.getByText('50')).toBeInTheDocument();
    const taskRow = screen.getByText('公开 API 退款总结').closest('tr')!;
    expect(within(taskRow).getByText('3 次调用')).toBeInTheDocument();
    expect(within(taskRow).getAllByRole('cell', { name: '3' })).toHaveLength(2);
    expect(
      within(taskRow).queryByRole('cell', { name: '2' })
    ).not.toBeInTheDocument();
    expect(runtimeApi.fetchApplicationRuns).toHaveBeenCalledWith('app-1', {
      page: 1,
      pageSize: 20,
      timeRangeDays: 7,
      sortBy: 'started_at',
      sortOrder: 'desc'
    });
    expect(
      screen.queryByRole('complementary', { name: '运行详情' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('dialog', { name: '运行详情' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('application-logs-floating-run-detail')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('application-logs-splitter')
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('application-logs-page')).not.toHaveClass(
      'application-logs-page--detail-open'
    );
    expect(screen.getByTestId('application-logs-list')).not.toHaveAttribute(
      'style'
    );

    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    await waitFor(() => {
      expect(
        runtimeApi.fetchApplicationRunConversationMessages
      ).toHaveBeenCalledWith('app-1', 'run-1', { limit: 5 });
    });
    const detailPane = await screen.findByRole('complementary', {
      name: '运行详情'
    });
    expect(detailPane).toBeInTheDocument();
    // AC-001: the detail subtitle only exposes the run ID and copy action.
    const runIdSubtitle = detailPane.querySelector(
      '.application-run-detail__run-id'
    );
    expect(runIdSubtitle).not.toBeNull();
    expect(runIdSubtitle).toHaveTextContent(/^run-1$/);
    expect(
      within(runIdSubtitle as HTMLElement).getByRole('button', {
        name: '复制运行 ID'
      })
    ).toBeInTheDocument();
    expect(
      screen.queryByTestId('application-run-detail-meta')
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('dialog', { name: '运行详情' })
    ).toBeInTheDocument();
    expect(screen.getAllByRole('table').length).toBeGreaterThan(0);
    expect(
      screen.queryByRole('button', { name: '返回日志' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByTestId('application-logs-splitter')
    ).not.toBeInTheDocument();
    expect(screen.getByTestId('application-logs-page')).not.toHaveClass(
      'application-logs-page--detail-open'
    );
    expect(screen.getByTestId('application-logs-list')).not.toHaveAttribute(
      'style'
    );
    expect(
      screen.getByTestId('application-logs-floating-run-detail')
    ).toBeInTheDocument();
    expect(
      within(
        screen.getByTestId('application-logs-floating-run-detail')
      ).getByRole('separator', { name: '从右侧调整运行详情宽度' })
    ).toBeInTheDocument();
    expect(
      within(
        screen.getByTestId('application-logs-floating-run-detail')
      ).getByRole('separator', { name: '从左侧调整运行详情宽度' })
    ).toBeInTheDocument();
    expect(
      within(
        screen.getByTestId('application-logs-floating-run-detail')
      ).getByRole('separator', { name: '向下调整运行详情高度' })
    ).toBeInTheDocument();

    const conversation = await screen.findByTestId(
      'debug-conversation-messages'
    );
    expect(within(conversation).getByText('你是项目助手')).toBeInTheDocument();
    expect(
      within(conversation).queryByText('上一轮问题')
    ).not.toBeInTheDocument();
    expect(
      within(conversation).queryByText('上一轮回答')
    ).not.toBeInTheDocument();
    expect(within(conversation).getByText('总结退款政策')).toBeInTheDocument();
    expect(within(conversation).getByText('退款政策摘要')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '查看执行轨迹' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByPlaceholderText('和 Bot 聊天')
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '发送调试消息' })
    ).not.toBeInTheDocument();
    expect(runtimeApi.resumeFlowRun).not.toHaveBeenCalled();
    expect(runtimeApi.completeCallbackTask).not.toHaveBeenCalled();
    expect(screen.queryByText('功能已开启')).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '管理功能' })
    ).not.toBeInTheDocument();
    expect(screen.queryByText('运行摘要')).not.toBeInTheDocument();
    expect(screen.queryByText('运行输入输出')).not.toBeInTheDocument();
    expect(screen.queryByText('事件时间线')).not.toBeInTheDocument();

    expect(
      within(detailPane).queryByRole('button', { name: /LLM.*llm/ })
    ).not.toBeInTheDocument();
    expect(
      within(detailPane).queryByLabelText('输入 JSON')
    ).not.toBeInTheDocument();

    const openLogButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看对话日志' },
        { timeout: 8_000 }
      ),
      'expected conversation log button'
    );
    fireEvent.click(openLogButton);

    const logPanel = await screen.findByRole('complementary', {
      name: '对话日志'
    });
    expect(logPanel).toBeInTheDocument();
    expect(
      screen.getByRole('dialog', { name: '对话日志' })
    ).toBeInTheDocument();
    expect(detailPane).not.toContainElement(logPanel);
    expect(detailPane).toContainElement(conversation);
    expect(
      screen.getByTestId('application-logs-floating-conversation-log')
    ).toBeInTheDocument();
    expect(within(logPanel).getByRole('tab', { name: '详情' })).toHaveAttribute(
      'aria-selected',
      'true'
    );
    expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalled();
    expect(trajectoryApi.fetchRunPayload).not.toHaveBeenCalled();
    fireEvent.click(
      await within(logPanel).findByRole('button', { name: '输出' })
    );
    expect(
      await within(logPanel).findByLabelText('输出 JSON')
    ).toHaveTextContent('退款政策摘要');
    expect(trajectoryApi.fetchRunPayload).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      'output_payload'
    );
    expect(within(logPanel).getByText('协议')).toBeInTheDocument();
    expect(within(logPanel).queryByText('节点数')).not.toBeInTheDocument();

    fireEvent.click(within(logPanel).getByRole('tab', { name: '追踪' }));
    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRunTraceTree).toHaveBeenCalledWith(
        'app-1',
        'run-1'
      );
    });
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).not.toHaveBeenCalled();

    fireEvent.click(
      await within(logPanel).findByRole('button', { name: /LLM.*llm/ })
    );
    const nodeDetail = await within(logPanel).findByRole('region', {
      name: 'LLM 节点详情'
    });
    expect(
      within(nodeDetail).queryByRole('button', { name: '详情' })
    ).not.toBeInTheDocument();
    await waitFor(() => {
      expect(
        runtimeApi.fetchApplicationRunTraceNodeContent
      ).toHaveBeenCalledWith('app-1', 'run-1', 'node_run:node-run-1');
    });
    await waitFor(() => {
      expect(
        within(logPanel)
          .getAllByLabelText('输入 JSON')
          .some((element) => element.textContent?.includes('总结退款政策'))
      ).toBe(true);
    });

    fireEvent.click(screen.getByRole('button', { name: '关闭运行详情' }));

    expect(
      screen.queryByRole('complementary', { name: '运行详情' })
    ).not.toBeInTheDocument();
  }, 40_000);

  test('refreshes a waiting callback conversation log until its reply arrives', async () => {
    runtimeApi.fetchApplicationRuns.mockReset();
    runtimeApi.fetchApplicationConversationMessages.mockReset();
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-active',
          application_id: 'app-1',
          scope_id: 'workspace-1',
          run_mode: 'published_api_run' as const,
          execution_stage: 'published' as const,
          invocation_source: 'agent_flow_api' as const,
          principal: {
            kind: 'application_api_key' as const,
            id: 'key-active',
            display_name: null
          },
          status: 'waiting_callback',
          target_node_id: 'node-llm',
          title: '公开 API 工具调用',
          expand_id: 'customer-42',
          authorized_display_name: 'root',
          compatibility_mode: 'openai-chat-completions-v1',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: null,
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
    runtimeApi.fetchApplicationRunConversationMessages
      .mockResolvedValueOnce(
        conversationMessagesPage([
          {
            id: 'msg-run-active-user',
            flow_run_id: 'run-active',
            role: 'user',
            content: '读取 README',
            sequence: 1,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: null,
            status: 'waiting_callback'
          },
          {
            id: 'msg-run-active-assistant',
            flow_run_id: 'run-active',
            role: 'assistant',
            content: '等待工具结果',
            sequence: 2,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: null,
            status: 'waiting_callback'
          }
        ])
      )
      .mockResolvedValue(
        conversationMessagesPage([
          {
            id: 'msg-run-active-user',
            flow_run_id: 'run-active',
            role: 'user',
            content: '读取 README',
            sequence: 1,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: '2026-04-17T09:00:05Z',
            status: 'succeeded'
          },
          {
            id: 'msg-run-active-assistant',
            flow_run_id: 'run-active',
            role: 'assistant',
            content: '最终回答',
            sequence: 2,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: '2026-04-17T09:00:05Z',
            status: 'succeeded'
          }
        ])
      );

    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );

    expect(await screen.findByText('公开 API 工具调用')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    await waitFor(() => {
      expect(
        runtimeApi.fetchApplicationRunConversationMessages.mock.calls.length
      ).toBe(1);
    });
    expect(await screen.findByText('等待工具结果')).toBeInTheDocument();
    await act(async () => {
      await new Promise((resolve) => window.setTimeout(resolve, 1200));
    });
    expect(await screen.findByText('最终回答')).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationRunConversationMessages.mock.calls.length
    ).toBeGreaterThan(1);
  }, 8_000);
});
