import { App as AntdApp } from 'antd';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
  applicationConversationMessagesQueryKey: (
    applicationId: string,
    input: { flowRunId?: string | null }
  ) =>
    [
      'applications',
      applicationId,
      'runtime',
      'conversation-messages',
      input.flowRunId ?? ''
    ] as const,
  fetchApplicationRuns: vi.fn(),
  fetchApplicationRunOverview: vi.fn(),
  fetchApplicationRunTraceTree: vi.fn(),
  fetchApplicationRunTraceNodeChildren: vi.fn(),
  fetchApplicationRunTraceNodeContent: vi.fn(),
  fetchApplicationRunTraceNodeDetail: vi.fn(),
  fetchApplicationRunTraceToolCallbackContent: vi.fn(),
  fetchApplicationRunResumeTimeline: vi.fn(),
  fetchApplicationConversationMessages: vi.fn(),
  fetchApplicationRunConversationMessages: vi.fn(),
  fetchRuntimeDebugArtifact: vi.fn(),
  fetchRuntimeDebugArtifacts: vi.fn(),
  exportApplicationRunTraceDump: vi.fn(),
  exportSelectedApplicationRunsTraceDumpZip: vi.fn(),
  createApplicationRunArchiveUploadSession: vi.fn(),
  uploadApplicationRunArchiveChunk: vi.fn(),
  completeApplicationRunArchiveUploadSession: vi.fn(),
  fetchApplicationRunArchiveImportJob: vi.fn(),
  resumeFlowRun: vi.fn(),
  completeCallbackTask: vi.fn()
}));

vi.mock('../../api/runtime', () => runtimeApi);

import { AppProviders } from '../../../../app/AppProviders';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';
import { ApplicationLogsPage } from '../../pages/ApplicationLogsPage';

