import { Grid } from 'antd';

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
  applicationLogConversationMessagesQueryKey: (
    applicationId: string,
    conversationId: string
  ) =>
    [
      'applications',
      applicationId,
      'log-conversation',
      conversationId
    ] as const,
  fetchApplicationLogConversationMessages: vi.fn(),
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
          message_id: item.id,
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

const trajectoryApi = vi.hoisted(() => ({
  fetchRunPayload: vi.fn()
}));

vi.mock('../../../api/trajectory', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../../api/trajectory')>()),
  fetchRunPayload: trajectoryApi.fetchRunPayload
}));

vi.mock('../../../api/runtime', () => runtimeApi);

import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../../shared/i18n/app-i18n';
import { resetAuthStore } from '../../../../../state/auth-store';

import {
  applicationRunsPage,
  conversationMessagesPage,
  sampleRunDetail,
  sampleRunOverview,
  sampleTraceNodeContent,
  sampleTraceTree
} from './floating-windows.support';

let getBoundingClientRectSpy: { mockRestore: () => void } | undefined;
let innerHeightSpy: { mockRestore: () => void } | undefined;
let innerWidthSpy: { mockRestore: () => void } | undefined;
let dateNowSpy: { mockRestore: () => void } | undefined;
let useBreakpointSpy: { mockRestore: () => void } | undefined;

beforeEach(async () => {
  // These scenarios assert desktop table rows and columns.
  useBreakpointSpy = vi.spyOn(Grid, 'useBreakpoint').mockReturnValue({
    xs: false,
    sm: true,
    md: true,
    lg: true,
    xl: false,
    xxl: false
  });
  window.history.replaceState({}, '', '/applications/app-1/logs');
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
  runtimeApi.fetchApplicationRunResumeTimeline.mockReset();
  runtimeApi.fetchApplicationConversationMessages.mockReset();
  runtimeApi.fetchApplicationRunConversationMessages.mockReset();
  runtimeApi.fetchRuntimeDebugArtifact.mockReset();
  trajectoryApi.fetchRunPayload.mockReset();
  trajectoryApi.fetchRunPayload.mockImplementation(
    async (
      _applicationId: string,
      _runId: string,
      section: 'input_payload' | 'output_payload'
    ) => sampleRunDetail().flow_run[section]
  );

  runtimeApi.fetchApplicationRuns.mockResolvedValue(
    applicationRunsPage([
      {
        id: 'run-1',
        application_id: 'app-1',
        scope_id: 'workspace-1',
        run_mode: 'published_api_run' as const,
        execution_stage: 'published' as const,
        invocation_source: 'agent_flow_api' as const,
        principal: {
          kind: 'application_api_key' as const,
          id: 'key-1',
          display_name: null
        },
        status: 'succeeded',
        target_node_id: 'node-llm',
        title: '公开 API 退款总结',
        expand_id: 'customer-42',
        authorized_display_name: 'root',
        compatibility_mode: 'openai-responses-v1',
        total_tokens: 50,
        input_tokens: 40,
        output_tokens: 10,
        input_cache_hit_tokens: 12,
        unique_node_count: 3,
        invocation_count: 3,
        tool_callback_count: 2,
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z',
        created_at: '2026-04-17T09:00:00Z',
        updated_at: '2026-04-17T09:00:01Z'
      }
    ])
  );
  runtimeApi.fetchApplicationRunTraceTree.mockResolvedValue(sampleTraceTree());
  runtimeApi.fetchApplicationRunOverview.mockResolvedValue(sampleRunOverview());
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
  useBreakpointSpy?.mockRestore();
  useBreakpointSpy = undefined;
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

export function mockViewportSize(width: number, height: number) {
  innerWidthSpy = vi.spyOn(window, 'innerWidth', 'get').mockReturnValue(width);
  innerHeightSpy = vi
    .spyOn(window, 'innerHeight', 'get')
    .mockReturnValue(height);
}

export { runtimeApi, trajectoryApi };
