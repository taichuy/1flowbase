import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

const templatesApi = vi.hoisted(() => ({
  officialAgentFlowTemplateCatalogQueryKey: ['templates', 'official-agent-flow'],
  officialAgentFlowTemplateCatalogStaleTimeMs: 7_200_000,
  fetchOfficialAgentFlowTemplateCatalog: vi.fn(),
  downloadOfficialAgentFlowTemplate: vi.fn()
}));

const applicationsApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  importAgentFlowTemplate: vi.fn(),
  previewAgentFlowTemplate: vi.fn()
}));

vi.mock('../api/templates', () => templatesApi);
vi.mock('../../applications/api/applications', () => applicationsApi);

import { AppProviders } from '../../../app/AppProviders';
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

function createTemplatePackage() {
  return {
    schema_version: '1flowbase.application-template/v1' as const,
    application: {
      application_type: 'agent_flow' as const,
      name: '多模态挂载测试',
      description: '',
      icon: 'RobotOutlined',
      icon_type: 'iconfont',
      icon_background: '#E6F7F2'
    },
    flow_document: {
      schemaVersion: '1flowbase.flow/v2',
      meta: {
        flowId: 'flow-template',
        name: '多模态挂载测试',
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
}

function renderPage() {
  return render(
    <AppProviders>
      <TemplatesPage />
    </AppProviders>
  );
}

describe('TemplatesPage', () => {
  beforeEach(async () => {
    window.localStorage.clear();
    await appI18n.changeLanguage('zh_Hans');
    resetAuthStore();
    authenticate();

    const template = createTemplatePackage();

    templatesApi.fetchOfficialAgentFlowTemplateCatalog.mockResolvedValue({
      source: {
        source_kind: 'official',
        source_label: '官方源',
        index_url: 'https://proxy.example.com/index.json'
      },
      page: {
        page: 1,
        page_size: 100,
        next_cursor: null
      },
      entries: [
        {
          workflow_id: 'multimodal-mount-test',
          schema_version: '1flowbase.application-template/v1',
          application: template.application,
          template_url:
            'https://proxy.example.com/agent-flow/workflows/multimodal-mount-test/template.json',
          template_sha256: 'sha256:abc123',
          updated_at: '2026-06-16T00:00:00.000Z',
          dependency_summary: '这个摘要不应该展示',
          tags: ['support'],
          author: '1flowbase',
          status: 'published'
        }
      ]
    });
    templatesApi.downloadOfficialAgentFlowTemplate.mockResolvedValue(template);
    applicationsApi.previewAgentFlowTemplate.mockResolvedValue({
      schema_version: '1flowbase.application-template/v1',
      application: template.application,
      dependencies: [],
      unresolved_nodes: [],
      document: template.flow_document
    });
    applicationsApi.importAgentFlowTemplate.mockReturnValue(new Promise(() => undefined));
  });

  afterEach(async () => {
    vi.clearAllMocks();
    await appI18n.changeLanguage('en_US');
  });

  test('lists official AgentFlow templates and imports through the backend download route', async () => {
    renderPage();

    expect(await screen.findByRole('heading', { name: '模板' })).toBeInTheDocument();
    expect(await screen.findByText('多模态挂载测试')).toBeInTheDocument();
    expect(screen.getByText('multimodal-mount-test')).toBeInTheDocument();
    expect(screen.getByText('sha256:abc123')).toBeInTheDocument();
    expect(screen.queryByText('这个摘要不应该展示')).not.toBeInTheDocument();
    expect(screen.queryByText('published')).not.toBeInTheDocument();
    expect(screen.queryByText('1flowbase')).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', { name: '导入模板-多模态挂载测试' })
    );

    await waitFor(
      () => {
        expect(templatesApi.downloadOfficialAgentFlowTemplate).toHaveBeenCalledWith(
          'multimodal-mount-test'
        );
        expect(applicationsApi.previewAgentFlowTemplate).toHaveBeenCalledWith(
          createTemplatePackage()
        );
      },
      { timeout: 10_000 }
    );

    const dialog = await screen.findByRole('dialog', undefined, { timeout: 10_000 });
    expect(within(dialog).getByText('导入 AgentFlow 模板')).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: '导入模板' }));

    await waitFor(
      () => {
        expect(applicationsApi.importAgentFlowTemplate).toHaveBeenCalledWith(
          {
            template: createTemplatePackage(),
            name: '多模态挂载测试',
            description: ''
          },
          'csrf-123'
        );
      },
      { timeout: 10_000 }
    );
  }, 15_000);
});
