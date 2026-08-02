import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const api = vi.hoisted(() => ({
  settingsMcpCatalogQueryKey: ['settings', 'mcp-management', 'catalog'],
  settingsMcpTemplateLibraryQueryKey: [
    'settings',
    'mcp-management',
    'template-library'
  ],
  fetchSettingsMcpTemplateLibrary: vi.fn(),
  syncSettingsMcpTemplateLibraryBundle: vi.fn(),
  previewSettingsMcpTemplateLibraryBundle: vi.fn(),
  importSettingsMcpTemplateLibraryBundle: vi.fn(),
  setSettingsMcpTemplateLibraryCurrentVersion: vi.fn(),
  deleteSettingsMcpTemplateLibraryRelease: vi.fn(),
  repairSettingsMcpTemplateLibraryRelease: vi.fn()
}));

vi.mock('../../../../api/mcp-management', () => api);

import { AppI18nProvider } from '../../../../../../app/AppI18nProvider';
import {
  resetAuthStore,
  useAuthStore
} from '../../../../../../state/auth-store';
import { McpTemplateLibrary } from '../McpTemplateLibrary';

const version = {
  bundle_version: '1.1.1',
  locale: 'zh_Hans' as const,
  minimum_host_version: '0.3.2',
  exported_from_system_version: '0.3.2',
  checksum: 'sha256:bundle',
  algorithm: 'ed25519',
  key_id: 'official-key',
  signature: 'signed',
  signature_status: 'verified'
};

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'root-1',
      account: 'root',
      effective_display_role: 'root',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'root-1',
      account: 'root',
      email: 'root@example.com',
      phone: null,
      nickname: 'Root',
      name: 'Root',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'root',
      permissions: []
    }
  });
}

function renderLibrary() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </AppI18nProvider>
  );
  return render(<McpTemplateLibrary variant="compact" />, { wrapper });
}

describe('McpTemplateLibrary', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    api.fetchSettingsMcpTemplateLibrary.mockResolvedValue({
      remote_available: true,
      bundles: [
        {
          organization: 'taichuy',
          bundle_id: 'zh',
          current_bundle_version: null,
          remote_versions: [version],
          local_versions: []
        },
        {
          organization: 'taichuy',
          bundle_id: 'en',
          current_bundle_version: null,
          remote_versions: [{ ...version, locale: 'en_US' }],
          local_versions: []
        }
      ]
    });
    api.syncSettingsMcpTemplateLibraryBundle.mockResolvedValue(undefined);
  });

  test('AC-004 keeps sync pending state scoped to each catalog row', async () => {
    let finishFirst!: () => void;
    api.syncSettingsMcpTemplateLibraryBundle.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          finishFirst = resolve;
        })
    );
    renderLibrary();

    const syncButtons = await screen.findAllByRole('button', { name: '同步' });
    fireEvent.click(syncButtons[0]);
    await waitFor(() => expect(syncButtons[0]).toHaveClass('ant-btn-loading'));
    expect(syncButtons[1]).not.toHaveClass('ant-btn-loading');
    fireEvent.click(syncButtons[1]);
    expect(api.syncSettingsMcpTemplateLibraryBundle).toHaveBeenCalledTimes(2);
    finishFirst();
  });

  test('AC-005 imports current local content and shows update/shared Tool impact', async () => {
    api.fetchSettingsMcpTemplateLibrary.mockResolvedValue({
      remote_available: true,
      bundles: [
        {
          organization: 'taichuy',
          bundle_id: '1flowbase_zh_hans',
          current_bundle_version: '1.1.1',
          remote_versions: [version],
          local_versions: [
            { ...version, downloaded_at: '2026-08-02T10:00:00Z' }
          ]
        }
      ]
    });
    const review = {
      manifest: {
        schema_version: '1flowbase.mcp.bundle/v2',
        organization: 'taichuy',
        bundle_id: '1flowbase_zh_hans',
        bundle_version: '1.1.1',
        locale: 'zh_Hans',
        minimum_host_version: '0.3.2',
        exported_from_system_version: '0.3.2',
        exported_at: '2026-08-02T10:00:00Z',
        files: []
      },
      current_system_version: '0.3.2',
      version_status: 'same_system_version',
      effect_summary: {
        changes: 1,
        already_present: 0,
        conflicts: 0,
        unavailable: 0,
        failed: 0
      },
      tools: [],
      instances: [
        {
          id: '1flowbase',
          effect: 'update',
          result: 'imported',
          reason: null
        }
      ],
      connections: [],
      shared_tool_impacts: [
        { tool_id: 'system.get_runtime', instance_ids: ['other-instance'] }
      ]
    };
    api.previewSettingsMcpTemplateLibraryBundle.mockResolvedValue(review);
    api.importSettingsMcpTemplateLibraryBundle.mockResolvedValue({
      ...review,
      status: 'completed'
    });
    renderLibrary();

    const row = await screen.findByRole('row', {
      name: /taichuy\/1flowbase_zh_hans/
    });
    fireEvent.click(within(row).getByRole('button', { name: '导入' }));
    expect(await screen.findByText('覆盖更新')).toBeInTheDocument();
    expect(screen.getByText(/system.get_runtime/)).toBeInTheDocument();
    expect(api.syncSettingsMcpTemplateLibraryBundle).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole('button', { name: '确认导入并覆盖' }));
    await waitFor(() =>
      expect(api.importSettingsMcpTemplateLibraryBundle).toHaveBeenCalledWith(
        'taichuy',
        '1flowbase_zh_hans',
        {},
        'csrf-123'
      )
    );
  });
});
