import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
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

describe('ApplicationLogsPage - sorting filtering pagination', () => {
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

  test('refreshes runs from durable source', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('公开 API 退款总结')).toBeInTheDocument();

    runtimeApi.fetchApplicationRuns.mockResolvedValueOnce(
      applicationRunsPage([
        {
          id: 'run-2',
          run_mode: 'published_api_run' as const,
          status: 'succeeded',
          target_node_id: 'node-llm',
          title: '刷新后的日志',
          expand_id: 'customer-43',
          authorized_account: 'root',
          compatibility_mode: 'openai-responses-v1',
          statistics: {
            total_tokens: 60,
            unique_node_count: 3,
            tool_callback_count: 20
          },
          started_at: '2026-04-17T10:00:00Z',
          finished_at: '2026-04-17T10:00:01Z',
          created_at: '2026-04-17T10:00:00Z',
          updated_at: '2026-04-17T10:00:01Z'
        }
      ])
    );

    fireEvent.click(screen.getByRole('button', { name: '刷新日志' }));

    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRuns).toHaveBeenLastCalledWith(
        'app-1',
        {
          page: 1,
          pageSize: 20,
          timeRangeDays: 7,
          sortBy: 'started_at',
          sortOrder: 'desc',
          cacheMode: 'refresh'
        }
      );
    });
    expect(await screen.findByText('刷新后的日志')).toBeInTheDocument();
  });

  test('sizes log filter selects from their longest option label', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    const expectedMeasuredLabels = [
      [
        '时间间隔',
        [
          '今天',
          '过去 7 天',
          '过去 4 周',
          '过去 3 月',
          '过去 12 月',
          '所有时间'
        ]
      ],
      ['排序字段', ['排序：开始时间', '排序：更新时间']]
    ] as const;

    expectedMeasuredLabels.forEach(([ariaLabel, measuredLabels]) => {
      expect(
        screen.getByRole('combobox', { name: ariaLabel })
      ).toBeInTheDocument();

      measuredLabels.forEach((label) => {
        // Hidden measurement spans are intentionally aria-hidden and have no text content.

        const measureItem = document.querySelector(
          `.autosize-select__measure-item[data-measure-label="${label}"]`
        );

        expect(measureItem).toBeInTheDocument();
      });
    });

    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/shared/ui/autosize-select/autosize-select.css'
      ),
      'utf8'
    );

    expect(cssSource).toContain('.autosize-select {');
    expect(cssSource).toContain('grid-template-columns: max-content;');
    expect(cssSource).toContain('.autosize-select__control {');
    expect(cssSource).toContain('width: 100%;');
  });

  test('combines run sort field and direction into one sort control', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    const filters = screen.getByRole('search');
    const sortControl = within(filters).getByTestId(
      'application-logs-sort-control'
    );

    expect(
      within(sortControl).getByRole('combobox', { name: '排序字段' })
    ).toBeInTheDocument();
    expect(within(sortControl).getByText('排序：')).toBeInTheDocument();
    expect(
      within(sortControl).getByRole('button', {
        name: '当前降序，切换为升序'
      })
    ).toBeInTheDocument();
    expect(
      within(filters).queryByRole('combobox', { name: '排序方向' })
    ).not.toBeInTheDocument();
  });

  test('toggles run sort direction from the merged sort control', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    fireEvent.click(
      screen.getByRole('button', { name: '当前降序，切换为升序' })
    );

    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRuns).toHaveBeenLastCalledWith(
        'app-1',
        {
          page: 1,
          pageSize: 20,
          timeRangeDays: 7,
          sortBy: 'started_at',
          sortOrder: 'asc'
        }
      );
    });
    expect(
      await screen.findByRole('button', { name: '当前升序，切换为降序' })
    ).toBeInTheDocument();
  });

  test('refetches runs when selecting a different sort field', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '排序字段' }));
    fireEvent.click(
      await screen.findByText('更新时间', {
        selector: '.ant-select-item-option-content'
      })
    );

    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRuns).toHaveBeenLastCalledWith(
        'app-1',
        {
          page: 1,
          pageSize: 20,
          timeRangeDays: 7,
          sortBy: 'updated_at',
          sortOrder: 'desc'
        }
      );
    });
  });

  test('filters application logs by time range and title records query', async () => {
    runtimeApi.fetchApplicationRuns
      .mockResolvedValueOnce(
        applicationRunsPage([
          {
            id: 'run-refund',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-04-17T10:00:00Z',
            finished_at: '2026-04-17T10:05:00Z',
            created_at: '2026-04-17T10:00:00Z',
            updated_at: '2026-04-17T10:05:00Z'
          },
          {
            id: 'run-weather',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: '2026-04-17T12:00:00Z',
            created_at: '2026-04-17T09:00:00Z',
            updated_at: '2026-04-17T12:00:00Z'
          }
        ])
      )
      .mockResolvedValueOnce(
        applicationRunsPage([
          {
            id: 'run-refund',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            title: '退款规则',
            started_at: '2026-04-17T10:00:00Z',
            finished_at: '2026-04-17T10:05:00Z',
            created_at: '2026-04-17T10:00:00Z',
            updated_at: '2026-04-17T10:05:00Z'
          }
        ])
      )
      .mockResolvedValueOnce(
        applicationRunsPage([
          {
            id: 'run-refund',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-04-17T10:00:00Z',
            finished_at: '2026-04-17T10:05:00Z',
            created_at: '2026-04-17T10:00:00Z',
            updated_at: '2026-04-17T10:05:00Z'
          },
          {
            id: 'run-weather',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: '2026-04-17T12:00:00Z',
            created_at: '2026-04-17T09:00:00Z',
            updated_at: '2026-04-17T12:00:00Z'
          }
        ])
      )
      .mockResolvedValueOnce(
        applicationRunsPage([
          {
            id: 'run-refund',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-04-17T10:00:00Z',
            finished_at: '2026-04-17T10:05:00Z',
            created_at: '2026-04-17T10:00:00Z',
            updated_at: '2026-04-17T10:05:00Z'
          },
          {
            id: 'run-weather',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-04-17T09:00:00Z',
            finished_at: '2026-04-17T12:00:00Z',
            created_at: '2026-04-17T09:00:00Z',
            updated_at: '2026-04-17T12:00:00Z'
          },
          {
            id: 'run-old',
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            started_at: '2026-03-01T09:00:00Z',
            finished_at: '2026-03-01T09:02:00Z',
            created_at: '2026-03-01T09:00:00Z',
            updated_at: '2026-03-01T09:02:00Z'
          }
        ])
      );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('run-refund')).toBeInTheDocument();
    expect(screen.getByText('run-weather')).toBeInTheDocument();
    expect(screen.queryByText('run-old')).not.toBeInTheDocument();
    expect(
      screen.getByRole('combobox', { name: '时间间隔' })
    ).toBeInTheDocument();
    expect(
      screen.getByText('过去 7 天', {
        selector: '.ant-select-selection-item'
      })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', {
        name: '更新时间'
      })
    ).toBeInTheDocument();
    expect(screen.getByText('2026/4/17 18:05:00')).toBeInTheDocument();
    expect(screen.getByText('2026/4/17 20:00:00')).toBeInTheDocument();

    const searchInput = screen.getByPlaceholderText('搜索标题');
    fireEvent.change(searchInput, { target: { value: '退款' } });

    await waitFor(() => {
      expect(screen.getByText('run-refund')).toBeInTheDocument();
      expect(screen.queryByText('run-weather')).not.toBeInTheDocument();
    });
    expect(runtimeApi.fetchApplicationRuns).toHaveBeenLastCalledWith('app-1', {
      page: 1,
      pageSize: 20,
      timeRangeDays: 7,
      sortBy: 'started_at',
      sortOrder: 'desc',
      titleIncludes: '退款'
    });
    fireEvent.change(screen.getByPlaceholderText('搜索标题'), {
      target: { value: '' }
    });
    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRuns).toHaveBeenLastCalledWith(
        'app-1',
        {
          page: 1,
          pageSize: 20,
          timeRangeDays: 7,
          sortBy: 'started_at',
          sortOrder: 'desc'
        }
      );
    });
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '时间间隔' }));
    fireEvent.click(
      await screen.findByText('所有时间', {
        selector: '.ant-select-item-option-content'
      })
    );

    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRuns).toHaveBeenLastCalledWith(
        'app-1',
        {
          page: 1,
          pageSize: 20,
          timeRangeDays: null,
          sortBy: 'started_at',
          sortOrder: 'desc'
        }
      );
    });
    expect(await screen.findByText('run-old')).toBeInTheDocument();
  });

  test('requests 20 runs per page and refetches when pagination changes', async () => {
    runtimeApi.fetchApplicationRuns
      .mockResolvedValueOnce(
        applicationRunsPage(
          Array.from({ length: 20 }, (_, index) => ({
            id: `run-${index + 1}`,
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            title: `title-${index + 1}`,
            expand_id: null,
            authorized_account: 'root',
            started_at: `2026-04-17T09:${String(index).padStart(2, '0')}:00Z`,
            finished_at: `2026-04-17T09:${String(index).padStart(2, '0')}:30Z`,
            created_at: `2026-04-17T09:${String(index).padStart(2, '0')}:00Z`,
            updated_at: `2026-04-17T09:${String(index).padStart(2, '0')}:30Z`
          })),
          { total: 42, page: 1, page_size: 20 }
        )
      )
      .mockResolvedValueOnce(
        applicationRunsPage(
          Array.from({ length: 20 }, (_, index) => ({
            id: `run-${index + 21}`,
            run_mode: 'debug_flow_run' as const,
            status: 'succeeded',
            target_node_id: null,
            title: `title-${index + 21}`,
            expand_id: null,
            authorized_account: 'root',
            started_at: `2026-04-16T09:${String(index).padStart(2, '0')}:00Z`,
            finished_at: `2026-04-16T09:${String(index).padStart(2, '0')}:30Z`,
            created_at: `2026-04-16T09:${String(index).padStart(2, '0')}:00Z`,
            updated_at: `2026-04-16T09:${String(index).padStart(2, '0')}:30Z`
          })),
          { total: 42, page: 2, page_size: 20 }
        )
      );

    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(await screen.findByText('title-1')).toBeInTheDocument();
    expect(runtimeApi.fetchApplicationRuns).toHaveBeenNthCalledWith(
      1,
      'app-1',
      {
        page: 1,
        pageSize: 20,
        timeRangeDays: 7,
        sortBy: 'started_at',
        sortOrder: 'desc'
      }
    );
    expect(screen.getByText('共 42 条')).toBeInTheDocument();

    fireEvent.click(screen.getByTitle('2'));

    await waitFor(() => {
      expect(runtimeApi.fetchApplicationRuns).toHaveBeenNthCalledWith(
        2,
        'app-1',
        {
          page: 2,
          pageSize: 20,
          timeRangeDays: 7,
          sortBy: 'started_at',
          sortOrder: 'desc'
        }
      );
    });
    expect(await screen.findByText('title-21')).toBeInTheDocument();
  });
});
