import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  applicationCatalogQueryKey: ['applications', 'catalog'],
  installedAgentFlowsQueryKey: ['applications', 'installed-agent-flows'],
  fetchApplications: vi.fn(),
  fetchApplicationCatalog: vi.fn(),
  fetchApplicationDetail: vi.fn(),
  fetchInstalledAgentFlows: vi.fn(),
  createApplication: vi.fn(),
  createApplicationTag: vi.fn(),
  deleteApplication: vi.fn(),
  exportApplicationArchive: vi.fn(),
  importApplicationArchive: vi.fn(),
  previewApplicationArchive: vi.fn(),
  previewInstalledApplicationExtension: vi.fn(),
  importInstalledApplicationExtension: vi.fn(),
  updateApplication: vi.fn()
}));

vi.mock('../api/applications', async (importOriginal) => ({
  ...(await importOriginal<typeof import('../api/applications')>()),
  ...applicationsApi
}));

import { AppProviders } from '../../../app/AppProviders';
import { appI18n } from '../../../shared/i18n/app-i18n';
import { resetAuthStore, useAuthStore } from '../../../state/auth-store';
import { ApplicationListPage } from '../pages/ApplicationListPage';

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
      permissions: [
        'application.create.all',
        'application.delete.own',
        'application.edit.own',
        'application.view.all'
      ],
      meta: {
        ui: {
          locale: {
            preferred_locale: 'zh_Hans'
          }
        }
      }
    }
  });
}

function renderPage() {
  return render(
    <AppProviders>
      <ApplicationListPage />
    </AppProviders>
  );
}

