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

import { AppProviders } from '../../../../app/AppProviders';
import { appI18n } from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
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

describe('ApplicationLogsPage - table field settings', () => {
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
          execution_stage: 'published' as const,
          invocation_source: 'agent_flow_api' as const,
          principal: {
            kind: 'application_api_key' as const,
            id: 'api-key-1',
            display_name: 'Local Claude Code'
          },
          status: 'succeeded',
          target_node_id: 'node-llm',
          title: '公开 API 退款总结',
          expand_id: 'customer-42',
          authorized_account: 'root',
          compatibility_mode: 'openai-responses-v1',
          total_tokens: 128,
          input_tokens: 100,
          output_tokens: 28,
          count_tokens_input_tokens: 6_956,
          input_cache_hit_tokens: 64,
          input_cache_hit_rate: 0.9832,
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

  test('shows token breakdown columns from run summaries', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect(
      await screen.findByRole('columnheader', { name: '输入 tokens' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '输出 tokens' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: 'CountTokens' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '命中缓存 tokens' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '缓存命中率' })
    ).toBeInTheDocument();
    expect(screen.getByText('100')).toBeInTheDocument();
    expect(screen.getByText('28')).toBeInTheDocument();
    expect(screen.getByText('6,956')).toBeInTheDocument();
    expect(screen.getByText('64')).toBeInTheDocument();
    expect(screen.getByText('98.32%')).toBeInTheDocument();
  });

  test('persists table column visibility in user preferences meta', async () => {
    useAuthStore.getState().setAuthenticated({
      csrfToken: 'csrf-123',
      actor: {
        id: 'user-1',
        account: 'root',
        effective_display_role: 'root',
        current_workspace_id: 'workspace-1'
      },
      me: {
        id: 'user-1',
        account: 'root',
        email: 'root@example.com',
        phone: null,
        nickname: 'Root',
        name: 'Root',
        avatar_url: null,
        introduction: '',
        preferred_locale: null,
        effective_display_role: 'root',
        permissions: [],
        meta: {}
      }
    });
    const hiddenColumnsMeta = {
      ui: {
        data_tables: {
          'applications.logs.runs': {
            visibleColumnKeys: [
              'title',
              'status',
              'run_mode',
              'authorized_account',
              'started_at',
              'duration',
              'action'
            ],
            columnWidths: {}
          }
        }
      }
    };
    const fetchMock = vi
      .spyOn(globalThis, 'fetch')
      .mockImplementation(async (input, init) => {
        const url = input instanceof Request ? input.url : String(input);
        const method = init?.method ?? 'GET';
        const meta =
          url.includes('/api/console/me/meta') && method === 'PATCH'
            ? hiddenColumnsMeta
            : {};

        return new Response(
          JSON.stringify({
            data: {
              id: 'user-1',
              account: 'root',
              email: 'root@example.com',
              phone: null,
              nickname: 'Root',
              name: 'Root',
              avatar_url: null,
              introduction: '',
              preferred_locale: null,
              effective_display_role: 'root',
              permissions: [],
              meta
            },
            meta: null
          }),
          {
            status: 200,
            headers: { 'content-type': 'application/json' }
          }
        );
      });
    const { unmount } = render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    expect(
      screen.getByRole('columnheader', {
        name: 'expand_id'
      })
    ).toBeInTheDocument();

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '字段配置' }));
    fireEvent.click(
      await screen.findByText('expand_id', {
        selector: '.ant-select-item-option-content'
      })
    );

    await waitFor(() => {
      const meta = useAuthStore.getState().me?.meta as
        | {
            ui?: {
              data_tables?: {
                'applications.logs.runs'?: {
                  visibleColumnKeys?: string[];
                };
              };
            };
          }
        | undefined;

      expect(fetchMock).toHaveBeenCalledWith(
        expect.stringContaining('/api/console/me/meta'),
        expect.objectContaining({
          method: 'PATCH',
          headers: expect.objectContaining({
            'x-csrf-token': 'csrf-123'
          }),
          body: expect.stringContaining('"applications.logs.runs"')
        })
      );
      expect(
        meta?.ui?.data_tables?.['applications.logs.runs']?.visibleColumnKeys
      ).not.toContain('expand_id');
    });

    unmount();
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    await waitFor(() => {
      expect(
        screen.queryByRole('columnheader', {
          name: 'expand_id'
        })
      ).not.toBeInTheDocument();
    });
    fetchMock.mockRestore();
  });

  test('places table field configuration with the filters', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);
    const filters = screen.getByRole('search');

    expect(
      within(filters).getByRole('combobox', { name: '字段配置' })
    ).toBeInTheDocument();
  });

  test('renders table field configuration with Ant Design multiple select', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    const trigger = screen.getByRole('combobox', { name: '字段配置' });

    expect(trigger).toHaveAttribute('aria-haspopup', 'listbox');
    expect(
      screen.queryByText('字段配置', {
        selector: '.application-runs-table__column-selector-trigger-caret'
      })
    ).not.toBeInTheDocument();
  });

  test('keeps table field configuration as a native responsive multiple select', async () => {
    const tableSource = await readFile(
      path.resolve(process.cwd(), 'src/shared/ui/data-table/DataTable.tsx'),
      'utf8'
    );

    expect(tableSource).toContain('mode="multiple"');
    expect(tableSource).toContain('maxTagCount="responsive"');
    expect(tableSource).toContain('popupMatchSelectWidth');
    expect(tableSource).not.toContain('maxTagCount={0}');
    expect(tableSource).not.toContain("maxTagPlaceholder={() => '字段配置'}");
  });

  test('opens table field configuration as a dropdown menu', async () => {
    render(
      <AppProviders>
        <ApplicationLogsPage applicationId="app-1" />
      </AppProviders>
    );

    expect((await screen.findAllByRole('table')).length).toBeGreaterThan(0);

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '字段配置' }));

    const fieldListbox = await screen.findByRole('listbox');

    expect(
      within(fieldListbox).getByRole('option', { name: 'expand_id' })
    ).toBeInTheDocument();
    expect(
      within(fieldListbox).getByRole('option', { name: '运行 ID' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '重置默认字段' })
    ).toBeInTheDocument();
  });
});
