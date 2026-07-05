import { render, screen } from '@testing-library/react';
import { vi } from 'vitest';

const runtimeApi = vi.hoisted(() => ({
  applicationRunsQueryKey: (
    applicationId: string,
    input?: {
      page?: number;
      pageSize?: number;
      timeRangeDays?: number | null;
      sortBy?: 'started_at' | 'finished_at' | 'created_at' | 'updated_at';
      sortOrder?: 'asc' | 'desc';
      titleIncludes?: string;
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
      input?.sortOrder ?? 'desc',
      input?.titleIncludes ?? ''
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
    input: {
      conversationId?: string | null;
      flowRunId?: string | null;
      page?: number;
      pageSize?: number;
    }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'conversation-messages',
      input.conversationId ?? '',
      input.flowRunId ?? '',
      input.page ?? 1,
      input.pageSize ?? 5
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
  fetchApplicationRunTraceTree: vi.fn(),
  fetchApplicationRunTraceNodeChildren: vi.fn(),
  fetchApplicationRunTraceNodeContent: vi.fn(),
  fetchApplicationRunResumeTimeline: vi.fn(),
  fetchApplicationConversationMessages: vi.fn(),
  fetchApplicationRunConversationMessages: vi.fn(),
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

function applicationRunsPage<T>(
  items: T[],
  overrides?: Partial<{
    total: number;
    page: number;
    page_size: number;
  }>
) {
  return {
    items,
    total: overrides?.total ?? items.length,
    page: overrides?.page ?? 1,
    page_size: overrides?.page_size ?? 20
  };
}

describe('ApplicationLogsPage - query states', () => {
  beforeEach(async () => {
    window.localStorage.clear();
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await appI18n.changeLanguage('zh_Hans');
    runtimeApi.fetchApplicationRuns.mockReset();
    runtimeApi.fetchApplicationConversationMessages.mockReset();
    runtimeApi.fetchRuntimeDebugArtifact.mockReset();
  });

  afterEach(() => {
    resetAuthStore();
  });

  test('shows a loading state instead of a blank logs section', () => {
    runtimeApi.fetchApplicationRuns.mockReturnValue(new Promise(() => {}));

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(screen.getByTestId('application-logs-page')).toBeInTheDocument();
    expect(screen.getByText('正在加载日志')).toBeInTheDocument();
  });

  test('shows a retryable error state when run records fail to load', async () => {
    runtimeApi.fetchApplicationRuns.mockRejectedValue(
      new Error('runtime model is not registered')
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('日志加载失败')).toBeInTheDocument();
    expect(
      screen.getByText(
        '请刷新日志；如果持续失败，确认内置日志 Data Model 已完成迁移。'
      )
    ).toBeInTheDocument();
    expect(screen.getByText('刷新日志')).toBeInTheDocument();
  });

  test('does not auto refresh running records from the logs list', async () => {
    const setIntervalSpy = vi
      .spyOn(window, 'setInterval')
      .mockReturnValue(123 as unknown as ReturnType<typeof window.setInterval>);
    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        {
          id: 'run-1',
          application_id: 'app-1',
          scope_id: 'scope-1',
          run_mode: 'published_api_run' as const,
          status: 'running',
          target_node_id: 'node-llm',
          title: '运行中的公开 API 请求',
          expand_id: null,
          external_user: null,
          authorized_account: 'root',
          compatibility_mode: 'openai-responses-v1',
          total_tokens: null,
          input_tokens: null,
          output_tokens: null,
          input_cache_hit_tokens: null,
          input_cache_hit_rate: null,
          unique_node_count: 1,
          tool_callback_count: 0,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: null,
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:00Z'
        }
      ])
    );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    await screen.findByText('运行中的公开 API 请求');
    expect(runtimeApi.fetchApplicationRuns).toHaveBeenCalledTimes(1);
    expect(
      setIntervalSpy.mock.calls.some(([, delay]) => delay === 2_000)
    ).toBe(false);

    setIntervalSpy.mockRestore();
  });
});
