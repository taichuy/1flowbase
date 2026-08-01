import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { Modal } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const extensionsApi = vi.hoisted(() => ({
  settingsInstalledExtensionsQueryKey: vi.fn((cursor?: string) => [
    'settings',
    'extension-center',
    'installed',
    cursor ?? 'start'
  ]),
  settingsExtensionCatalogQueryKey: vi.fn(
    (category: string, cursor?: string) => [
      'settings',
      'extension-center',
      'catalog',
      category,
      cursor ?? 'start'
    ]
  ),
  fetchSettingsInstalledExtensions: vi.fn(),
  fetchSettingsExtensionCatalog: vi.fn(),
  fetchSettingsExtensionCatalogEntry: vi.fn(),
  checkSettingsExtensionUpdates: vi.fn(),
  installSettingsExtension: vi.fn(),
  uploadSettingsExtension: vi.fn(),
  getSettingsExtensionRiskChallenge: vi.fn()
}));

vi.mock('../api/extensions', () => extensionsApi);

import { AppI18nProvider } from '../../../app/AppI18nProvider';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { SettingsExtensionCenterSection } from '../pages/settings-page/SettingsExtensionCenterSection';

const installedEntry = {
  category: 'runtime-extensions' as const,
  artifact_kind: 'model_provider',
  artifact_id: '@taichuy/openai',
  display_name: 'OpenAI Provider',
  description: 'Installed provider extension',
  current_version: '1.0.0',
  system_requirements: '>=0.3.0',
  installation_status: 'installed',
  source: 'official',
  trust: 'official',
  warnings: [],
  installation: { id: 'installation-1' },
  local_artifact: { installed_path: '/api/plugins/openai' }
};

const catalogEntry = {
  category: 'runtime-extensions' as const,
  id: '@taichuy/openai',
  name: 'OpenAI Provider',
  organization: '@taichuy',
  artifact: 'openai',
  version: '1.1.0',
  description: 'Remote provider extension',
  host_version_requirement: '>=0.4.0',
  source: { kind: 'github_release' },
  signature: { key_id: 'official-key' },
  checksum: 'sha256:catalog-entry',
  download_locator: { url: 'https://example.com/openai.1flowbasepkg' },
  catalog_page: 1,
  catalog_source: 'official',
  current_version: '1.0.0',
  installation_status: 'installed',
  artifact_kind: 'model_provider',
  installation_source: 'official',
  trust: 'official',
  warnings: [],
  compatibility: null
};

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
      nickname: 'root',
      name: 'root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: ['system.extension-center.manage.all']
    }
  });
}

function renderSection() {
  const client = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false }
    }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </AppI18nProvider>
  );

  return render(<SettingsExtensionCenterSection />, { wrapper });
}

