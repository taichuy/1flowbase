import { readFile } from 'node:fs/promises';
import path from 'node:path';
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

import { resetAuthStore } from '../../../../state/auth-store';

function applicationRunsPage<T>(
  items: T[],
  overrides?: Partial<{
    total: number;
    page: number;
    page_size: number;
  }>
) {
  return {
    items: items.map((item) => ({
      log_conversation_id: null,
      log_task_run_id: null,
      member_run_ids: [],
      parent_task_run_id: null,
      outcome: 'final_answer_observed',
      user_input: null,
      final_output: null,
      execution_stage: (item as { run_mode?: string }).run_mode?.startsWith(
        'debug_'
      )
        ? 'debug'
        : 'published',
      invocation_source: (item as { run_mode?: string }).run_mode?.startsWith(
        'debug_'
      )
        ? 'debug'
        : 'agent_flow_api',
      principal: {
        kind: 'application_api_key',
        id: 'key-1',
        display_name: null
      },
      ...item
    })),
    total: overrides?.total ?? items.length,
    page: overrides?.page ?? 1,
    page_size: overrides?.page_size ?? 20
  };
}

describe('ApplicationLogsPage - layout CSS', () => {
  let getBoundingClientRectSpy: { mockRestore: () => void } | undefined;
  let innerHeightSpy: { mockRestore: () => void } | undefined;
  let innerWidthSpy: { mockRestore: () => void } | undefined;
  let dateNowSpy: { mockRestore: () => void } | undefined;

  beforeEach(() => {
    window.localStorage.clear();
    window.history.replaceState({}, '', '/applications/app-1/logs');
    dateNowSpy = vi
      .spyOn(Date, 'now')
      .mockReturnValue(new Date('2026-04-18T00:00:00Z').getTime());
    runtimeApi.fetchApplicationRuns.mockReset();
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
          authorized_display_name: 'root',
          compatibility_mode: 'openai-responses-v1',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          created_at: '2026-04-17T09:00:00Z',
          updated_at: '2026-04-17T09:00:01Z'
        }
      ])
    );
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue({
      items: [
        {
          message_id: 'fixture-message-2',
          run_id: 'run-1:context:0',
          detail_run_id: null,
          can_open_detail: false,
          role: 'system',
          content: '你是项目助手',
          started_at: '2026-04-17T08:59:00Z',
          finished_at: '2026-04-17T08:59:01Z',
          status: 'succeeded',
          query: null,
          model: 'deepseek-chat',
          answer: null,
          is_current: false
        },
        {
          message_id: 'fixture-message-1',
          run_id: 'run-1',
          detail_run_id: 'run-1',
          can_open_detail: true,
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          status: 'succeeded',
          query: '总结退款政策',
          model: 'deepseek-chat',
          answer: '退款政策摘要',
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

  test('uses floating window CSS instead of a docked splitter override', async () => {
    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/applications/pages/application-logs-page.css'
      ),
      'utf8'
    );

    expect(cssSource).not.toContain('application-logs-page--detail-open');
    expect(cssSource).not.toContain('--application-runs-table-body-height');
    expect(cssSource).toContain('flex: 1 1 auto;');
    expect(cssSource).toContain('width: 100%;');
    expect(cssSource).toContain('.application-logs-floating-window');
    expect(cssSource).toContain('position: fixed;');
    expect(cssSource).toContain(
      '.application-logs-floating-window__resize--left'
    );
    expect(cssSource).toContain('cursor: move;');
    expect(cssSource).not.toContain('position: static;');
  });

  test('pins the logs page list to the parent full-height layout instead of a nested viewport calc', async () => {
    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/applications/pages/application-logs-page.css'
      ),
      'utf8'
    );

    expect(cssSource).not.toContain('calc(100vh - 120px)');
    expect(cssSource).toMatch(
      /\.application-logs-page\s*\{[^}]*height:\s*100%;[^}]*min-height:\s*0;[^}]*box-sizing:\s*border-box;/s
    );
    expect(cssSource).toMatch(
      /\.application-logs-page\s*\{[^}]*padding:\s*0;/s
    );
    expect(cssSource).toMatch(
      /\.application-logs-page__stack\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;[^}]*height:\s*100%;/s
    );
    expect(cssSource).toMatch(
      /\.application-logs-page__list\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;[^}]*flex:\s*1 1 auto;[^}]*min-height:\s*0;[^}]*overflow-x:\s*hidden;[^}]*overflow-y:\s*hidden;/s
    );
  });

  test('keeps the table header and pagination fixed around the row scroll area', async () => {
    const cssSource = await readFile(
      path.resolve(process.cwd(), 'src/shared/ui/data-table/data-table.css'),
      'utf8'
    );
    const pageCssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/applications/pages/application-logs-page.css'
      ),
      'utf8'
    );
    const tableSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/applications/components/logs/ApplicationRunsTable.tsx'
      ),
      'utf8'
    );

    expect(cssSource).toMatch(
      /\.data-table\s*\{[^}]*flex:\s*1 1 auto;[^}]*\}/s
    );
    expect(cssSource).toMatch(
      /\.data-table__scroll-area\s*\{[^}]*flex:\s*1 1 auto;[^}]*overflow-x:\s*auto;[^}]*overflow-y:\s*auto;[^}]*\}/s
    );
    expect(cssSource).toMatch(
      /\.data-table \.ant-table-thead > tr > th\s*\{[^}]*position:\s*sticky;[^}]*top:\s*0;[^}]*z-index:\s*2;[^}]*\}/s
    );
    expect(cssSource).toMatch(
      /\.data-table__pagination\s*\{[^}]*flex:\s*0 0 auto;[^}]*\}/s
    );
    expect(pageCssSource).toMatch(
      /\.application-logs-page__list\s*\{[^}]*overflow-y:\s*hidden;[^}]*\}/s
    );
    expect(tableSource).not.toContain("y: '100%'");
  });

  test('keeps horizontal scrolling on the runs table wrapper instead of the Ant Design body', async () => {
    const cssSource = await readFile(
      path.resolve(process.cwd(), 'src/shared/ui/data-table/data-table.css'),
      'utf8'
    );
    const tableSource = await readFile(
      path.resolve(process.cwd(), 'src/shared/ui/data-table/DataTable.tsx'),
      'utf8'
    );

    expect(tableSource).not.toMatch(/\s+sticky(?:\s|\n|\/?>)/);
    expect(tableSource).not.toContain('x: fixedTableWidth');
    expect(tableSource).toContain('minWidth: fixedTableWidth');
    expect(cssSource).toMatch(
      /\.data-table__scroll-area\s*\{[^}]*overflow-x:\s*auto;[^}]*overflow-y:\s*auto;/s
    );
    expect(cssSource).toMatch(
      /\.data-table \.ant-table-body\s*\{[^}]*overflow-x:\s*hidden !important;[^}]*\}/s
    );
  });

  test('renders logs inside the viewport section layout height chain', async () => {
    const pageSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/features/applications/pages/ApplicationDetailPage.tsx'
      ),
      'utf8'
    );

    expect(pageSource).toMatch(
      /contentWidth=\{[\s\S]*requestedSectionKey === 'orchestration'[\s\S]*\? 'full'[\s\S]*: 'wide'[\s\S]*\}/
    );
    expect(pageSource).toMatch(
      /heightMode=\{[\s\S]*requestedSectionKey === 'logs'[\s\S]*requestedSectionKey === 'api'[\s\S]*\? 'viewport'[\s\S]*: 'natural'[\s\S]*\}/
    );
  });
});
