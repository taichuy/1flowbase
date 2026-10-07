import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, expect, test, vi } from 'vitest';
import type {
  ConsoleApplicationCatalog,
  ConsoleApplicationCollector
} from '@1flowbase/api-client';

const api = vi.hoisted(() => ({
  fetchApplicationCatalog: vi.fn(),
  getApplicationsApiBaseUrl: () => 'https://console.example.com',
  applicationCatalogQueryKey: ['applications', 'catalog']
}));
const clipboard = vi.hoisted(() => ({ copyTextToClipboard: vi.fn() }));
vi.mock('../../api/applications', () => api);
vi.mock('../../../../shared/ui/clipboard/copy-text', () => clipboard);
vi.mock('@tanstack/react-router', () => ({
  Link: ({
    params,
    children
  }: {
    params: { applicationId: string };
    children: ReactNode;
  }) => <a href={`/applications/${params.applicationId}/api`}>{children}</a>
}));
import { AppProviders } from '../../../../app/AppProviders';
import { ApplicationCollectorPage } from '../../pages/ApplicationCollectorPage';

const applicationId = '1ab933c4-2f52-405c-bb1d-86d9e998c06c';
const collector: ConsoleApplicationCollector = {
  collector_code: 'codex-logs-collector',
  source_client: 'codex',
  display_name: 'Codex',
  description: 'Catalog provided description',
  version: '0.1.0',
  execution_target: 'client',
  documentation_url:
    'https://github.com/taichuy/1flowbase-official-plugins/blob/main/runtime-extensions/@taichuy/codex-logs-collector/README.md',
  shell_installer_url:
    'https://github.com/taichuy/1flowbase-official-plugins/releases/download/codex-logs-collector-v0.1.0/install.sh',
  powershell_installer_url:
    'https://github.com/taichuy/1flowbase-official-plugins/releases/download/codex-logs-collector-v0.1.0/install.ps1'
};
const catalog: ConsoleApplicationCatalog = {
  types: [],
  workflow_triggers: [],
  tags: [],
  collectors: [collector]
};
const renderPage = () =>
  render(
    <AppProviders>
      <ApplicationCollectorPage applicationId={applicationId} />
    </AppProviders>
  );
beforeEach(() => {
  vi.clearAllMocks();
  api.fetchApplicationCatalog.mockResolvedValue(catalog);
  clipboard.copyTextToClipboard.mockResolvedValue(undefined);
});

test('catalog tabs, installation detail and back consume backend metadata without fabricated status', async () => {
  renderPage();
  expect(await screen.findByText(collector.description)).toBeInTheDocument();
  expect(
    screen.queryByText(/Claude Code|OpenCode|OpenClaw|已安装|在线|最近采集/)
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: 'Codex' }));
  expect(screen.getByRole('tab', { name: 'Codex' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
  fireEvent.click(screen.getByRole('button', { name: '安装采集器' }));
  expect(screen.getByRole('region', { name: '安装步骤' })).toBeInTheDocument();
  expect(
    screen.getByRole('link', { name: '管理当前应用的 API Key' })
  ).toHaveAttribute('href', `/applications/${applicationId}/api`);
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  const command = document.querySelector('pre')!.textContent!;
  expect(command).toContain(collector.shell_installer_url);
  expect(command).toContain(
    `--endpoint 'https://console.example.com/api/logs/v1/events'`
  );
  expect(command).toContain(`--installation-id '${applicationId}'`);
  expect(command).toContain(`--version '0.1.0'`);
  expect(command).not.toMatch(/node |scripts\/node|api_key|API_KEY|sk-/);
  fireEvent.click(screen.getByRole('button', { name: '返回采集器目录' }));
  expect(
    screen.getByRole('button', { name: '安装采集器' })
  ).toBeInTheDocument();
  expect(screen.getByRole('tab', { name: 'Codex' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
});

test('OS selection copies the selected public installer command, never the credential', async () => {
  renderPage();
  fireEvent.click(await screen.findByRole('button', { name: '安装采集器' }));
  fireEvent.click(screen.getByRole('button', { name: '复制命令' }));
  await waitFor(() =>
    expect(clipboard.copyTextToClipboard).toHaveBeenCalledWith(
      document.querySelector('pre')!.textContent
    )
  );
  expect(clipboard.copyTextToClipboard.mock.calls[0][0]).toContain('mktemp');
  fireEvent.click(screen.getByRole('radio', { name: 'Windows (PowerShell)' }));
  const command = document.querySelector('pre')!.textContent!;
  expect(command).toContain(collector.powershell_installer_url);
  expect(command).toContain('Invoke-WebRequest');
  expect(command).toContain('-UseBasicParsing');
  expect(command).toContain(
    "-Endpoint 'https://console.example.com/api/logs/v1/events'"
  );
  expect(command).toContain("-Version '0.1.0'");
  expect(command).toContain(`-InstallationId '${applicationId}'`);
  expect(command).not.toContain('--endpoint');
  expect(command).toContain('finally');
  expect(command).not.toContain('install.sh');
  fireEvent.click(screen.getByRole('button', { name: '复制命令' }));
  await waitFor(() =>
    expect(clipboard.copyTextToClipboard).toHaveBeenLastCalledWith(command)
  );
});

test('loading, error and retry retain filter and do not offer stale installation', async () => {
  let reject!: (reason: Error) => void;
  api.fetchApplicationCatalog.mockReturnValueOnce(
    new Promise((_, rej) => {
      reject = rej;
    })
  );
  renderPage();
  expect(screen.getByLabelText('正在加载采集器')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: 'Codex' }));
  await act(async () => reject(new Error('catalog unavailable')));
  expect(await screen.findByText('无法加载采集器目录。')).toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: '安装采集器' })
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /^重\s*试$/ }));
  expect(
    await screen.findByRole('button', { name: '安装采集器' })
  ).toBeInTheDocument();
  expect(screen.getByRole('tab', { name: 'Codex' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
});

test('empty backend catalog remains empty instead of inventing a Codex release', async () => {
  api.fetchApplicationCatalog.mockResolvedValue({ ...catalog, collectors: [] });
  renderPage();
  expect(
    await screen.findByText('暂无可用的受支持采集器。')
  ).toBeInTheDocument();
  expect(
    screen.queryByRole('button', { name: '安装采集器' })
  ).not.toBeInTheDocument();
});

test('Codex tab filters source_client rather than display name', async () => {
  api.fetchApplicationCatalog.mockResolvedValue({
    ...catalog,
    collectors: [{ ...collector, source_client: 'another_client' }]
  });
  renderPage();
  expect(
    await screen.findByRole('button', { name: '安装采集器' })
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: 'Codex' }));
  expect(screen.getByText('暂无可用的受支持采集器。')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: '全部' }));
  expect(
    screen.getByRole('button', { name: '安装采集器' })
  ).toBeInTheDocument();
});
