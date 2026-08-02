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

import type { ConsoleApplicationRunDetail as ApplicationRunDetail } from '@1flowbase/api-client';
import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../../shared/i18n/app-i18n';
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

describe('ApplicationLogsPage - artifacts trace overview', () => {
  let currentRunDetail: ApplicationRunDetail;
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
          authorized_account: 'root',
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
      flow_run: sampleRunDetail().flow_run,
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

  test('loads conversation log detail and trace artifacts from application logs', async () => {
    const detail = sampleRunDetail();
    detail.flow_run.output_payload = {
      answer: {
        __runtime_debug_artifact: true,
        artifact_scope: 'field',
        field_path: ['answer'],
        is_truncated: true,
        original_size_bytes: 4096,
        preview_size_bytes: 128,
        content_type: 'application/json',
        artifact_ref: 'artifact-detail-answer',
        preview: '详情截断回答'
      }
    };
    detail.node_runs[0]!.output_payload = {
      answer: {
        __runtime_debug_artifact: true,
        artifact_scope: 'field',
        field_path: ['answer'],
        is_truncated: true,
        original_size_bytes: 4096,
        preview_size_bytes: 128,
        content_type: 'application/json',
        artifact_ref: 'artifact-trace-answer',
        preview: '追踪截断回答'
      },
      rendered_templates: {}
    };
    currentRunDetail = detail;
    runtimeApi.fetchRuntimeDebugArtifacts.mockImplementation(
      async (_applicationId: string, artifactRefs: string[]) => ({
        artifacts: artifactRefs.map((artifactRef) => {
          if (artifactRef === 'artifact-detail-answer') {
            return {
              artifact_ref: artifactRef,
              content_type: 'application/json',
              value: '详情完整回答'
            };
          }

          if (artifactRef === 'artifact-trace-answer') {
            return {
              artifact_ref: artifactRef,
              content_type: 'application/json',
              value: '追踪完整回答'
            };
          }

          throw new Error(`unexpected artifact: ${artifactRef}`);
        })
      })
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));

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
    expect(
      within(logPanel).queryByRole('button', {
        name: '加载完整值'
      })
    ).not.toBeInTheDocument();
    expect(runtimeApi.fetchRuntimeDebugArtifacts).not.toHaveBeenCalledWith(
      'app-1',
      ['artifact-detail-answer']
    );

    fireEvent.click(within(logPanel).getByRole('tab', { name: '追踪' }));
    fireEvent.click(
      await within(logPanel).findByRole('button', { name: /LLM/ })
    );
    const nodeDetail = await openLazyLlmNodeDetail(logPanel);
    await waitFor(() =>
      expect(runtimeApi.fetchApplicationRunTraceNodeContent).toHaveBeenCalled()
    );
    await waitFor(() =>
      expect(within(nodeDetail).getByLabelText('输出 JSON')).toHaveTextContent(
        '追踪截断回答'
      )
    );
    const traceLoadButton = await within(nodeDetail).findByRole('button', {
      name: '加载完整值'
    });
    expect(traceLoadButton).toBeEnabled();
    fireEvent.click(traceLoadButton);

    expect(runtimeApi.fetchRuntimeDebugArtifacts).toHaveBeenCalledWith(
      'app-1',
      ['artifact-trace-answer']
    );
    await waitFor(() =>
      expect(
        within(logPanel)
          .getAllByLabelText('输出 JSON')
          .some((element) => element.textContent?.includes('追踪完整回答'))
      ).toBe(true)
    );
  }, 20_000);

  test('keeps prior conversation context while opening the selected run log', async () => {
    const priorRunDetail = sampleRunDetail();
    priorRunDetail.node_runs[0]!.debug_payload = {
      tool_callbacks: [
        {
          id: 'call-problem-review',
          name: 'problem_review',
          callback_status: 'returned',
          execution_status: 'succeeded',
          request_round_index: 0,
          result_round_index: 1,
          duration_ms: 1500,
          detail_ref: 'call-problem-review'
        }
      ]
    };
    priorRunDetail.statistics = {
      count_tokens_input_tokens: null,
      total_tokens: 4213,
      input_tokens: 3414,
      output_tokens: 799,
      input_cache_hit_tokens: 37376,
      input_cache_hit_rate: null,
      unique_node_count: 3,
      tool_callback_count: 1
    };
    const currentRunDetail = sampleRunDetail();
    currentRunDetail.run!.id = 'run-2';
    currentRunDetail.flow_run.id = 'run-2';
    currentRunDetail.flow_run.title = '回来后 recap';
    currentRunDetail.node_runs[0]!.flow_run_id = 'run-2';
    currentRunDetail.statistics = {
      count_tokens_input_tokens: null,
      total_tokens: 3843,
      input_tokens: 3353,
      output_tokens: 490,
      input_cache_hit_tokens: 38336,
      input_cache_hit_rate: null,
      unique_node_count: 3,
      tool_callback_count: 0
    };
    const detailsByRunId = new Map([
      ['run-1', priorRunDetail],
      ['run-2', currentRunDetail]
    ]);
    const detailForRun = (runId: string) => {
      const detail = detailsByRunId.get(runId);

      if (!detail) {
        throw new Error(`unexpected run: ${runId}`);
      }

      return detail;
    };

    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-2',
          run_mode: 'published_api_run' as const,
          status: 'succeeded',
          target_node_id: 'node-llm',
          title: '回来后 recap',
          external_conversation_id: 'conversation-1',
          started_at: '2026-04-17T09:01:00Z',
          finished_at: '2026-04-17T09:01:01Z',
          created_at: '2026-04-17T09:01:00Z',
          updated_at: '2026-04-17T09:01:01Z'
        }
      ])
    );
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue({
      items: [
        {
          run_id: 'run-2:context:0',
          detail_run_id: null,
          can_open_detail: false,
          role: 'user',
          content: '调用工具 problem_review',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          status: 'succeeded',
          query: null,
          model: null,
          answer: null,
          is_current: false
        },
        {
          run_id: 'run-2:context:1',
          detail_run_id: null,
          can_open_detail: false,
          role: 'assistant',
          content: '上一轮调用了 problem_review',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          status: 'succeeded',
          query: null,
          model: null,
          answer: null,
          is_current: false
        },
        {
          run_id: 'run-2:context:2',
          detail_run_id: null,
          can_open_detail: false,
          role: 'system',
          content: '当前 run 系统提示词',
          started_at: '2026-04-17T09:01:00Z',
          finished_at: '2026-04-17T09:01:01Z',
          status: 'succeeded',
          query: null,
          model: null,
          answer: null,
          is_current: true
        },
        {
          run_id: 'run-2',
          detail_run_id: 'run-2',
          can_open_detail: true,
          role: null,
          content: null,
          started_at: '2026-04-17T09:01:00Z',
          finished_at: '2026-04-17T09:01:01Z',
          status: 'succeeded',
          query: null,
          model: null,
          answer: '回来后 recap',
          is_current: true
        }
      ],
      page: {
        has_before: false,
        has_after: false,
        before_cursor: null,
        after_cursor: null
      }
    });
    runtimeApi.fetchApplicationRunTraceTree.mockImplementation(
      async (_applicationId: string, runId: string) =>
        traceTreeFromDetail(detailForRun(runId))
    );
    runtimeApi.fetchApplicationRunOverview.mockImplementation(
      async (_applicationId: string, runId: string) =>
        runOverviewFromDetail(detailForRun(runId))
    );
    runtimeApi.fetchApplicationRunTraceNodeContent.mockImplementation(
      async (_applicationId: string, runId: string, traceNodeId: string) =>
        traceNodeContentFromDetail(detailForRun(runId), traceNodeId)
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-2')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '查看运行详情' }));
    await waitFor(() =>
      expect(
        runtimeApi.fetchApplicationRunConversationMessages
      ).toHaveBeenCalledWith('app-1', 'run-2', {
        limit: 5
      })
    );

    const conversationMessages = await screen.findByTestId(
      'debug-conversation-messages'
    );
    expect(
      within(conversationMessages).getByText('调用工具 problem_review')
    ).toBeInTheDocument();
    expect(
      within(conversationMessages).getByText('上一轮调用了 problem_review')
    ).toBeInTheDocument();
    expect(
      within(conversationMessages).getByText('当前 run 系统提示词')
    ).toBeInTheDocument();

    const logButtons = await within(conversationMessages).findAllByRole(
      'button',
      {
        name: '查看对话日志'
      }
    );
    expect(logButtons).toHaveLength(1);
    fireEvent.click(logButtons[0]!);

    const logPanel = await screen.findByRole('complementary', {
      name: '对话日志'
    });
    fireEvent.click(within(logPanel).getByRole('tab', { name: '追踪' }));
    await waitFor(() =>
      expect(runtimeApi.fetchApplicationRunTraceTree).toHaveBeenCalledWith(
        'app-1',
        'run-2'
      )
    );
    expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalledWith(
      'app-1',
      'run-1'
    );
  }, 20_000);
});
