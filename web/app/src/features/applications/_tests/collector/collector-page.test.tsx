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
import { ApiClientError } from '@1flowbase/api-client';
const api = vi.hoisted(() => ({
  fetchApplicationCatalog: vi.fn(),
  installApplicationCollector: vi.fn(),
  getApplicationsApiBaseUrl: vi.fn(),
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
import { useAuthStore } from '../../../../state/auth-store';
import { ApplicationCollectorPage } from '../../pages/ApplicationCollectorPage';
const applicationId = '1ab933c4-2f52-405c-bb1d-86d9e998c06c';
const assetBase =
  '/api/public/client-collectors/taichuy/codex-logs-collector/0.1.0/assets';
const collector: ConsoleApplicationCollector = {
  collector_code: 'codex-logs-collector',
  source_client: 'codex',
  display_name: 'Codex',
  description: 'Catalog provided description',
  version: '0.1.0',
  execution_target: 'client',
  catalog_id: 'runtime-extensions:taichuy/codex-logs-collector',
  category: 'runtime-extensions',
  installation_status: 'installed',
  installed_version: '0.1.0',
  extension_installation_id: 'installation-one',
  installable: true,
  can_install: true,
  can_update: true,
  asset_base_url: assetBase,
  documentation_url: `${assetBase}/README.md`,
  shell_installer_url: `${assetBase}/install.sh`,
  powershell_installer_url: `${assetBase}/install.ps1`
};
const uninstalled: ConsoleApplicationCollector = {
  ...collector,
  installation_status: 'not_installed',
  installed_version: null,
  extension_installation_id: null,
  asset_base_url: null,
  documentation_url: null,
  shell_installer_url: null,
  powershell_installer_url: null
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
  useAuthStore.setState({ csrfToken: 'csrf-collector-test' });
  api.fetchApplicationCatalog.mockResolvedValue(catalog);
  api.getApplicationsApiBaseUrl.mockReturnValue('https://console.example.com');
  api.installApplicationCollector.mockResolvedValue({
    installation: { id: 'installation-one' }
  });
  clipboard.copyTextToClipboard.mockResolvedValue(undefined);
});

test('real platform installed state opens version-pinned local CLI detail and returns to current filter', async () => {
  renderPage();
  expect(await screen.findByText(collector.description)).toBeInTheDocument();
  expect(screen.getByText('平台已安装')).toBeInTheDocument();
  expect(
    screen.queryByText(/Claude Code|OpenCode|OpenClaw|在线|最近采集/)
  ).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole('tab', { name: 'Codex' }));
  fireEvent.click(screen.getByRole('button', { name: '下载采集 CLI' }));
  expect(screen.getByRole('region', { name: '安装步骤' })).toBeInTheDocument();
  expect(
    screen.getByRole('link', { name: '管理当前应用的 API Key' })
  ).toHaveAttribute('href', `/applications/${applicationId}/api`);
  expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
  const command = document.querySelector('pre')!.textContent!;
  expect(command).toContain(
    `https://console.example.com${assetBase}/install.sh`
  );
  expect(command).toContain(
    `--release-base 'https://console.example.com${assetBase}'`
  );
  expect(command).toContain(
    "--endpoint 'https://console.example.com/api/logs/v1/events'"
  );
  expect(command).not.toMatch(/github|node |api_key|API_KEY|sk-/);
  fireEvent.click(screen.getByRole('button', { name: '返回采集器目录' }));
  expect(
    screen.getByRole('button', { name: '下载采集 CLI' })
  ).toBeInTheDocument();
  expect(screen.getByRole('tab', { name: 'Codex' })).toHaveAttribute(
    'aria-selected',
    'true'
  );
});

test('platform install uses existing extension mutation, waits for backend refresh, then enables local CLI', async () => {
  api.fetchApplicationCatalog
    .mockResolvedValueOnce({ ...catalog, collectors: [uninstalled] })
    .mockResolvedValue(catalog);
  let resolve!: (value: unknown) => void;
  api.installApplicationCollector.mockReturnValueOnce(
    new Promise((res) => {
      resolve = res;
    })
  );
  renderPage();
  fireEvent.click(
    await screen.findByRole('button', { name: '安装到 1flowbase' })
  );
  await waitFor(() =>
    expect(api.installApplicationCollector).toHaveBeenCalledWith(
      {
        category: 'runtime-extensions',
        catalog_id: collector.catalog_id,
        version: '0.1.0',
        risk_override: undefined,
        compatibility_override: undefined
      },
      'csrf-collector-test',
      false
    )
  );
  expect(document.querySelector('pre')).toBeNull();
  expect(
    screen.queryByRole('button', { name: '下载采集 CLI' })
  ).not.toBeInTheDocument();
  await act(async () => resolve({ installation: { id: 'installation-one' } }));
  expect(
    await screen.findByRole('button', { name: '复制命令' })
  ).toBeInTheDocument();
  expect(api.fetchApplicationCatalog).toHaveBeenCalledTimes(2);
});

test('installation failure remains uninstalled and retryable without a local command', async () => {
  api.fetchApplicationCatalog.mockResolvedValue({
    ...catalog,
    collectors: [uninstalled]
  });
  api.installApplicationCollector.mockRejectedValueOnce(
    new Error('remote offline')
  );
  renderPage();
  fireEvent.click(
    await screen.findByRole('button', { name: '安装到 1flowbase' })
  );
  expect(
    await screen.findByText('无法安装到 1flowbase，请检查扩展中心配置后重试。')
  ).toBeInTheDocument();
  expect(
    screen.getByRole('button', { name: '安装到 1flowbase' })
  ).toBeEnabled();
  expect(document.querySelector('pre')).toBeNull();
});

test('existing integrity challenge requires explicit confirmation before retry', async () => {
  api.fetchApplicationCatalog
    .mockResolvedValueOnce({ ...catalog, collectors: [uninstalled] })
    .mockResolvedValue(catalog);
  api.installApplicationCollector.mockRejectedValueOnce(
    new ApiClientError({
      message: 'risk',
      status: 409,
      body: {
        code: 'extension_risk_confirmation_required',
        risk_challenge: {
          warnings: [
            {
              code: 'signature_missing',
              message: 'Signature is missing',
              overridable: true
            }
          ],
          compatibility: null
        }
      }
    })
  );
  renderPage();
  fireEvent.click(
    await screen.findByRole('button', { name: '安装到 1flowbase' })
  );
  expect(await screen.findByText('Signature is missing')).toBeInTheDocument();
  expect(api.installApplicationCollector).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole('button', { name: '确认安装到平台' }));
  await waitFor(() =>
    expect(api.installApplicationCollector).toHaveBeenLastCalledWith(
      expect.objectContaining({
        risk_override: {
          reason: 'user_confirmed',
          acknowledged_warnings: ['signature_missing']
        }
      }),
      'csrf-collector-test',
      false
    )
  );
});