describe('SettingsExtensionCenterSection', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      next_cursor: null,
      entries: [installedEntry]
    });
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [catalogEntry]
    });
    extensionsApi.fetchSettingsExtensionCatalogEntry.mockResolvedValue(
      catalogEntry
    );
    extensionsApi.checkSettingsExtensionUpdates.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: null,
      items: [
        {
          artifact_id: '@taichuy/openai',
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        }
      ]
    });
    extensionsApi.installSettingsExtension.mockResolvedValue({});
    extensionsApi.uploadSettingsExtension.mockResolvedValue({});
    extensionsApi.getSettingsExtensionRiskChallenge.mockReturnValue(null);
    vi.spyOn(Modal, 'confirm').mockReturnValue({ destroy: vi.fn() } as never);
  });

  test('Root-AC-002/003 renders seven tabs, loads installed inventory first, and checks only visible pages', async () => {
    renderSection();

    expect(await screen.findByText('OpenAI Provider')).toBeInTheDocument();
    expect(screen.getAllByRole('tab')).toHaveLength(7);
    expect(
      screen.getByRole('columnheader', { name: '来源' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '可信度' })
    ).toBeInTheDocument();
    expect(extensionsApi.fetchSettingsExtensionCatalog).not.toHaveBeenCalled();
    await waitFor(() => {
      expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledWith(
        {
          category: 'runtime-extensions',
          catalog_page: null,
          items: [
            {
              artifact_id: '@taichuy/openai',
              current_version: '1.0.0'
            }
          ]
        },
        'csrf-123'
      );
    });
    expect(
      screen
        .getByRole('button', { name: '更新' })
        .closest('[data-update-state]')
    ).toHaveAttribute('data-update-state', 'update_available');

    fireEvent.click(screen.getByRole('tab', { name: 'runtime-extensions' }));
    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'runtime-extensions',
        undefined
      );
      expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledWith(
        expect.objectContaining({ catalog_page: 'page-1' }),
        'csrf-123'
      );
    });
  });

  test('Root-AC-004 resolves and performs an installed-row update instead of switching tabs', async () => {
    renderSection();
    const row = await screen.findByRole('row', { name: /OpenAI Provider/ });
    fireEvent.click(within(row).getByRole('button', { name: '更新' }));

    await waitFor(() => {
      expect(
        extensionsApi.fetchSettingsExtensionCatalogEntry
      ).toHaveBeenCalledWith('runtime-extensions', '@taichuy/openai');
    });
    const confirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    await confirmation?.onOk?.();
    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        catalogEntry,
        'csrf-123',
        {},
        true
      );
    });
  });

  test('Root-AC-006 lists a structured risk challenge and retries with its exact codes', async () => {
    const riskError = new Error('confirmation required');
    const challenge = {
      warnings: [
        {
          code: 'signature_invalid',
          message: '签名与产物内容不一致，但仍可继续。',
          overridable: true
        },
        {
          code: 'policy_locked',
          message: '该警告不可由用户确认覆盖。',
          overridable: false
        }
      ],
      compatibility: null
    };
    extensionsApi.installSettingsExtension
      .mockRejectedValueOnce(riskError)
      .mockResolvedValueOnce({});
    extensionsApi.getSettingsExtensionRiskChallenge.mockImplementation(
      (error: unknown) => (error === riskError ? challenge : null)
    );
    renderSection();
    fireEvent.click(screen.getByRole('tab', { name: 'runtime-extensions' }));
    const row = await screen.findByRole('row', { name: /OpenAI Provider/ });
    fireEvent.click(within(row).getByRole('button', { name: '更新' }));
    const installConfirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    await installConfirmation?.onOk?.();

    await waitFor(() => expect(Modal.confirm).toHaveBeenCalledTimes(2));
    const riskConfirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    render(riskConfirmation?.content as ReactNode);
    expect(
      screen.getByText('签名与产物内容不一致，但仍可继续。')
    ).toBeInTheDocument();
    expect(screen.getByText('该警告不可由用户确认覆盖。')).toBeInTheDocument();
    await riskConfirmation?.onOk?.();

    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenLastCalledWith(
        catalogEntry,
        'csrf-123',
        {
          risk_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['signature_invalid']
          }
        },
        true
      );
    });
  });

  test('Root-AC-003/006 exposes upload and marks a failed current-page check red', async () => {
    extensionsApi.checkSettingsExtensionUpdates.mockRejectedValue(
      new Error('catalog unavailable')
    );
    const view = renderSection();

    const row = await screen.findByRole('row', { name: /OpenAI Provider/ });
    await waitFor(() => {
      expect(
        within(row).getByRole('button', { name: '更新' }).closest('span')
          ?.parentElement
      ).toHaveAttribute('data-update-state', 'unknown_error');
    });

    fireEvent.click(screen.getByRole('button', { name: '上传插件' }));
    const uploadDialog = await screen.findByRole('dialog');
    const input = uploadDialog.querySelector('input[type="file"]');
    const file = new File(['extension'], 'extension.1flowbasepkg');
    fireEvent.mouseDown(
      within(uploadDialog).getByRole('combobox', { name: '类型' })
    );
    fireEvent.click(
      await screen.findByRole('option', { name: 'runtime-extensions' })
    );
    fireEvent.change(input!, { target: { files: [file] } });
    fireEvent.click(
      within(uploadDialog).getByRole('button', { name: '上传并安装' })
    );
    const uploadConfirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    await uploadConfirmation?.onOk?.();

    await waitFor(() => {
      expect(extensionsApi.uploadSettingsExtension).toHaveBeenCalledWith(
        file,
        'runtime-extensions',
        'csrf-123',
        {}
      );
    });
    view.unmount();
  });
});
