import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { vi } from 'vitest';

type ConversationMessagePageItem = {
  id: string;
  flow_run_id: string | null;
  role: 'system' | 'user' | 'assistant';
  content: string;
  sequence: number;
  status?: string;
  started_at?: string | null;
  finished_at?: string | null;
};

const runtimeApi = vi.hoisted(() => ({
  applicationRunsQueryKey: (
    applicationId: string,
    input?: {
      page?: number;
      pageSize?: number;
      timeRangeDays?: number | null;
      sortBy?: 'started_at' | 'finished_at' | 'created_at';
      sortOrder?: 'asc' | 'desc';
      cacheMode?: 'default' | 'refresh';
    }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      input?.page ?? 1,
      input?.pageSize ?? 20,
      input?.timeRangeDays ?? 'all',
      input?.sortBy ?? 'started_at',
      input?.sortOrder ?? 'desc'
    ] as const,
  applicationRunTraceTreeQueryKey: (applicationId: string, runId: string) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'trace-tree'
    ] as const,
  applicationRunOverviewQueryKey: (applicationId: string, runId: string) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'overview'
    ] as const,
  applicationRunTraceNodeChildrenQueryKey: (
    applicationId: string,
    runId: string,
    traceNodeId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'trace-tree',
      traceNodeId,
      'children'
    ] as const,
  applicationRunTraceNodeContentQueryKey: (
    applicationId: string,
    runId: string,
    traceNodeId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'trace-tree',
      traceNodeId,
      'content'
    ] as const,
  applicationRunResumeTimelineQueryKey: (
    applicationId: string,
    runId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'resume-timeline'
    ] as const,
  applicationConversationMessagesQueryKey: (
    applicationId: string,
    runId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'conversation',
      'around',
      runId
    ] as const,
  applicationRunConversationMessagesQueryKey: (
    applicationId: string,
    runId: string
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'runs',
      runId,
      'conversation-messages'
    ] as const,
  fetchApplicationRuns: vi.fn(),
  fetchApplicationRunOverview: vi.fn(),
  fetchApplicationRunTraceTree: vi.fn(),
  fetchApplicationRunTraceNodeChildren: vi.fn(),
  fetchApplicationRunTraceNodeContent: vi.fn(),
  fetchApplicationRunResumeTimeline: vi.fn(),
  fetchApplicationConversationMessages: vi.fn(),
  fetchApplicationRunConversationMessages: vi.fn().mockImplementation(
    async (
      appId: string,
      runId: string,
      options?: {
        limit?: number;
      }
    ) => {
      const rawPage = await runtimeApi.fetchApplicationConversationMessages(
        appId,
        {
          flowRunId: runId,
          page: 1,
          pageSize: options?.limit ?? 5
        }
      );
      return {
        items: rawPage.items.map((item: ConversationMessagePageItem) => ({
          ...item,
          run_id: item.flow_run_id ?? `message:${item.id}`,
          detail_run_id: item.flow_run_id,
          can_open_detail: item.flow_run_id === 'run-0',
          status: item.status ?? 'succeeded'
        })),
        page: {
          has_before: false,
          has_after: false,
          before_cursor: null,
          after_cursor: null
        }
      };
    }
  ),
  fetchRuntimeDebugArtifact: vi.fn(),
  fetchRuntimeDebugArtifacts: vi.fn(),
  exportApplicationRunTraceDump: vi.fn(),
  exportSelectedApplicationRunsTraceDumpZip: vi.fn(),
  resumeFlowRun: vi.fn(),
  completeCallbackTask: vi.fn()
}));

vi.mock('../../api/runtime', () => runtimeApi);

import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { resetAuthStore } from '../../../../state/auth-store';
import { ApplicationLogsPage } from '../../pages/ApplicationLogsPage';

import {
  applicationRunsPage,
  conversationMessagesPage,
  lastElement,
  sampleRunDetail,
  sampleRunOverview,
  sampleTraceNodeContent,
  sampleTraceTree
} from './floating-windows.support';

