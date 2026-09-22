import { configureExecutionProjection } from './trajectory/projection';
import '../../../agent-flow/_tests/debug-console/trajectory/navigation';
import { App as AntdApp } from 'antd';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { vi } from 'vitest';

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
  applicationLogConversationMessagesQueryKey: (
    applicationId: string,
    externalConversationId: string,
    input?: {
      aroundRunId?: string | null;
      before?: string | null;
      after?: string | null;
      limit?: number;
    }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'logs',
      'conversations',
      externalConversationId,
      input?.aroundRunId ?? '',
      input?.before ?? '',
      input?.after ?? '',
      input?.limit ?? 5
    ] as const,
  fetchApplicationRuns: vi.fn(),
  fetchApplicationRunOverview: vi.fn(),
  fetchApplicationRunTraceTree: vi.fn(),
  fetchApplicationRunTraceNodeChildren: vi.fn(),
  fetchApplicationRunTraceNodeContent: vi.fn(),
  fetchApplicationRunTraceNodeDetail: vi.fn(),
  fetchApplicationRunResumeTimeline: vi.fn(),
  fetchApplicationConversationMessages: vi.fn(),
  fetchApplicationLogConversationMessages: vi.fn(),
  fetchApplicationRunConversationMessages: vi.fn(),
  fetchRuntimeDebugArtifact: vi.fn(),
  fetchRuntimeDebugArtifacts: vi.fn(),
  exportApplicationRunTraceDump: vi.fn(),
  exportSelectedApplicationRunsTraceDumpZip: vi.fn(),
  resumeFlowRun: vi.fn(),
  completeCallbackTask: vi.fn()
}));

vi.mock('../../api/runtime', () => runtimeApi);
vi.mock('../../api/trajectory', () => ({
  fetchWorkflowTrajectory: vi.fn().mockResolvedValue({
    items: [],
    next_cursor: null,
    nodes: [],
    time_start: null,
    time_end: null
  }),
  fetchProviderTrajectoryBody: vi.fn()
}));

import type { ConsoleApplicationRunDetail as ApplicationRunDetail } from '@1flowbase/api-client';
import { AppProviders } from '../../../../app/AppProviders';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { resetAuthStore } from '../../../../state/auth-store';
import { ApplicationLogsPage } from '../../pages/ApplicationLogsPage';
import {
  applicationRunsPage,
  conversationMessagesPage,
  lastElement,
  openLazyLlmNodeDetail,
  runOverviewFromDetail,
  sampleRunDetail,
  traceNodeContentFromDetail,
  traceTreeFromDetail
} from './artifacts-trace.support';