test.each([
  {
    value: { ...uninstalled, can_install: false },
    message: '需要扩展中心安装权限，请联系管理员安装采集包。'
  },
  {
    value: {
      ...collector,
      installation_status: 'missing' as const,
      asset_base_url: null,
      shell_installer_url: null,
      powershell_installer_url: null
    },
    message: '本地安装包缺失或损坏，请在扩展中心移除此版本后重新安装。'
  }
])(
  'permission and missing local package never generate a runnable command',
  async ({ value, message }) => {
    api.fetchApplicationCatalog.mockResolvedValue({
      ...catalog,
      collectors: [value]
    });
    renderPage();
    expect(await screen.findByText(message)).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '下载采集 CLI' })
    ).not.toBeInTheDocument();
    const button = screen.queryByRole('button', { name: '安装到 1flowbase' });
    if (button) expect(button).toBeDisabled();
    expect(document.querySelector('pre')).toBeNull();
  }
);

test('installed package keeps local downloads when remote catalog no longer supplies it', async () => {
  api.fetchApplicationCatalog.mockResolvedValue({
    ...catalog,
    collectors: [{ ...collector, installable: false }]
  });
  renderPage();
  fireEvent.click(await screen.findByRole('button', { name: '下载采集 CLI' }));
  expect(document.querySelector('pre')!.textContent).toContain(
    `--release-base 'https://console.example.com${assetBase}'`
  );
  expect(api.installApplicationCollector).not.toHaveBeenCalled();
});

test('update keeps current pinned download version until a genuine platform update completes', async () => {
  api.fetchApplicationCatalog.mockResolvedValue({
    ...catalog,
    collectors: [{ ...collector, version: '0.2.0' }]
  });
  renderPage();
  fireEvent.click(await screen.findByRole('button', { name: '下载采集 CLI' }));
  expect(document.querySelector('pre')!.textContent).toContain(
    "--version '0.1.0'"
  );
  fireEvent.click(screen.getByRole('button', { name: '返回采集器目录' }));
  fireEvent.click(screen.getByRole('button', { name: '更新平台采集包' }));
  await waitFor(() =>
    expect(api.installApplicationCollector).toHaveBeenCalledWith(
      expect.objectContaining({ version: '0.2.0' }),
      'csrf-collector-test',
      true
    )
  );
});

test.each([
  { base: '', prefix: '' },
  { base: '/console-proxy/', prefix: '/console-proxy' }
])(
  'relative base $base gives absolute same-platform script and release URLs in both commands',
  async ({ base, prefix }) => {
    api.getApplicationsApiBaseUrl.mockReturnValue(base);
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: '下载采集 CLI' })
    );
    const resolvedBase = `${window.location.origin}${prefix}`;
    const command = document.querySelector('pre')!.textContent!;
    expect(command).toContain(
      `--endpoint '${resolvedBase}/api/logs/v1/events'`
    );
    expect(command).toContain(`--release-base '${resolvedBase}${assetBase}'`);
    fireEvent.click(
      screen.getByRole('radio', { name: 'Windows (PowerShell)' })
    );
    const ps = document.querySelector('pre')!.textContent!;
    expect(ps).toContain(`-ReleaseBase '${resolvedBase}${assetBase}'`);
    expect(ps).toContain(`${resolvedBase}${assetBase}/install.ps1`);
    fireEvent.click(screen.getByRole('button', { name: '复制命令' }));
    await waitFor(() =>
      expect(clipboard.copyTextToClipboard).toHaveBeenLastCalledWith(ps)
    );
  }
);

test('catalog error and retry preserve filter without using stale instructions', async () => {
  let reject!: (reason: Error) => void;
  api.fetchApplicationCatalog.mockReturnValueOnce(
    new Promise((_, rej) => {
      reject = rej;
    })
  );
  renderPage();
  expect(screen.getByLabelText('正在加载采集器')).toBeInTheDocument();
  await act(async () => reject(new Error('catalog unavailable')));
  expect(await screen.findByText('无法加载采集器目录。')).toBeInTheDocument();
  fireEvent.click(screen.getByRole('button', { name: /^重\s*试$/ }));
  expect(
    await screen.findByRole('button', { name: '下载采集 CLI' })
  ).toBeInTheDocument();
});

test('empty backend catalog does not invent any collector or tab', async () => {
  api.fetchApplicationCatalog.mockResolvedValue({ ...catalog, collectors: [] });
  renderPage();
  expect(
    await screen.findByText('暂无可用的受支持采集器。')
  ).toBeInTheDocument();
  expect(screen.queryByRole('tab', { name: 'Codex' })).not.toBeInTheDocument();
});