describe('ApplicationLogsPage - floating windows timeline', () => {
  let getBoundingClientRectSpy: { mockRestore: () => void } | undefined;
  let innerHeightSpy: { mockRestore: () => void } | undefined;
  let innerWidthSpy: { mockRestore: () => void } | undefined;
  let dateNowSpy: { mockRestore: () => void } | undefined;

  beforeEach(async () => {
    window.localStorage.clear();
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    dateNowSpy = vi
      .spyOn(Date, 'now')
      .mockReturnValue(new Date('2026-04-18T00:00:00Z').getTime());
    runtimeApi.fetchApplicationRuns.mockReset();
    runtimeApi.fetchApplicationRunOverview.mockReset();
    runtimeApi.fetchApplicationRunTraceTree.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeContent.mockReset();
    runtimeApi.fetchApplicationRunResumeTimeline.mockReset();
    runtimeApi.fetchApplicationConversationMessages.mockReset();
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchRuntimeDebugArtifact.mockReset();

    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-1',
          run_mode: 'published_api_run' as const,
          status: 'succeeded',
          target_node_id: 'node-llm',
          title: '公开 API 退款总结',
          expand_id: 'customer-42',
          authorized_account: 'root',
          compatibility_mode: 'openai-responses-v1',
          total_tokens: 50,
          input_tokens: 40,
          output_tokens: 10,
          input_cache_hit_tokens: 12,
          unique_node_count: 3,
          tool_callback_count: 20,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
    runtimeApi.fetchApplicationRunTraceTree.mockResolvedValue(
      sampleTraceTree()
    );
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue(
      sampleRunOverview()
    );
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockResolvedValue({
      items: [],
      page_info: {
        has_more: false,
        next_cursor: null,
        page_size: 20
      }
    });
    runtimeApi.fetchApplicationRunTraceNodeContent.mockResolvedValue(
      sampleTraceNodeContent()
    );
    runtimeApi.fetchApplicationRunResumeTimeline.mockResolvedValue({
      flow_run_status: sampleRunDetail().flow_run.status,
      callback_tasks: sampleRunDetail().callback_tasks,
      events: sampleRunDetail().events
    });
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([
        {
          id: 'msg-history-system',
          flow_run_id: null,
          role: 'system',
          content: '你是项目助手',
          sequence: 1,
          started_at: '2026-04-17T08:59:00Z',
          finished_at: '2026-04-17T08:59:01Z'
        },
        {
          id: 'msg-run-1-user',
          flow_run_id: 'run-1',
          role: 'user',
          content: '总结退款政策',
          sequence: 2,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        },
        {
          id: 'msg-run-1-assistant',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '退款政策摘要',
          sequence: 3,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
  });

  afterEach(() => {
    resetAuthStore();
    getBoundingClientRectSpy?.mockRestore();
    getBoundingClientRectSpy = undefined;
    innerHeightSpy?.mockRestore();
    innerHeightSpy = undefined;
    innerWidthSpy?.mockRestore();
    innerWidthSpy = undefined;
    dateNowSpy?.mockRestore();
    dateNowSpy = undefined;
  });

  test('lets a floating window move past the viewport bottom while keeping its header reachable', async () => {
    innerWidthSpy = vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1280);
    innerHeightSpy = vi
      .spyOn(window, 'innerHeight', 'get')
      .mockReturnValue(900);

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const detailWindow = await screen.findByTestId(
      'application-logs-floating-run-detail'
    );
    expect(detailWindow).toHaveStyle({
      top: '112px',
      height: '720px'
    });

    fireEvent.mouseDown(within(detailWindow).getByText('运行详情'), {
      button: 0,
      clientX: 980,
      clientY: 130
    });
    fireEvent.mouseMove(window, {
      clientX: 980,
      clientY: 1100
    });
    fireEvent.mouseUp(window);

    expect(detailWindow).toHaveStyle({
      left: '888px',
      top: '852px',
      height: '720px'
    });
  }, 20_000);

  test('lets opened floating windows move independently after initial placement', async () => {
    innerWidthSpy = vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1280);
    innerHeightSpy = vi
      .spyOn(window, 'innerHeight', 'get')
      .mockReturnValue(900);

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const runDetailWindow = await screen.findByTestId(
      'application-logs-floating-run-detail'
    );
    const openLogButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看对话日志' },
        { timeout: 8_000 }
      ),
      'expected conversation log button'
    );
    fireEvent.click(openLogButton);

    const conversationLogWindow = await screen.findByTestId(
      'application-logs-floating-conversation-log'
    );

    expect(runDetailWindow).toHaveStyle({
      left: '888px',
      top: '112px',
      width: '360px'
    });
    expect(conversationLogWindow).toHaveStyle({
      left: '512px',
      top: '112px',
      width: '360px'
    });

    fireEvent.mouseDown(within(conversationLogWindow).getByText('对话日志'), {
      button: 0,
      clientX: 560,
      clientY: 130
    });
    fireEvent.mouseMove(window, {
      clientX: 740,
      clientY: 170
    });
    fireEvent.mouseUp(window);

    expect(conversationLogWindow).toHaveStyle({
      left: '692px',
      top: '152px'
    });
    expect(runDetailWindow).toHaveStyle({
      left: '888px',
      top: '112px'
    });
  }, 20_000);

  test('opens resume timeline from run detail without covering existing floating windows', async () => {
    innerWidthSpy = vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(1280);
    innerHeightSpy = vi
      .spyOn(window, 'innerHeight', 'get')
      .mockReturnValue(900);
    runtimeApi.fetchApplicationRunResumeTimeline.mockResolvedValue({
      flow_run_status: sampleRunDetail().flow_run.status,
      callback_tasks: [
        {
          id: 'callback-1',
          callback_kind: 'llm_tool_calls',
          status: 'completed',
          created_at: '2026-04-17T09:00:01Z',
          completed_at: '2026-04-17T09:00:02Z'
        }
      ],
      events: [
        ...sampleRunDetail().events,
        {
          id: 'event-resume-requested',
          event_type: 'public_run_resume_requested',
          description: 'callback-1',
          created_at: '2026-04-17T09:00:01Z'
        }
      ]
    });

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const runDetailWindow = await screen.findByTestId(
      'application-logs-floating-run-detail'
    );
    const openLogButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看对话日志' },
        { timeout: 8_000 }
      ),
      'expected conversation log button'
    );
    fireEvent.click(openLogButton);

    const conversationLogWindow = await screen.findByTestId(
      'application-logs-floating-conversation-log'
    );
    expect(conversationLogWindow).toHaveStyle({
      left: '512px',
      top: '112px',
      width: '360px'
    });

    const openResumeTimelineButton = lastElement(
      await screen.findAllByRole(
        'button',
        { name: '查看 Resume 时间线' },
        { timeout: 8_000 }
      ),
      'expected resume timeline button'
    );
    fireEvent.click(openResumeTimelineButton);

    const resumeTimelineWindow = await screen.findByTestId(
      'application-logs-floating-resume-timeline'
    );
    expect(runDetailWindow).toHaveStyle({
      left: '888px',
      top: '112px',
      width: '360px'
    });
    expect(conversationLogWindow).toHaveStyle({
      left: '512px',
      top: '112px',
      width: '360px'
    });
    expect(resumeTimelineWindow).toHaveStyle({
      left: '136px',
      top: '112px',
      width: '360px'
    });
    expect(
      await screen.findByRole('complementary', { name: 'Resume 时间线' })
    ).toBeInTheDocument();
    expect(screen.getByText('Resume 请求已接收')).toBeInTheDocument();
    expect(screen.getByText('工具调用回调')).toBeInTheDocument();
    expect(runtimeApi.fetchApplicationRunResumeTimeline).toHaveBeenCalledWith(
      'app-1',
      'run-1'
    );
  }, 20_000);

  test('opens resume timeline for the clicked historical conversation message run', async () => {
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue(
      conversationMessagesPage([
        {
          id: 'msg-run-0-assistant',
          flow_run_id: 'run-0',
          role: 'assistant',
          content: '上一轮回答',
          sequence: 1,
          started_at: '2026-04-17T08:59:00Z',
          finished_at: '2026-04-17T08:59:01Z'
        },
        {
          id: 'msg-run-1-assistant',
          flow_run_id: 'run-1',
          role: 'assistant',
          content: '退款政策摘要',
          sequence: 2,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z'
        }
      ])
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    const conversation = await screen.findByTestId(
      'debug-conversation-messages'
    );
    const openResumeTimelineButtons = await within(conversation).findAllByRole(
      'button',
      { name: '查看 Resume 时间线' },
      { timeout: 8_000 }
    );
    fireEvent.click(openResumeTimelineButtons[0]);

    await screen.findByTestId('application-logs-floating-resume-timeline');
    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRunResumeTimeline).toHaveBeenCalledWith(
        'app-1',
        'run-0'
      );
    });
  }, 20_000);

  test('keeps the runs table layout unchanged while floating windows are open', async () => {
    innerHeightSpy = vi
      .spyOn(window, 'innerHeight', 'get')
      .mockReturnValue(920);
    getBoundingClientRectSpy = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function getBoundingClientRect(this: HTMLElement) {
        if (this.classList.contains('application-logs-page__list')) {
          return {
            bottom: 120,
            height: 0,
            left: 0,
            right: 0,
            top: 120,
            width: 1200,
            x: 0,
            y: 120,
            toJSON: () => ({})
          };
        }

        if (this.classList.contains('ant-table-thead')) {
          return {
            bottom: 176,
            height: 56,
            left: 0,
            right: 0,
            top: 120,
            width: 900,
            x: 0,
            y: 120,
            toJSON: () => ({})
          };
        }

        if (this.classList.contains('ant-table-wrapper')) {
          return {
            bottom: 760,
            height: 640,
            left: 0,
            right: 0,
            top: 120,
            width: 900,
            x: 0,
            y: 120,
            toJSON: () => ({})
          };
        }

        return {
          bottom: 0,
          height: 0,
          left: 0,
          right: 0,
          top: 0,
          width: 0,
          x: 0,
          y: 0,
          toJSON: () => ({})
        };
      });

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

    expect(
      await screen.findByRole('complementary', { name: '运行详情' })
    ).toBeInTheDocument();
    expect(screen.getByTestId('application-logs-page')).not.toHaveClass(
      'application-logs-page--detail-open'
    );
    expect(screen.getByTestId('application-logs-list')).not.toHaveAttribute(
      'style'
    );
  });
});
