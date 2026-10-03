import { App } from 'antd';
import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { vi } from 'vitest';
import { AppProviders } from '../../../../app/AppProviders';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { ApplicationLogsWorkspace } from '../../components/logs/workspace/ApplicationLogsWorkspace';
import {
  fetchApplicationRunArchiveImportJob,
  fetchApplicationRuns,
  exportSelectedApplicationRunsTraceDumpZip
} from '../../api/runtime';
import { useAuthStore } from '../../../../state/auth-store';
import type { ApplicationRunSummary } from '../../api/runtime';

vi.mock('../../api/runtime', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../api/runtime')>()),
  fetchApplicationRuns: vi.fn(),
  fetchApplicationRunArchiveImportJob: vi.fn(),
  exportSelectedApplicationRunsTraceDumpZip: vi
    .fn()
    .mockResolvedValue({ blob: new Blob(['zip']) })
}));
vi.mock('../../lib/run-export-download', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../../lib/run-export-download')>()),
  saveApplicationRunExport: vi.fn()
}));
vi.mock('../../components/logs/ApplicationRunDetailPanel', () => ({
  ApplicationRunDetailPanel: ({
    applicationId,
    runId
  }: {
    applicationId: string;
    runId: string;
  }) => (
    <div data-testid="detail-owner">
      {applicationId}/{runId}
    </div>
  )
}));

const applications = [
  { id: 'app-a', name: 'Alpha' },
  { id: 'app-b', name: 'Beta' }
];
const applicationIds = applications.map((app) => app.id);
function run(id: string, application_id: string): ApplicationRunSummary {
  return {
    id,
    application_id,
    title: id,
    status: 'succeeded',
    invocation_count: 1,
    outcome: 'final_answer_observed',
    execution_stage: 'published',
    invocation_source: 'agent_flow_api',
    principal: { kind: 'user', id: 'root', display_name: 'Root' },
    started_at: '2026-10-01T10:00:00Z',
    total_cost: 0,
    total_tokens: 1
  } as ApplicationRunSummary;
}
function mount() {
  return render(
    <AppProviders>
      <App>
        <ApplicationLogsWorkspace
          applicationId=""
          applicationIds={applicationIds}
          applications={applications}
        />
      </App>
    </AppProviders>
  );
}

beforeEach(async () => {
  vi.clearAllMocks();
  window.localStorage.clear();
  window.localStorage.setItem('1flowbase.ui.locale_preference', 'zh_Hans');
  window.history.replaceState({}, '', '/route/pages/logs');
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('zh_Hans');
  useAuthStore.setState({ csrfToken: 'test-csrf' });
  vi.mocked(fetchApplicationRuns).mockResolvedValue({
    items: [run('run-a', 'app-a'), run('run-b', 'app-b')],
    total: 2,
    page: 1,
    page_size: 20
  });
});

test('opens each row with its own application and retains its owner when the row leaves the page', async () => {
  mount();
  const detailButtons = await screen.findAllByRole('button', {
    name: '查看运行详情'
  });
  fireEvent.click(detailButtons[1]!);
  expect(await screen.findByTestId('detail-owner')).toHaveTextContent(
    'app-b/run-b'
  );
  expect(
    new URLSearchParams(window.location.search).get('application_id')
  ).toBe('app-b');
  vi.mocked(fetchApplicationRuns).mockResolvedValue({
    items: [],
    total: 0,
    page: 1,
    page_size: 20
  });
  fireEvent.change(screen.getByRole('textbox', { name: '关键字搜索' }), {
    target: { value: 'absent' }
  });
  await waitFor(() =>
    expect(fetchApplicationRuns).toHaveBeenLastCalledWith(
      applicationIds,
      expect.objectContaining({ titleIncludes: 'absent' })
    )
  );
  expect(screen.getByTestId('detail-owner')).toHaveTextContent('app-b/run-b');
});

test('exports selected rows by application and requires an explicit import target', async () => {
  mount();
  await screen.findByRole('checkbox', { name: '选择导出 run-a' });
  fireEvent.click(screen.getByRole('checkbox', { name: '选择导出 run-a' }));
  fireEvent.click(screen.getByRole('checkbox', { name: '选择导出 run-b' }));
  fireEvent.click(screen.getByRole('button', { name: '导出已选日志' }));
  await waitFor(() =>
    expect(exportSelectedApplicationRunsTraceDumpZip).toHaveBeenCalledWith(
      'app-b',
      ['run-b'],
      'test-csrf'
    )
  );
  expect(exportSelectedApplicationRunsTraceDumpZip).toHaveBeenCalledWith(
    'app-a',
    ['run-a'],
    'test-csrf'
  );
  fireEvent.click(screen.getByRole('button', { name: '导入运行归档' }));
  expect(
    await screen.findByRole('combobox', { name: '选择导入目标应用' })
  ).toBeInTheDocument();
  const importDialog = screen.getByRole('dialog');
  expect(within(importDialog).getByText('导入运行归档')).toBeInTheDocument();
  expect(within(importDialog).getByRole('button', { name: /确\s*定/ })).toBeDisabled();
});

test('restores an in-progress import with its chosen application after reload', async () => {
  window.localStorage.setItem(
    '1flowbase.application.all-agent-flow.run_archive_import_target',
    'app-b'
  );
  window.localStorage.setItem(
    '1flowbase.application.app-b.run_archive_import_job',
    JSON.stringify({ jobId: 'job-b', fileName: 'archive.zip' })
  );
  vi.mocked(fetchApplicationRunArchiveImportJob).mockResolvedValue({
    status: 'succeeded',
    imported_run_count: 1,
    source_to_target_run_ids: [
      { source_run_id: 'source-b', target_run_id: 'imported-b' }
    ]
  } as Awaited<ReturnType<typeof fetchApplicationRunArchiveImportJob>>);
  mount();
  await waitFor(() =>
    expect(fetchApplicationRunArchiveImportJob).toHaveBeenCalledWith(
      'app-b',
      'job-b'
    )
  );
  expect(await screen.findByTestId('detail-owner')).toHaveTextContent(
    'app-b/imported-b'
  );
  expect(
    window.localStorage.getItem(
      '1flowbase.application.app-b.run_archive_import_job'
    )
  ).toBeNull();
});
