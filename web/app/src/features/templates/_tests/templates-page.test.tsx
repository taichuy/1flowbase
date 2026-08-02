import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const templatesApi = vi.hoisted(() => ({
  officialAgentFlowTemplateCatalogQueryKey: [
    'templates',
    'official-agent-flow',
    'catalog'
  ],
  fetchOfficialAgentFlowTemplateCatalog: vi.fn(),
  previewOfficialAgentFlowTemplate: vi.fn(),
  importOfficialAgentFlowTemplate: vi.fn(),
  syncOfficialAgentFlowTemplate: vi.fn(),
  switchOfficialAgentFlowTemplateCurrent: vi.fn(),
  deleteOfficialAgentFlowTemplateRelease: vi.fn(),
  repairOfficialAgentFlowTemplateRelease: vi.fn()
}));

vi.mock('../api/templates', () => templatesApi);
vi.mock('../../applications/api/applications', () => ({
  applicationsQueryKey: ['applications']
}));

import { AppI18nProvider } from '../../../app/AppI18nProvider';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { TemplatesPage } from '../pages/TemplatesPage';

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
      preferred_locale: 'zh_Hans',
      effective_display_role: 'root',
      permissions: [],
      meta: { ui: { locale: { preferred_locale: 'zh_Hans' } } }
    }
  });
}

function version(releaseVersion: number, name = 'Fusion') {
  return {
    template_id: name.toLowerCase(),
    release_version: releaseVersion,
    exported_from_system_version: '0.3.1',
    exported_at: `2026-08-0${releaseVersion}T00:00:00Z`,
    application: { name, description: `${name} description` },
    checksum: `sha256:${name}:${releaseVersion}`,
    algorithm: 'sha256',
    key_id: 'official-key'
  };
}

function entry({
  templateId = 'fusion',
  name = 'Fusion',
  currentReleaseVersion = null,
  localVersions = [],
  remoteVersions = [1]
}: {
  templateId?: string;
  name?: string;
  currentReleaseVersion?: number | null;
  localVersions?: number[];
  remoteVersions?: number[];
} = {}) {
  return {
    template_id: templateId,
    source_path: localVersions.length > 0 ? `/templates/${templateId}` : null,
    current_release_version: currentReleaseVersion,
    local_versions: localVersions.map((item) => ({
      ...version(item, name),
      template_id: templateId
    })),
    remote_versions: remoteVersions.map((item) => ({
      ...version(item, name),
      template_id: templateId,
      download_url: `https://templates.example/${templateId}/${item}`,
      signature: `signature-${item}`
    }))
  };
}

const preview = {
  schema_version: '1flowbase.application-template/v1' as const,
  application: {
    application_type: 'agent_flow' as const,
    name: 'Fusion',
    description: 'Fusion description',
    icon: null,
    icon_type: null,
    icon_background: null
  },
  dependencies: [],
  unresolved_nodes: [],
  document: {
    schemaVersion: '1flowbase.flow/v2',
    meta: {
      flowId: 'flow-template',
      name: 'Fusion',
      description: '',
      tags: []
    },
    graph: { nodes: [], edges: [] },
    editor: {
      viewport: { x: 0, y: 0, zoom: 1 },
      annotations: [],
      activeContainerPath: []
    }
  }
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}

function renderPage() {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false }, mutations: { retry: false } }
  });
  const wrapper = ({ children }: { children: ReactNode }) => (
    <AppI18nProvider>
      <QueryClientProvider client={client}>{children}</QueryClientProvider>
    </AppI18nProvider>
  );
  return render(<TemplatesPage />, { wrapper });
}