function applicationRunsPage(
  items: Array<Record<string, unknown>>,
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

function runSummary(id: string, title: string) {
  return {
    id,
    application_id: 'app-1',
    scope_id: 'workspace-1',
    run_mode: 'published_api_run',
    status: 'succeeded',
    target_node_id: 'node-llm',
    title,
    expand_id: null,
    external_user: null,
    authorized_account: 'root',
    api_key_id: null,
    api_key_name_snapshot: null,
    publication_version_id: null,
    external_conversation_id: null,
    external_trace_id: null,
    compatibility_mode: 'openai-responses-v1',
    idempotency_key: null,
    total_tokens: 10,
    input_tokens: 8,
    output_tokens: 2,
    input_cache_hit_tokens: 0,
    unique_node_count: 2,
    tool_callback_count: 0,
    started_at: '2026-04-17T09:00:00Z',
    finished_at: '2026-04-17T09:00:01Z',
    created_at: '2026-04-17T09:00:00Z',
    updated_at: '2026-04-17T09:00:01Z'
  };
}

function authenticate() {
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
}

function renderLogsPage() {
  return render(
    <AppProviders>
      <AntdApp>
        <ApplicationLogsPage applicationId="app-1" />
      </AntdApp>
    </AppProviders>
  );
}

function getRunSelectionCheckbox(title: string) {
  return screen.getByLabelText(`选择导出 ${title}`);
}

describe('ApplicationLogsPage - run export downloads', () => {
  let createObjectUrlSpy: ReturnType<typeof vi.fn>;
  let revokeObjectUrlSpy: ReturnType<typeof vi.fn>;
  let anchorClickSpy: { mockRestore: () => void } | undefined;
  let dateNowSpy: { mockRestore: () => void } | undefined;

  beforeEach(async () => {
    window.localStorage.clear();
    window.history.replaceState({}, '', '/applications/app-1/logs');
    window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
    await loadApplicationI18nResources();
    await appI18n.changeLanguage('zh_Hans');
    authenticate();
    dateNowSpy = vi
      .spyOn(Date, 'now')
      .mockReturnValue(new Date('2026-04-18T00:00:00Z').getTime());
    createObjectUrlSpy = vi.fn(() => 'blob:trace-dump');
    revokeObjectUrlSpy = vi.fn();
    Object.defineProperty(window.URL, 'createObjectURL', {
      configurable: true,
      value: createObjectUrlSpy
    });
    Object.defineProperty(window.URL, 'revokeObjectURL', {
      configurable: true,
      value: revokeObjectUrlSpy
    });
    anchorClickSpy = vi
      .spyOn(HTMLAnchorElement.prototype, 'click')
      .mockImplementation(() => undefined);
    runtimeApi.fetchApplicationRuns.mockReset();
    runtimeApi.fetchApplicationRunOverview.mockReset();
    runtimeApi.fetchApplicationRunTraceTree.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeChildren.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeContent.mockReset();
    runtimeApi.fetchApplicationRunTraceNodeDetail.mockReset();
    runtimeApi.fetchApplicationRunTraceToolCallbackContent.mockReset();
    runtimeApi.fetchApplicationRunResumeTimeline.mockReset();
    runtimeApi.fetchApplicationConversationMessages.mockReset();
    runtimeApi.fetchApplicationRunConversationMessages.mockReset();
    runtimeApi.fetchRuntimeDebugArtifact.mockReset();
    runtimeApi.fetchRuntimeDebugArtifacts.mockReset();
    runtimeApi.exportApplicationRunTraceDump.mockReset();
    runtimeApi.exportSelectedApplicationRunsTraceDumpZip.mockReset();
    runtimeApi.createApplicationRunArchiveUploadSession.mockReset();
    runtimeApi.uploadApplicationRunArchiveChunk.mockReset();
    runtimeApi.completeApplicationRunArchiveUploadSession.mockReset();
    runtimeApi.fetchApplicationRunArchiveImportJob.mockReset();
    Object.defineProperty(window, 'crypto', {
      configurable: true,
      value: {
        subtle: {
          digest: vi.fn().mockResolvedValue(Uint8Array.from([0xab]).buffer)
        }
      }
    });
    Object.defineProperty(Blob.prototype, 'arrayBuffer', {
      configurable: true,
      value: vi.fn().mockResolvedValue(new TextEncoder().encode('chunk').buffer)
    });
    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage([
        runSummary('run-1', '退款总结'),
        runSummary('run-2', '天气查询')
      ])
    );
    runtimeApi.fetchApplicationRunOverview.mockResolvedValue({
      flow_run: {
        id: 'run-1',
        status: 'succeeded',
        input_payload: { query: '退款' },
        output_payload: { answer: '退款摘要' },
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      run: {
        id: 'run-1',
        status: 'succeeded',
        compatibility_mode: 'openai-responses-v1',
        started_at: '2026-04-17T09:00:00Z',
        finished_at: '2026-04-17T09:00:01Z'
      },
      statistics: {
        invocation_count: 1,
        total_tokens: 10,
        unique_node_count: 2,
        tool_callback_count: 0
      },
      answer_snapshot: null
    });
    runtimeApi.fetchApplicationRunConversationMessages.mockResolvedValue({
      items: [
        {
          message_id: 'fixture-message-1',
          run_id: 'run-1',
          detail_run_id: 'run-1',
          can_open_detail: true,
          role: 'assistant',
          content: '退款摘要',
          started_at: '2026-04-17T09:00:00Z',
          finished_at: '2026-04-17T09:00:01Z',
          status: 'succeeded',
          query: '退款',
          model: 'deepseek-chat',
          answer: '退款摘要',
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
    runtimeApi.exportApplicationRunTraceDump.mockResolvedValue({
      blob: new Blob(['{}'], { type: 'application/json' }),
      filename: 'run-1.json',
      contentType: 'application/json'
    });
    runtimeApi.exportSelectedApplicationRunsTraceDumpZip.mockResolvedValue({
      blob: new Blob(['zip'], { type: 'application/zip' }),
      filename: 'selected-runs.zip',
      contentType: 'application/zip'
    });
    runtimeApi.createApplicationRunArchiveUploadSession.mockResolvedValue({
      session_id: 'session-1',
      application_id: 'app-1',
      status: 'uploading',
      filename: 'archive.json',
      total_size_bytes: 17,
      received_bytes: 0,
      expected_sha256: 'sha256:ab',
      created_at: '2026-06-24T00:00:00Z',
      updated_at: '2026-06-24T00:00:00Z'
    });
    runtimeApi.uploadApplicationRunArchiveChunk.mockResolvedValue({
      session_id: 'session-1',
      chunk_index: 0,
      chunk_size_bytes: 17,
      chunk_sha256: 'sha256:ab',
      received_bytes: 17,
      status: 'uploaded'
    });
    runtimeApi.completeApplicationRunArchiveUploadSession.mockResolvedValue({
      job_id: 'job-1',
      application_id: 'app-1',
      upload_session_id: 'session-1',
      status: 'queued',
      archive_version: 1,
      archive_sha256: 'sha256:archive',
      run_count: 1,
      imported_run_count: 0,
      source_to_target_run_ids: [],
      error_payload: null,
      result_payload: {},
      created_at: '2026-06-24T00:00:00Z',
      updated_at: '2026-06-24T00:00:00Z',
      started_at: null,
      finished_at: null
    });
    runtimeApi.fetchApplicationRunArchiveImportJob.mockResolvedValue({
      job_id: 'job-1',
      application_id: 'app-1',
      upload_session_id: 'session-1',
      status: 'succeeded',
      archive_version: 1,
      archive_sha256: 'sha256:archive',
      run_count: 1,
      imported_run_count: 1,
      source_to_target_run_ids: [
        { source_run_id: 'source-run-1', target_run_id: 'imported-run-1' }
      ],
      error_payload: null,
      result_payload: {},
      created_at: '2026-06-24T00:00:00Z',
      updated_at: '2026-06-24T00:00:01Z',
      started_at: '2026-06-24T00:00:00Z',
      finished_at: '2026-06-24T00:00:01Z'
    });
  });

  afterEach(() => {
    resetAuthStore();
    anchorClickSpy?.mockRestore();
    anchorClickSpy = undefined;
    dateNowSpy?.mockRestore();
    dateNowSpy = undefined;
  });

  test('exports only selected visible run ids from the current page', async () => {
    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    const exportButton = screen.getByRole('button', {
      name: '导出已选日志'
    });

    expect(exportButton).toBeDisabled();

    fireEvent.click(getRunSelectionCheckbox('退款总结'));
    fireEvent.click(getRunSelectionCheckbox('天气查询'));

    await waitFor(() => expect(exportButton).toBeEnabled());
    fireEvent.click(exportButton);

    await waitFor(() => {
      expect(
        runtimeApi.exportSelectedApplicationRunsTraceDumpZip
      ).toHaveBeenCalledWith('app-1', ['run-1', 'run-2'], 'csrf-123');
    });
    expect(createObjectUrlSpy).toHaveBeenCalledWith(expect.any(Blob));
    expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalled();
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).not.toHaveBeenCalled();
  });

  test('does not render a selected run archive export action', async () => {
    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', {
        name: '导出已选运行归档'
      })
    ).not.toBeInTheDocument();
    expect(
      runtimeApi.exportSelectedApplicationRunsTraceDumpZip
    ).not.toHaveBeenCalled();
    expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalled();
  });

  test('does not render a per-row run archive export action', async () => {
    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', {
        name: '导出运行归档：退款总结'
      })
    ).not.toBeInTheDocument();
    expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalled();
  });

  test('shows loading while selected zip export is pending', async () => {
    let resolveExport:
      | ((download: {
          blob: Blob;
          filename: string;
          contentType: string;
        }) => void)
      | undefined;
    runtimeApi.exportSelectedApplicationRunsTraceDumpZip.mockReturnValueOnce(
      new Promise<{
        blob: Blob;
        filename: string;
        contentType: string;
      }>((resolve) => {
        resolveExport = resolve;
      })
    );

    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    fireEvent.click(getRunSelectionCheckbox('退款总结'));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '导出已选日志' })).toBeEnabled()
    );

    const exportButton = screen.getByRole('button', {
      name: '导出已选日志'
    });
    fireEvent.click(exportButton);

    await waitFor(() => {
      expect(exportButton).toHaveClass('ant-btn-loading');
    });

    resolveExport?.({
      blob: new Blob(['zip'], { type: 'application/zip' }),
      filename: 'selected-runs.zip',
      contentType: 'application/zip'
    });

    await waitFor(() => {
      expect(exportButton).not.toHaveClass('ant-btn-loading');
    });
  });

  test('clears selected runs after pagination, search, filters and durable refresh', async () => {
    runtimeApi.fetchApplicationRuns.mockResolvedValue(
      applicationRunsPage(
        [runSummary('run-1', '退款总结'), runSummary('run-2', '天气查询')],
        { total: 42 }
      )
    );

    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    const exportButton = screen.getByRole('button', {
      name: '导出已选日志'
    });

    fireEvent.click(getRunSelectionCheckbox('退款总结'));
    await waitFor(() => expect(exportButton).toBeEnabled());
    fireEvent.click(screen.getByTitle('2'));
    await waitFor(() => expect(exportButton).toBeDisabled());

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    fireEvent.click(getRunSelectionCheckbox('退款总结'));
    await waitFor(() => expect(exportButton).toBeEnabled());
    fireEvent.change(screen.getByPlaceholderText('搜索标题'), {
      target: { value: '天气' }
    });
    await waitFor(() => expect(exportButton).toBeDisabled());

    expect(await screen.findByText('天气查询')).toBeInTheDocument();
    fireEvent.click(getRunSelectionCheckbox('天气查询'));
    await waitFor(() => expect(exportButton).toBeEnabled());
    fireEvent.mouseDown(screen.getByRole('combobox', { name: '时间间隔' }));
    fireEvent.click(
      await screen.findByText('所有时间', {
        selector: '.ant-select-item-option-content'
      })
    );
    await waitFor(() => expect(exportButton).toBeDisabled());

    expect(await screen.findByText('天气查询')).toBeInTheDocument();
    fireEvent.click(getRunSelectionCheckbox('天气查询'));
    await waitFor(() => expect(exportButton).toBeEnabled());
    fireEvent.click(screen.getByRole('button', { name: '刷新日志' }));
    await waitFor(() => expect(exportButton).toBeDisabled());
  }, 10_000);

  test('reports export failures without saving an empty file', async () => {
    runtimeApi.exportSelectedApplicationRunsTraceDumpZip.mockRejectedValueOnce(
      new Error('export failed')
    );

    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    fireEvent.click(getRunSelectionCheckbox('退款总结'));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '导出已选日志' })).toBeEnabled()
    );
    fireEvent.click(screen.getByRole('button', { name: '导出已选日志' }));

    expect(await screen.findByText('导出日志失败')).toBeInTheDocument();
    expect(createObjectUrlSpy).not.toHaveBeenCalled();
  });

  test('requires csrf token before selected zip export', async () => {
    resetAuthStore();

    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    fireEvent.click(getRunSelectionCheckbox('退款总结'));
    await waitFor(() =>
      expect(screen.getByRole('button', { name: '导出已选日志' })).toBeEnabled()
    );
    fireEvent.click(screen.getByRole('button', { name: '导出已选日志' }));

    expect(
      await screen.findByText('缺少 CSRF token，无法导出日志')
    ).toBeInTheDocument();
    expect(
      runtimeApi.exportSelectedApplicationRunsTraceDumpZip
    ).not.toHaveBeenCalled();
    expect(createObjectUrlSpy).not.toHaveBeenCalled();
  });

  test('uploads an archive file, polls the import job and opens the target run', async () => {
    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    const importInput = screen.getByTestId(
      'application-logs-archive-import-input'
    ) as HTMLInputElement;
    Object.defineProperty(importInput, 'files', {
      configurable: true,
      value: [
        new File(['{"archive_version":1}'], 'archive.json', {
          type: 'application/json'
        })
      ]
    });
    fireEvent.change(importInput);

    await waitFor(() => {
      expect(
        runtimeApi.createApplicationRunArchiveUploadSession
      ).toHaveBeenCalledWith(
        'app-1',
        {
          filename: 'archive.json',
          total_size_bytes: 21,
          expected_sha256: 'sha256:ab',
          chunk_size_bytes: 1024 * 1024
        },
        'csrf-123'
      );
    });
    expect(runtimeApi.uploadApplicationRunArchiveChunk).toHaveBeenCalledWith(
      'app-1',
      'session-1',
      0,
      expect.any(Blob),
      'sha256:ab',
      'csrf-123'
    );
    expect(
      runtimeApi.completeApplicationRunArchiveUploadSession
    ).toHaveBeenCalledWith('app-1', 'session-1', 'csrf-123');
    expect(runtimeApi.fetchApplicationRunArchiveImportJob).toHaveBeenCalledWith(
      'app-1',
      'job-1'
    );
    expect(await screen.findByText('已导入 1 条运行')).toBeInTheDocument();
    expect(
      await screen.findByRole('complementary', { name: '运行详情' })
    ).toBeInTheDocument();
    expect(
      window.localStorage.getItem(
        '1flowbase.application.app-1.run_archive_import_job'
      )
    ).toBeNull();
  });

  test('uploads an archive file when web crypto digest is unavailable', async () => {
    Object.defineProperty(window, 'crypto', {
      configurable: true,
      value: {}
    });
    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    const importInput = screen.getByTestId(
      'application-logs-archive-import-input'
    ) as HTMLInputElement;
    Object.defineProperty(importInput, 'files', {
      configurable: true,
      value: [
        new File(['{"archive_version":1}'], 'archive.json', {
          type: 'application/json'
        })
      ]
    });
    fireEvent.change(importInput);

    await waitFor(() => {
      expect(
        runtimeApi.createApplicationRunArchiveUploadSession
      ).toHaveBeenCalledWith(
        'app-1',
        expect.objectContaining({
          filename: 'archive.json',
          total_size_bytes: 21,
          expected_sha256: expect.stringMatching(/^sha256:[0-9a-f]{64}$/),
          chunk_size_bytes: 1024 * 1024
        }),
        'csrf-123'
      );
    });
    expect(runtimeApi.uploadApplicationRunArchiveChunk).toHaveBeenCalledWith(
      'app-1',
      'session-1',
      0,
      expect.any(Blob),
      expect.stringMatching(/^sha256:[0-9a-f]{64}$/),
      'csrf-123'
    );
  });

  test('resumes a persisted archive import job after returning to the page', async () => {
    window.localStorage.setItem(
      '1flowbase.application.app-1.run_archive_import_job',
      JSON.stringify({ jobId: 'job-1', fileName: 'archive.json' })
    );

    renderLogsPage();

    await waitFor(() => {
      expect(
        runtimeApi.fetchApplicationRunArchiveImportJob
      ).toHaveBeenCalledWith('app-1', 'job-1');
    });
    expect(await screen.findByText('已导入 1 条运行')).toBeInTheDocument();
    expect(
      window.localStorage.getItem(
        '1flowbase.application.app-1.run_archive_import_job'
      )
    ).toBeNull();
    expect(
      await screen.findByRole('complementary', { name: '运行详情' })
    ).toBeInTheDocument();
  });

  test('exports a single run from the conversation log floating window without composing trace content', async () => {
    renderLogsPage();

    expect(await screen.findByText('退款总结')).toBeInTheDocument();
    fireEvent.click(screen.getAllByRole('button', { name: '查看运行详情' })[0]);
    expect(
      await screen.findByRole('complementary', { name: '运行详情' })
    ).toBeInTheDocument();
    fireEvent.click(
      (await screen.findAllByRole('button', { name: '查看对话日志' }))[0]
    );
    expect(
      await screen.findByRole('complementary', { name: '对话日志' })
    ).toBeInTheDocument();
    fireEvent.click(
      await screen.findByRole('button', { name: '导出当前运行 JSON' })
    );

    await waitFor(() => {
      expect(runtimeApi.exportApplicationRunTraceDump).toHaveBeenCalledWith(
        'app-1',
        'run-1'
      );
    });
    expect(createObjectUrlSpy).toHaveBeenCalledWith(expect.any(Blob));
    expect(runtimeApi.fetchApplicationRunTraceTree).not.toHaveBeenCalled();
    expect(
      runtimeApi.fetchApplicationRunTraceNodeContent
    ).not.toHaveBeenCalled();
  }, 10_000);
});
