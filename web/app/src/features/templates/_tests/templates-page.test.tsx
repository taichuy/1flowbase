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

const templatesApi = vi.hoisted(() => ({
  installedAgentFlowTemplatesQueryKey: [
    'templates',
    'installed-agent-flow'
  ] as const,
  fetchInstalledAgentFlowTemplates: vi.fn(),
  selectInstalledAgentFlowVersion: vi.fn(),
  deleteInstalledAgentFlowVersion: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'] as const,
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn()
}));

vi.mock('../api/templates', () => templatesApi);
vi.mock('../../applications/api/applications', () => applicationsApi);

import { AppI18nProvider } from '../../../app/AppI18nProvider';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { TemplatesPage } from '../pages/TemplatesPage';

const family = {
  id: 'installation-current',
  category: 'agent-flow',
  catalog_id: 'agent-flow:taichuy/fusion',
  organization: 'taichuy',
  artifact_id: 'fusion',
  version: '2.0.0',
  node_id: 'node-1',
  source: 'official',
  trust: 'official',
  warnings: [],
  local_path: '/extensions/fusion/2.0.0',
  checksum: 'sha256:current',
  signature_status: 'valid',
  signature_algorithm: 'ed25519',
  signing_key_id: 'official',
  status: 'installed',
  is_current: true,
  application_action: 'import_agent_flow',
  application_status: 'not_applied',
  installed_by: 'user-1',
  created_at: '2026-08-02T00:00:00Z',
  updated_at: '2026-08-02T00:00:00Z',
  installed_versions: [
    {
      id: 'installation-current',
      version: '2.0.0',
      source: 'official',
      trust: 'official',
      warnings: [],
      local_path: '/extensions/fusion/2.0.0',
      checksum: 'sha256:current',
      signature_status: 'valid',
      signature_algorithm: 'ed25519',
      signing_key_id: 'official',
      status: 'installed',
      is_current: true,
      installed_by: 'user-1',
      created_at: '2026-08-02T00:00:00Z',
      updated_at: '2026-08-02T00:00:00Z'
    },
    {
      id: 'installation-history',
      version: '1.0.0',
      source: 'upload',
      trust: 'unknown',
      warnings: [],
      local_path: '/extensions/fusion/1.0.0',
      checksum: 'sha256:history',
      signature_status: 'missing',
      signature_algorithm: null,
      signing_key_id: null,
      status: 'installed',
      is_current: false,
      installed_by: 'user-1',
      created_at: '2026-08-01T00:00:00Z',
      updated_at: '2026-08-01T00:00:00Z'
    }
  ]
};

function renderPage() {
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
  return render(<TemplatesPage />, { wrapper });
}

describe('installed Agent Flow template library', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    resetAuthStore();
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
        permissions: ['application.create.all']
      }
    });
    templatesApi.fetchInstalledAgentFlowTemplates.mockResolvedValue({
      limit: 50,
      total_entries: 1,
      next_cursor: null,
      entries: [family]
    });
    templatesApi.selectInstalledAgentFlowVersion.mockResolvedValue(family);
    templatesApi.deleteInstalledAgentFlowVersion.mockResolvedValue(family);
    applicationsApi.previewInstalledApplicationExtension.mockResolvedValue({
      extension_installation_id: 'installation-history',
      application_status: 'not_applied',
      integrity_warnings: [],
      required_integrity_override: null,
      preview: {
        application: {
          application_type: 'agent_flow',
          name: 'Fusion 1',
          description: 'Historical template',
          icon: null,
          icon_type: null,
          icon_background: null
        },
        dependencies: [],
        unresolved_nodes: [],
        flow_document: {}
      }
    });
    applicationsApi.importInstalledApplicationExtension.mockReturnValue(
      new Promise(() => undefined)
    );
  });

  test('AC-006 renders DB installed families, explicit current, and version history', async () => {
    renderPage();
    const row = await screen.findByRole('row', { name: /fusion/ });
    expect(within(row).getByText('2.0.0')).toBeInTheDocument();
    fireEvent.click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('1.0.0')).toBeInTheDocument();
    expect(within(drawer).getByText('当前')).toBeInTheDocument();
  });

  test('AC-006 selects and deletes versions by installation id', async () => {
    renderPage();
    const row = await screen.findByRole('row', { name: /fusion/ });
    fireEvent.click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');
    const history = within(drawer).getByText('1.0.0').closest('.ant-list-item');
    fireEvent.click(
      within(history as HTMLElement).getByRole('button', { name: '设为当前' })
    );
    await waitFor(() => {
      expect(templatesApi.selectInstalledAgentFlowVersion).toHaveBeenCalledWith(
        'installation-history',
        'csrf-123'
      );
    });

    fireEvent.click(within(row).getByRole('button', { name: '查看' }));
    const reopened = await screen.findByRole('dialog');
    const current = within(reopened)
      .getByText('2.0.0')
      .closest('.ant-list-item');
    fireEvent.click(
      within(current as HTMLElement).getByRole('button', { name: '删除版本' })
    );
    const confirmation = await screen.findByText(
      '确认删除这个本地模板版本？此操作不会改变已创建的应用。'
    );
    fireEvent.click(
      within(confirmation.closest('.ant-modal') as HTMLElement).getByRole(
        'button',
        { name: '删除版本' }
      )
    );
    await waitFor(() => {
      expect(templatesApi.deleteInstalledAgentFlowVersion).toHaveBeenCalledWith(
        'installation-current',
        'csrf-123'
      );
    });
  });

  test('AC-006 previews and imports from a historical installed version', async () => {
    renderPage();
    const row = await screen.findByRole('row', { name: /fusion/ });
    fireEvent.click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');
    const history = within(drawer).getByText('1.0.0').closest('.ant-list-item');
    fireEvent.click(
      within(history as HTMLElement).getByRole('button', {
        name: '从此版本导入'
      })
    );
    await waitFor(() => {
      expect(
        applicationsApi.previewInstalledApplicationExtension
      ).toHaveBeenCalledWith('installation-history');
    });
    const importDialog = await screen.findByText('Fusion 1');
    fireEvent.click(
      within(importDialog.closest('.ant-modal') as HTMLElement).getByRole(
        'button',
        { name: '导入应用' }
      )
    );
    await waitFor(() => {
      expect(
        applicationsApi.importInstalledApplicationExtension
      ).toHaveBeenCalledWith(
        'installation-history',
        { name: 'Fusion 1', description: 'Historical template' },
        'csrf-123'
      );
    });
  });
});