describe('ApplicationLogsPage - artifacts trace floating detail', () => {
  let currentRunDetail: ApplicationRunDetail;
  let getBoundingClientRectSpy: { mockRestore: () => void } | undefined;
  let innerHeightSpy: { mockRestore: () => void } | undefined;
  let innerWidthSpy: { mockRestore: () => void } | undefined;
  let dateNowSpy: { mockRestore: () => void } | undefined;

  beforeEach(async () => {
    window.localStorage.clear();
    window.history.replaceState({}, '', '/applications/app-1/logs');
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await loadApplicationI18nResources();
    await appI18n.changeLanguage('zh_Hans');
    dateNowSpy = vi
      .spyOn(Date, 'now')
      .mockReturnValue(new Date('2026-04-18T00:00:00Z').getTime());
    runtimeApi.fetchApplicationRuns.mockReset();
    runtimeApi.fetchApplicationRunOverview.mockReset();
    runtimeApi.fetchApplicationRunTraceTree.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeContent.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeDetail.mockReset();
    runtimeApi.fetchApplicationRunResumeTimeline.mockReset();
    runtimeApi.fetchApplicationConversationMessages.mockReset();
    runtimeApi.fetchApplicationLogConversationMessages.mockReset();
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchRuntimeDebugArtifact.mockReset();
    runtimeApi.fetchRuntimeDebugArtifacts.mockReset();
    currentRunDetail = sampleRunDetail();

    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-1',
          run_mode: 'published_api_run' as const,
          status: 'succeeded',
          target_node_id: 'node-llm',
          title: '公开 API 退款总结',
          expand_id: 'customer-42',
          authorized_display_name: 'root',
          compatibility_mode: 'openai-responses-v1',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
    runtimeApi.fetchApplicationRunTraceTree.mockImplementation(async () =>
      traceTreeFromDetail(currentRunDetail)
    );
    runtimeApi.fetchApplicationRunOverview.mockImplementation(async () =>
      runOverviewFromDetail(currentRunDetail)
    );
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockResolvedValue({
      items: [],
      page_info: {
        has_more: false,
        next_cursor: null,
        page_size: 20
      }
    });
    runtimeApi.fetchApplicationRunTraceNodeContent.mockImplementation(
      async (_applicationId: string, _runId: string, traceNodeId: string) =>
        traceNodeContentFromDetail(currentRunDetail, traceNodeId)
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

  async function openTrace() {
    render(
      <AppProviders>
        <AntdApp>
          <ApplicationLogsPage applicationId="app-1" />
        </AntdApp>
      </AppProviders>
    );
    await screen.findByText('run-1');
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));
    fireEvent.click(
      lastElement(
        await screen.findAllByRole('button', { name: '查看对话日志' }),
        'conversation log entry'
      )
    );
    const panel = await screen.findByRole('complementary', {
      name: '对话日志'
    });
    fireEvent.click(within(panel).getByRole('tab', { name: '追踪' }));
    const llm = await within(panel).findAllByRole('button', { name: /LLM/ });
    return { panel, llm };
  }

  async function openTool(execution: HTMLElement, name = 'lookup_weather') {
    const tools = await within(execution).findByRole('button', {
      name: /Tools/
    });
    expect(tools).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(tools);
    const tool = await within(execution).findByRole('button', {
      name: new RegExp(name)
    });
    return { tools, tool };
  }

  test('inspects real fusion branches in the workflow tree without eagerly loading branch bodies', async () => {
    configureExecutionProjection(runtimeApi, { mode: 'fusion' });
    const { panel, llm } = await openTrace();
    fireEvent.click(llm[0]!);
    const execution = await openLazyLlmNodeDetail(panel);
    expect(
      screen.queryByRole('dialog', { name: '调用轨迹' })
    ).not.toBeInTheDocument();
    const { tool } = await openTool(execution);
    expect(tool).toHaveTextContent('fusion');
    fireEvent.click(tool);
    const risk = await within(execution).findByRole('button', {
      name: /Risk Panel/
    });
    expect(
      await within(execution).findByRole('button', { name: /Support Panel/ })
    ).toBeInTheDocument();
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).not.toHaveBeenCalledWith('app-1', 'run-1', 'branch-1-1');
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).not.toHaveBeenCalledWith('app-1', 'run-1', 'branch-1-2');
    fireEvent.click(risk);
    const detail = await within(execution).findByRole('region', {
      name: 'Risk Panel 节点详情'
    });
    expect(
      runtimeApi.fetchApplicationRunTraceNodeDetail
    ).not.toHaveBeenCalled();
    expect(await within(detail).findByLabelText('输入 JSON')).toHaveTextContent(
      'Risk Panel input'
    );
    expect(runtimeApi.fetchApplicationRunTraceNodeContent).toHaveBeenCalledWith(
      'app-1',
      'run-1',
      'branch-1-1'
    );
    expect(
      await within(detail).findByLabelText('数据处理 JSON')
    ).toHaveTextContent('recorded-provider');
    expect(
      within(detail).queryByRole('button', { name: '调用轨迹' })
    ).not.toBeInTheDocument();
    expect(
      within(detail).getByRole('button', { name: /branch_policy_lookup/ })
    ).toBeInTheDocument();
    expect(
      within(detail).getByLabelText('数据处理 JSON')
    ).not.toHaveTextContent('branch_policy_lookup');
    expect(await within(detail).findByLabelText('输出 JSON')).toHaveTextContent(
      'Risk Panel result'
    );
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).not.toHaveBeenCalledWith('app-1', 'run-1', 'branch-1-2');
  });

  test('retains loaded execution details and artifacts across floating window activation', async () => {
    configureExecutionProjection(runtimeApi, { artifact: true });
    runtimeApi.fetchRuntimeDebugArtifacts.mockResolvedValue({
      artifacts: [
        {
          artifact_ref: 'artifact-branch-output',
          value: { text: 'retained branch result' }
        }
      ]
    });
    const { panel, llm } = await openTrace();
    fireEvent.click(llm[0]!);
    const execution = await openLazyLlmNodeDetail(panel);
    expect(
      screen.queryByRole('dialog', { name: '调用轨迹' })
    ).not.toBeInTheDocument();
    const { tool, tools } = await openTool(execution);
    fireEvent.click(tool);
    fireEvent.click(
      await within(execution).findByRole('button', { name: /Image LLM/ })
    );
    const detail = await within(execution).findByRole('region', {
      name: 'Image LLM 节点详情'
    });
    expect(runtimeApi.fetchRuntimeDebugArtifacts).not.toHaveBeenCalled();
    fireEvent.click(
      await within(detail).findByRole('button', { name: '加载完整值' })
    );
    await waitFor(() =>
      expect(
        runtimeApi.fetchRuntimeDebugArtifacts
      ).toHaveBeenCalledExactlyOnceWith('app-1', ['artifact-branch-output'])
    );
    await waitFor(() =>
      expect(within(detail).getByLabelText('输出 JSON')).toHaveTextContent(
        'retained branch result'
      )
    );
    const contentCalls =
      runtimeApi.fetchApplicationRunTraceNodeContent.mock.calls.length;
    fireEvent.mouseDown(
      screen.getByTestId('application-logs-floating-run-detail')
    );
    expect(tools).toHaveAttribute('aria-expanded', 'true');
    expect(tool).toHaveAttribute('aria-expanded', 'true');
    expect(within(detail).getByLabelText('输出 JSON')).toHaveTextContent(
      'retained branch result'
    );
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).toHaveBeenCalledTimes(contentCalls);
    expect(runtimeApi.fetchRuntimeDebugArtifacts).toHaveBeenCalledTimes(1);
  });
});
