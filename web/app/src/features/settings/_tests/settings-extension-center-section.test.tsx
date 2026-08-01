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
  id: 'extension-installation-1',
  category: 'runtime-extensions' as const,
  catalog_id: 'runtime-extensions:taichuy/openai',
  organization: '@taichuy',
  artifact_id: 'openai',
  version: '1.0.0',
  node_id: 'node-1',
  source: 'official',
  trust: 'official',
  warnings: [],
  local_path: '/api/plugins/openai',
  checksum: 'sha256:installed',
  signature_status: 'valid',
  signature_algorithm: 'ed25519',
  signing_key_id: 'official-key',
  status: 'installed',
  installed_by: 'user-1',
  created_at: '2026-08-01T10:00:00Z',
  updated_at: '2026-08-01T10:00:00Z'
};

const catalogEntry = {
  category: 'runtime-extensions' as const,
  id: 'runtime-extensions:taichuy/openai',
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

async function selectUploadCategory(
  uploadDialog: HTMLElement,
  category: string
) {
  const combobox = within(uploadDialog).getByRole('combobox', { name: '类型' });
  const pointerTarget = combobox.closest('.ant-select-selector');
  if (!pointerTarget) {
    throw new Error('upload category Select has no visible pointer target');
  }

  fireEvent.mouseDown(pointerTarget);
  const listbox = await screen.findByRole('listbox');
  fireEvent.click(within(listbox).getByRole('option', { name: category }));
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
          artifact_id: 'runtime-extensions:taichuy/openai',
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

    expect(await screen.findByText('openai')).toBeInTheDocument();
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
              artifact_id: 'runtime-extensions:taichuy/openai',
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
    const row = await screen.findByRole('row', { name: /openai/ });
    fireEvent.click(within(row).getByRole('button', { name: '更新' }));

    await waitFor(() => {
      expect(
        extensionsApi.fetchSettingsExtensionCatalogEntry
      ).toHaveBeenCalledWith(
        'runtime-extensions',
        'runtime-extensions:taichuy/openai'
      );
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

    const row = await screen.findByRole('row', { name: /openai/ });
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
    await selectUploadCategory(uploadDialog, 'runtime-extensions');
    fireEvent.change(input!, { target: { files: [file] } });
    fireEvent.click(
      within(uploadDialog).getByRole('button', { name: '上传并安装' })
    );
    const uploadConfirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    await uploadConfirmation?.onOk?.();

    await waitFor(() => {
      expect(extensionsApi.uploadSettingsExtension).toHaveBeenCalledWith(
        file,
        { category: 'runtime-extensions' },
        'csrf-123',
        {}
      );
    });
    view.unmount();
  });

  test('Root-AC-004 keeps install available for all six catalog categories without artifact_kind', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
      async (category: string) => ({
        category,
        catalog_page: 'page-1',
        catalog_page_number: 1,
        catalog_page_checksum: `sha256:${category}`,
        catalog_page_locator: `${category}/catalog/v1/pages/1.json`,
        limit: 20,
        next_cursor: null,
        total_entries: 1,
        entries: [
          {
            ...catalogEntry,
            category,
            id: `${category}.sample`,
            name: `${category} Extension`,
            artifact_kind: null,
            current_version: null,
            installation_status: 'not_installed'
          }
        ]
      })
    );
    renderSection();

    for (const category of [
      'agent-flow',
      'capability-plugins',
      'host-extensions',
      'i18n',
      'mcp',
      'runtime-extensions'
    ]) {
      fireEvent.click(screen.getByRole('tab', { name: category }));
      const row = await screen.findByRole('row', {
        name: new RegExp(`${category} Extension`)
      });
      expect(
        within(row).getByRole('button', { name: '安装' })
      ).toBeInTheDocument();
    }
  });

  test.each([
    {
      category: 'agent-flow' as const,
      version: '1.2.0',
      expectedVersion: '1.2.0'
    },
    {
      category: 'i18n' as const,
      version: '',
      expectedVersion: undefined
    }
  ])(
    'Root-AC-006 submits explicit $category upload identity without filename inference',
    async ({ category, version, expectedVersion }) => {
      renderSection();
      fireEvent.click(screen.getByRole('button', { name: '上传插件' }));
      const uploadDialog = await screen.findByRole('dialog');
      await selectUploadCategory(uploadDialog, category);

      fireEvent.change(
        within(uploadDialog).getByRole('textbox', { name: '组织' }),
        { target: { value: '@taichuy' } }
      );
      fireEvent.change(
        within(uploadDialog).getByRole('textbox', { name: '产物标识' }),
        { target: { value: 'sample-extension' } }
      );
      if (version) {
        fireEvent.change(
          within(uploadDialog).getByRole('textbox', { name: '版本' }),
          { target: { value: version } }
        );
      }
      const file = new File(['extension'], 'opaque-upload.bin');
      fireEvent.change(uploadDialog.querySelector('input[type="file"]')!, {
        target: { files: [file] }
      });
      fireEvent.click(
        within(uploadDialog).getByRole('button', { name: '上传并安装' })
      );
      const confirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
      await confirmation?.onOk?.();

      await waitFor(() => {
        expect(extensionsApi.uploadSettingsExtension).toHaveBeenCalledWith(
          file,
          {
            category,
            organization: '@taichuy',
            artifact_id: 'sample-extension',
            ...(expectedVersion ? { version: expectedVersion } : {})
          },
          'csrf-123',
          {}
        );
      });
    }
  );
});
