import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  applicationCatalogQueryKey: ['applications', 'catalog'],
  fetchApplications: vi.fn(),
  fetchApplicationCatalog: vi.fn(),
  createApplication: vi.fn(),
  createApplicationTag: vi.fn(),
  deleteApplication: vi.fn(),
  exportAgentFlowTemplate: vi.fn(),
  importAgentFlowTemplate: vi.fn(),
  previewAgentFlowTemplate: vi.fn(),
  updateApplication: vi.fn()
}));

vi.mock('../api/applications', () => applicationsApi);

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
        { value: 'agent_flow', label: 'AgentFlow' },
        { value: 'workflow', label: '工作流' }
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
    applicationsApi.createApplication.mockResolvedValue({ id: 'app-3' });
    applicationsApi.createApplicationTag.mockResolvedValue({
      id: 'tag-2',
      name: '内部',
      application_count: 0
    });
    applicationsApi.deleteApplication.mockResolvedValue(undefined);
    applicationsApi.exportAgentFlowTemplate.mockReturnValue(new Promise(() => undefined));
    applicationsApi.importAgentFlowTemplate.mockReturnValue(new Promise(() => undefined));
    applicationsApi.previewAgentFlowTemplate.mockResolvedValue({
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
    applicationsApi.updateApplication.mockResolvedValue(undefined);
  });

  afterEach(async () => {
    await appI18n.changeLanguage('en_US');
  });

  test('renders backend-driven type tabs and filters the list', async () => {
    renderPage();

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
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

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '管理标签-客服助手' }));

    const dialog = await screen.findByRole('dialog', undefined, { timeout: 10_000 });
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

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByRole('button', { name: '更多操作-客服助手' }));
    fireEvent.click(await screen.findByText('编辑信息'));

    const dialog = await screen.findByRole('dialog', undefined, { timeout: 10_000 });
    expect(within(dialog).getByText('编辑应用信息')).toBeInTheDocument();
    fireEvent.change(within(dialog).getByLabelText('应用名称'), {
      target: { value: '客服助手 Pro' }
    });
    fireEvent.change(within(dialog).getByLabelText('应用简介'), {
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

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();

    expect(screen.queryByRole('button', { name: '进入应用' })).not.toBeInTheDocument();
    expect(screen.getByRole('link', { name: '进入应用-客服助手' })).toHaveAttribute(
      'href',
      '/applications/app-1/orchestration'
    );
    expect(screen.getByRole('button', { name: '更多操作-客服助手' })).toBeInTheDocument();
  }, 15_000);

  test('copies application metadata from the card action', async () => {
    renderPage();

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByRole('button', { name: '更多操作-客服助手' }));

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

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByRole('button', { name: '更多操作-客服助手' }));
    fireEvent.click(screen.getByText('导出模板'));

    await waitFor(
      () => {
        expect(applicationsApi.exportAgentFlowTemplate).toHaveBeenCalledWith('app-1');
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('previews and imports a selected AgentFlow template file', async () => {
    const template = {
      schema_version: '1flowbase.application-template/v1',
      application: {
        application_type: 'agent_flow',
        name: '导入客服助手',
        description: '导入描述',
        icon: null,
        icon_type: null,
        icon_background: null
      },
      flow_document: {
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
      },
      dependencies: []
    };
    renderPage();

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
    const input = screen.getByLabelText('导入模板文件') as HTMLInputElement;
    const file = new File([JSON.stringify(template)], 'support-template.json', {
      type: 'application/json'
    });
    Object.defineProperty(file, 'text', {
      value: () => Promise.resolve(JSON.stringify(template))
    });

    fireEvent.change(input, { target: { files: [file] } });

    await waitFor(
      () => {
        expect(applicationsApi.previewAgentFlowTemplate).toHaveBeenCalledWith(template);
      },
      { timeout: 10_000 }
    );

    const dialog = await screen.findByRole('dialog', undefined, { timeout: 10_000 });
    expect(within(dialog).getByText('导入 AgentFlow 模板')).toBeInTheDocument();
    expect(within(dialog).getByText('模板依赖已就绪')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: '导入模板' }));

    await waitFor(
      () => {
        expect(applicationsApi.importAgentFlowTemplate).toHaveBeenCalledWith(
          {
            template,
            name: '导入客服助手',
            description: '导入描述'
          },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);

  test('confirms and deletes an application from the card action', async () => {
    renderPage();

    expect(await screen.findByText('客服助手', {}, { timeout: 10_000 })).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByRole('button', { name: '更多操作-客服助手' }));
    fireEvent.click(await screen.findByText('删除'));

    const dialog = await screen.findByRole('dialog', undefined, { timeout: 10_000 });
    expect(within(dialog).getByText(/相关的编排、草稿、运行记录和标签绑定/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: /删\s*除/ }));

    await waitFor(
      () => {
        expect(applicationsApi.deleteApplication).toHaveBeenCalledWith('app-1', 'csrf-123');
      },
      { timeout: 10_000 }
    );
  }, 15_000);
});