describe('Agent Flow template library', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    resetAuthStore();
    authenticate();
    await appI18n.changeLanguage('zh_Hans');
    templatesApi.previewOfficialAgentFlowTemplate.mockResolvedValue(preview);
    templatesApi.importOfficialAgentFlowTemplate.mockReturnValue(
      new Promise(() => undefined)
    );
    templatesApi.syncOfficialAgentFlowTemplate.mockResolvedValue(version(3));
    templatesApi.switchOfficialAgentFlowTemplateCurrent.mockResolvedValue(
      version(2)
    );
    templatesApi.deleteOfficialAgentFlowTemplateRelease.mockResolvedValue(
      undefined
    );
    templatesApi.repairOfficialAgentFlowTemplateRelease.mockResolvedValue(
      version(2)
    );
  });

  afterEach(async () => {
    await appI18n.changeLanguage('en_US');
  });

  test('previews an absent local template, opens the shared modal, and imports a new application', async () => {
    templatesApi.fetchOfficialAgentFlowTemplateCatalog.mockResolvedValue({
      remote_available: true,
      remote_error: null,
      templates: [entry()]
    });
    renderPage();

    fireEvent.click(await screen.findByRole('button', { name: '导入-Fusion' }));
    await waitFor(() => {
      expect(
        templatesApi.previewOfficialAgentFlowTemplate
      ).toHaveBeenCalledWith('fusion', undefined, 'csrf-123');
    });
    const dialog = await screen.findByRole('dialog');
    fireEvent.click(within(dialog).getByRole('button', { name: '导入应用' }));
    await waitFor(() => {
      expect(templatesApi.importOfficialAgentFlowTemplate).toHaveBeenCalledWith(
        'fusion',
        {
          name: 'Fusion',
          description: 'Fusion description'
        },
        'csrf-123'
      );
    });
  });

  test('previews the current local release without requesting a remote artifact', async () => {
    templatesApi.fetchOfficialAgentFlowTemplateCatalog.mockResolvedValue({
      remote_available: true,
      remote_error: null,
      templates: [entry({ currentReleaseVersion: 2, localVersions: [1, 2] })]
    });
    renderPage();
    fireEvent.click(await screen.findByRole('button', { name: '导入-Fusion' }));
    await waitFor(() => {
      expect(
        templatesApi.previewOfficialAgentFlowTemplate
      ).toHaveBeenCalledWith('fusion', 2, 'csrf-123');
    });
    expect(templatesApi.syncOfficialAgentFlowTemplate).not.toHaveBeenCalled();
  });

  test('syncs only the selected row and never shows two row spinners', async () => {
    const pending = deferred<ReturnType<typeof version>>();
    templatesApi.syncOfficialAgentFlowTemplate.mockReturnValue(pending.promise);
    templatesApi.fetchOfficialAgentFlowTemplateCatalog.mockResolvedValue({
      remote_available: true,
      remote_error: null,
      templates: [
        entry({
          currentReleaseVersion: 1,
          localVersions: [1],
          remoteVersions: [1, 2]
        }),
        entry({
          templateId: 'deepseek',
          name: 'Deepseek',
          currentReleaseVersion: 1,
          localVersions: [1],
          remoteVersions: [1, 2]
        })
      ]
    });
    renderPage();
    const fusionRow = await screen.findByRole('row', { name: /Fusion/ });
    await screen.findByRole('row', { name: /Deepseek/ });
    fireEvent.click(within(fusionRow).getByRole('button', { name: '同步' }));

    await waitFor(() => {
      expect(
        within(screen.getByRole('row', { name: /Fusion/ })).getByRole(
          'button',
          { name: /同步/ }
        )
      ).toHaveClass('ant-btn-loading');
      const other = within(
        screen.getByRole('row', { name: /Deepseek/ })
      ).getByRole('button', { name: '同步' });
      expect(other).toBeDisabled();
      expect(other).not.toHaveClass('ant-btn-loading');
    });
    pending.resolve(version(2));
    await waitFor(() => {
      expect(templatesApi.syncOfficialAgentFlowTemplate).toHaveBeenCalledWith(
        'fusion',
        undefined,
        'csrf-123'
      );
    });
  });

  test('manages remote and local release history without changing applications', async () => {
    templatesApi.fetchOfficialAgentFlowTemplateCatalog.mockResolvedValue({
      remote_available: true,
      remote_error: null,
      templates: [
        entry({
          currentReleaseVersion: 1,
          localVersions: [1, 2],
          remoteVersions: [1, 2, 3]
        })
      ]
    });
    renderPage();
    const row = await screen.findByRole('row', { name: /Fusion/ });
    fireEvent.click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');

    const remoteRow = within(drawer).getByRole('row', { name: /v3/ });
    fireEvent.click(
      within(remoteRow).getByRole('button', { name: '同步到本地' })
    );
    await waitFor(() => {
      expect(templatesApi.syncOfficialAgentFlowTemplate).toHaveBeenCalledWith(
        'fusion',
        3,
        'csrf-123'
      );
    });

    const localRow = within(drawer).getByRole('row', { name: /v2/ });
    fireEvent.click(within(localRow).getByRole('button', { name: '设为当前' }));
    await waitFor(() => {
      expect(
        templatesApi.switchOfficialAgentFlowTemplateCurrent
      ).toHaveBeenCalledWith('fusion', 2, 'csrf-123');
    });
    fireEvent.click(
      within(localRow).getByRole('button', { name: '从此版本导入' })
    );
    await waitFor(() => {
      expect(
        templatesApi.previewOfficialAgentFlowTemplate
      ).toHaveBeenCalledWith('fusion', 2, 'csrf-123');
    });
    const importDialog = await screen.findByRole('dialog', {
      name: '导入应用压缩包'
    });
    fireEvent.click(within(importDialog).getByRole('button', { name: '取消' }));
    fireEvent.click(
      within(localRow).getByRole('button', { name: '修复本地版本' })
    );
    await waitFor(() => {
      expect(
        templatesApi.repairOfficialAgentFlowTemplateRelease
      ).toHaveBeenCalledWith('fusion', 2, 'csrf-123');
    });
    fireEvent.click(within(localRow).getByRole('button', { name: '删除版本' }));
    await screen.findByText(
      '确认删除这个本地模板版本？此操作不会改变已创建的应用。'
    );
    const deleteButtons = screen.getAllByRole('button', { name: '删除版本' });
    fireEvent.click(deleteButtons[deleteButtons.length - 1]!);
    await waitFor(() => {
      expect(
        templatesApi.deleteOfficialAgentFlowTemplateRelease
      ).toHaveBeenCalledWith('fusion', 2, 'csrf-123');
    });
  });

  test('keeps local preview and import available while the remote source is unavailable', async () => {
    templatesApi.fetchOfficialAgentFlowTemplateCatalog.mockResolvedValue({
      remote_available: false,
      remote_error: 'network failure',
      templates: [
        entry({
          currentReleaseVersion: 1,
          localVersions: [1],
          remoteVersions: []
        })
      ]
    });
    renderPage();
    expect(await screen.findByText('远程模板源当前不可用')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '导入-Fusion' }));
    await waitFor(() => {
      expect(
        templatesApi.previewOfficialAgentFlowTemplate
      ).toHaveBeenCalledWith('fusion', 1, 'csrf-123');
    });
    expect(screen.queryByText('network failure')).not.toBeInTheDocument();
  });
});