describe('ApplicationListPage', () => {
  beforeEach(async () => {
    window.localStorage.clear();
    await appI18n.changeLanguage('zh_Hans');
    resetAuthStore();
    authenticate();
    applicationsApi.fetchApplicationCatalog.mockResolvedValue({
      types: [
        {
          value: 'agent_flow',
          label: 'AgentFlow'
        },
        {
          value: 'workflow',
          label: '工作流'
        }
      ],
      workflow_triggers: [
        {
          value: 'extension',
          label: '扩展触发'
        },
        {
          value: 'schedule',
          label: '定时触发'
        }
      ],
      tags: [{ id: 'tag-1', name: '客服', application_count: 1 }]
    });
    applicationsApi.fetchApplications.mockResolvedValue([
      {
        id: 'app-1',
        application_type: 'agent_flow',
        name: '客服助手',
        description: '处理客服',
        icon: null,
        icon_type: null,
        icon_background: null,
        updated_at: '2026-04-16T12:00:00.000Z',
        created_by: 'user-1',
        tags: [{ id: 'tag-1', name: '客服' }]
      },
      {
        id: 'app-2',
        application_type: 'workflow',
        name: '审批流',
        description: '处理审批',
        icon: null,
        icon_type: null,
        icon_background: null,
        updated_at: '2026-04-16T13:00:00.000Z',
        created_by: 'user-2',
        tags: []
      }
    ]);
    applicationsApi.fetchInstalledAgentFlows.mockResolvedValue({
      limit: 50,
      total_entries: 0,
      next_cursor: null,
      entries: []
    });
    applicationsApi.fetchApplicationDetail.mockResolvedValue({
      id: 'app-1',
      application_type: 'agent_flow',
      workflow_trigger_type: null,
      name: '客服助手',
      description: '处理客服',
      icon: null,
      icon_type: null,
      icon_background: null,
      created_by: 'user-1',
      updated_at: '2026-04-16T12:00:00.000Z',
      tags: [{ id: 'tag-1', name: '客服' }],
      sections: {
        orchestration: {
          status: 'ready',
          subject_kind: 'agent_flow',
          subject_status: 'ready',
          current_subject_id: 'flow-1',
          current_draft_id: 'draft-1'
        },
        api: {
          status: 'active',
          credential_kind: 'application_api_key',
          invoke_routing_mode: 'api_key_bound_application',
          invoke_path_template: '/api/agent/v1/runs',
          api_capability_status: 'enabled',
          credentials_status: 'configured'
        },
        logs: {
          status: 'ready',
          runs_capability_status: 'enabled',
          run_object_kind: 'application_run',
          log_retention_status: 'enabled'
        },
        monitoring: {
          status: 'planned',
          metrics_capability_status: 'planned',
          metrics_object_kind: 'application_metrics',
          tracing_config_status: 'not_configured'
        }
      }
    });
    applicationsApi.createApplication.mockResolvedValue({ id: 'app-3' });
    applicationsApi.createApplicationTag.mockResolvedValue({
      id: 'tag-2',
      name: '内部',
      application_count: 0
    });
    applicationsApi.deleteApplication.mockResolvedValue(undefined);
    applicationsApi.exportApplicationArchive.mockReturnValue(
      new Promise(() => undefined)
    );
    applicationsApi.importApplicationArchive.mockReturnValue(
      new Promise(() => undefined)
    );
    applicationsApi.previewApplicationArchive.mockResolvedValue({
      schema_version: '1flowbase.application-template/v1',
      application: {
        application_type: 'agent_flow',
        name: '导入客服助手',
        description: '导入描述',
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
          name: '导入客服助手',
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
    });
    applicationsApi.previewInstalledApplicationExtension.mockResolvedValue({
      extension_installation_id: 'agent-flow-installation-1',
      application_status: 'not_applied',
      integrity_warnings: [],
      required_integrity_override: null,
      preview: {
        application: {
          application_type: 'agent_flow',
          name: '本地客服模板',
          description: '本地模板描述',
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
    applicationsApi.updateApplication.mockResolvedValue(undefined);
  });

  afterEach(async () => {
    await appI18n.changeLanguage('en_US');
  });

  test('renders backend-driven type tabs and filters the list', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    expect(screen.getByText('AgentFlow')).toBeInTheDocument();
    expect(screen.getByText('工作流')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '工作流' }));

    await waitFor(
      () => {
        expect(screen.queryByText('客服助手')).not.toBeInTheDocument();
      },
      { timeout: 10_000 }
    );
    expect(screen.getByText('审批流')).toBeInTheDocument();
  }, 15_000);

  test('creates a new tag from the card dialog and saves it back to the application', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '管理标签-客服助手' }));

    const dialog = await screen.findByRole('dialog', undefined, {
      timeout: 10_000
    });
    expect(within(dialog).getByText('管理应用标签')).toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('新标签名称'), {
      target: { value: '内部' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '创建标签' }));

    await waitFor(
      () => {
        expect(applicationsApi.createApplicationTag).toHaveBeenCalledWith(
          { name: '内部' },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );

    fireEvent.click(within(dialog).getByRole('checkbox', { name: '内部' }));
    fireEvent.click(within(dialog).getByRole('button', { name: '保存标签' }));

    await waitFor(
      () => {
        expect(applicationsApi.updateApplication).toHaveBeenCalledWith(
          'app-1',
          {
            name: '客服助手',
            description: '处理客服',
            tag_ids: ['tag-1', 'tag-2']
          },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('edits application name and description from the card action', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    fireEvent.mouseDown(
      screen.getByRole('button', { name: '更多操作-客服助手' })
    );
    fireEvent.click(await screen.findByText('编辑信息'));

    const dialog = await screen.findByRole('dialog', undefined, {
      timeout: 10_000
    });
    expect(within(dialog).getByText('编辑应用信息')).toBeInTheDocument();
    const nameInput = await within(dialog).findByLabelText(
      '名称',
      {},
      { timeout: 10_000 }
    );
    fireEvent.change(nameInput, {
      target: { value: '客服助手 Pro' }
    });
    fireEvent.change(within(dialog).getByLabelText('简介'), {
      target: { value: '升级后的客服描述' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '保存修改' }));

    await waitFor(
      () => {
        expect(applicationsApi.updateApplication).toHaveBeenCalledWith(
          'app-1',
          {
            name: '客服助手 Pro',
            description: '升级后的客服描述',
            tag_ids: ['tag-1']
          },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('opens the application from the card link instead of a dedicated button', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();

    expect(
      screen.queryByRole('button', { name: '进入应用' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('link', { name: '进入应用-客服助手' })
    ).toHaveAttribute('href', '/applications/app-1/orchestration');
    expect(
      screen.getByRole('button', { name: '更多操作-客服助手' })
    ).toBeInTheDocument();
  }, 15_000);

  test('copies application metadata from the card action', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    fireEvent.mouseDown(
      screen.getByRole('button', { name: '更多操作-客服助手' })
    );

    fireEvent.click(screen.getByText('复制'));

    await waitFor(
      () => {
        expect(applicationsApi.createApplication).toHaveBeenCalledWith(
          {
            application_type: 'agent_flow',
            name: '客服助手 副本',
            description: '处理客服',
            icon: null,
            icon_type: null,
            icon_background: null
          },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('exports an AgentFlow template from the card action', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    fireEvent.mouseDown(
      screen.getByRole('button', { name: '更多操作-客服助手' })
    );
    fireEvent.click(screen.getByText('导出应用'));

    await waitFor(
      () => {
        expect(applicationsApi.exportApplicationArchive).toHaveBeenCalledWith([
          'app-1'
        ]);
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('AC-001 accepts an application JSON file and previews it for import', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    const input = screen.getByLabelText('导入') as HTMLInputElement;
    expect(input).toHaveAttribute(
      'accept',
      'application/zip,.zip,application/json,.json'
    );
    expect(
      screen.getByText('支持应用压缩包与应用模板 JSON 文件')
    ).toBeInTheDocument();
    const file = new File(['{}'], 'support-application.json', {
      type: 'application/json'
    });

    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(
      () => {
        expect(applicationsApi.previewApplicationArchive).toHaveBeenCalledWith(
          file
        );
      },
      { timeout: 10_000 }
    );

    const dialog = await screen.findByRole('dialog', undefined, {
      timeout: 10_000
    });
    expect(within(dialog).getByText('导入')).toBeInTheDocument();
    expect(within(dialog).getByText('应用依赖已就绪')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: '导入应用' }));

    await waitFor(
      () => {
        expect(applicationsApi.importApplicationArchive).toHaveBeenCalledWith(
          file,
          {
            name: '导入客服助手',
            description: '导入描述'
          },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('AC-007 selects a local installed Agent Flow and reuses preview/import without remote discovery', async () => {
    applicationsApi.fetchInstalledAgentFlows.mockResolvedValue({
      limit: 50,
      total_entries: 1,
      next_cursor: null,
      entries: [
        {
          id: 'agent-flow-installation-1',
          category: 'agent-flow',
          catalog_id: 'agent-flow:taichuy/support',
          organization: 'taichuy',
          artifact_id: 'support',
          version: '1.0.0',
          status: 'installed',
          application_action: 'import_agent_flow'
        }
      ]
    });
    renderPage();

    fireEvent.click(
      await screen.findByRole('button', { name: '从应用模板创建' })
    );
    const drawer = await screen.findByRole('dialog');
    expect(
      within(drawer).getByRole('link', { name: '管理本地模板' })
    ).toHaveAttribute('href', '/templates');
    fireEvent.click(
      await within(drawer).findByRole('button', { name: '导入应用' })
    );

    await waitFor(() => {
      expect(
        applicationsApi.previewInstalledApplicationExtension
      ).toHaveBeenCalledWith('agent-flow-installation-1');
    });
    const importTitle = (await screen.findAllByText('导入')).find(
      (element) => element.closest('[role="dialog"]')
    );
    const importDialog = importTitle?.closest('[role="dialog"]') ?? null;
    expect(importDialog).not.toBeNull();
    fireEvent.click(
      within(importDialog as HTMLElement).getByRole('button', {
        name: '导入应用'
      })
    );
    await waitFor(() => {
      expect(
        applicationsApi.importInstalledApplicationExtension
      ).toHaveBeenCalledWith(
        'agent-flow-installation-1',
        { name: '本地客服模板', description: '本地模板描述' },
        'csrf-123'
      );
    });
  }, 15_000);

  test('AC-007 empty local template picker links to the Agent Flow extension center', async () => {
    renderPage();
    fireEvent.click(
      await screen.findByRole('button', { name: '从应用模板创建' })
    );
    expect(
      await screen.findByRole('link', {
        name: '前往扩展中心安装 Agent Flow 模板'
      })
    ).toHaveAttribute('href', '/settings/extension-center/agent-flow');
  }, 15_000);

  test('confirms and deletes an application from the card action', async () => {
    renderPage();

    expect(
      await screen.findByText('客服助手', {}, { timeout: 10_000 })
    ).toBeInTheDocument();
    fireEvent.mouseDown(
      screen.getByRole('button', { name: '更多操作-客服助手' })
    );
    fireEvent.click(await screen.findByText('删除'));

    const dialog = await screen.findByRole('dialog', undefined, {
      timeout: 10_000
    });
    expect(
      within(dialog).getByText(/相关的编排、草稿、运行记录和标签绑定/)
    ).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: /删\s*除/ }));

    await waitFor(
      () => {
        expect(applicationsApi.deleteApplication).toHaveBeenCalledWith(
          'app-1',
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);
});
