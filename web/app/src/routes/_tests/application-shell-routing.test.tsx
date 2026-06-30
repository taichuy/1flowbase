import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { Grid } from 'antd';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import { ApiClientError } from '@1flowbase/api-client';
import { AppRouterProvider } from '../../app/router';
import { resetAuthStore, useAuthStore } from '../../state/auth-store';
import { renderReactFlowScene } from '../../test/renderers/render-react-flow-scene';

const ROUTE_EDITOR_WAIT_OPTIONS = { timeout: 20_000 };
const ROUTE_EDITOR_TEST_TIMEOUT = 40_000;

const applicationApi = vi.hoisted(() => ({
  applicationsQueryKey: ['applications'],
  applicationCatalogQueryKey: ['applications', 'catalog'],
  applicationDetailQueryKey: (applicationId: string) => ['applications', applicationId],
  applicationEnvironmentVariablesQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'environment-variables'
  ],
  getApplicationsApiBaseUrl: vi.fn().mockReturnValue('http://127.0.0.1:7800'),
  fetchApplications: vi.fn(),
  fetchApplicationCatalog: vi.fn(),
  createApplication: vi.fn(),
  updateApplication: vi.fn(),
  createApplicationTag: vi.fn(),
  fetchApplicationDetail: vi.fn(),
  fetchApplicationEnvironmentVariables: vi.fn(),
  replaceApplicationEnvironmentVariables: vi.fn()
}));

vi.mock('../../features/applications/api/applications', () => applicationApi);

const publicApi = vi.hoisted(() => ({
  applicationApiKeysQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'keys'
  ],
  applicationApiMappingQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'mapping'
  ],
  applicationApiPublicationQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'public-api',
    'publication'
  ],
  workflowScheduleTriggerQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'workflow',
    'schedule-trigger'
  ],
  applicationApiDocsCatalogQueryKey: (applicationId: string, locale?: string | null) => [
    'applications',
    applicationId,
    'public-api',
    'docs',
    'catalog',
    locale ?? 'default'
  ],
  applicationApiDocsCategoryOperationsQueryKey: (
    applicationId: string,
    categoryId: string,
    locale?: string | null
  ) => [
    'applications',
    applicationId,
    'public-api',
    'docs',
    'category',
    categoryId,
    'operations',
    locale ?? 'default'
  ],
  applicationApiDocsOperationSpecQueryKey: (
    applicationId: string,
    operationId: string,
    locale?: string | null
  ) => [
    'applications',
    applicationId,
    'public-api',
    'docs',
    'operation',
    operationId,
    'openapi',
    locale ?? 'default'
  ],
  fetchApplicationApiKeys: vi.fn(),
  createApplicationApiKey: vi.fn(),
  revokeApplicationApiKey: vi.fn(),
  fetchApplicationApiMapping: vi.fn(),
  fetchApplicationApiPublication: vi.fn(),
  publishApplicationApiVersion: vi.fn(),
  setApplicationApiEnabled: vi.fn(),
  fetchWorkflowScheduleTrigger: vi.fn(),
  saveWorkflowScheduleTrigger: vi.fn(),
  fetchApplicationApiDocsCatalog: vi.fn(),
  fetchApplicationApiDocsCategoryOperations: vi.fn(),
  fetchApplicationApiDocsOperationSpec: vi.fn(),
  getApplicationApiDocsLocale: vi.fn()
}));

vi.mock('../../features/applications/api/public-api', () => publicApi);

const orchestrationApi = vi.hoisted(() => ({
  orchestrationQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'orchestration'
  ],
  fetchOrchestrationState: vi.fn(),
  saveDraft: vi.fn(),
  restoreVersion: vi.fn()
}));

vi.mock('../../features/agent-flow/api/orchestration', () => orchestrationApi);

const nodeContributionsApi = vi.hoisted(() => ({
  nodeContributionsQueryKey: (applicationId: string) => [
    'applications',
    applicationId,
    'node-contributions'
  ],
  fetchNodeContributions: vi.fn()
}));

vi.mock(
  '../../features/agent-flow/api/node-contributions',
  () => nodeContributionsApi
);

import * as runtimeApi from '../../features/agent-flow/api/runtime';

function authenticate() {
  useAuthStore.getState().setAuthenticated({
    csrfToken: 'csrf-123',
    actor: {
      id: 'user-1',
      account: 'manager',
      effective_display_role: 'manager',
      current_workspace_id: 'workspace-1'
    },
    me: {
      id: 'user-1',
      account: 'manager',
      email: 'manager@example.com',
      phone: null,
      nickname: 'Manager',
      name: 'Manager',
      avatar_url: null,
      introduction: '',
      effective_display_role: 'manager',
      permissions: ['route_page.view.all', 'application.view.all']
    }
  });
}

function renderApplicationRouter() {
  return renderReactFlowScene(<AppRouterProvider />);
}

function createWorkflowApplicationDetail() {
  return {
    id: 'app-1',
    application_type: 'workflow',
    name: 'Ticket Workflow',
    description: 'ticket routing',
    icon: 'RobotOutlined',
    icon_type: 'iconfont',
    icon_background: '#E6F7F2',
    created_by: 'user-1',
    updated_at: '2026-04-15T09:00:00Z',
    tags: [],
    sections: {
      orchestration: {
        status: 'planned',
        subject_kind: 'workflow',
        subject_status: 'unconfigured',
        current_subject_id: null,
        current_draft_id: null
      },
      api: {
        status: 'planned',
        credential_kind: 'application_api_key',
        invoke_routing_mode: 'api_key_bound_application',
        invoke_path_template: null,
        api_capability_status: 'planned',
        credentials_status: 'planned'
      },
      logs: {
        status: 'planned',
        runs_capability_status: 'planned',
        run_object_kind: 'application_run',
        log_retention_status: 'planned'
      },
      monitoring: {
        status: 'planned',
        metrics_capability_status: 'planned',
        metrics_object_kind: 'application_metrics',
        tracing_config_status: 'planned'
      }
    }
  };
}

describe('application shell routing', () => {
  beforeEach(() => {
    resetAuthStore();
    authenticate();
    applicationApi.fetchApplications.mockReset();
    applicationApi.fetchApplications.mockResolvedValue([]);
    applicationApi.fetchApplicationCatalog.mockReset();
    applicationApi.fetchApplicationCatalog.mockResolvedValue({
      types: [{ value: 'agent_flow', label: 'AgentFlow' }],
      tags: []
    });
    applicationApi.fetchApplicationDetail.mockReset();
    applicationApi.fetchApplicationDetail.mockResolvedValue({
      id: 'app-1',
      application_type: 'agent_flow',
      name: 'Support Agent',
      description: 'customer support',
      icon: 'RobotOutlined',
      icon_type: 'iconfont',
      icon_background: '#E6F7F2',
      created_by: 'user-1',
      updated_at: '2026-04-15T09:00:00Z',
      tags: [],
      sections: {
        orchestration: {
          status: 'planned',
          subject_kind: 'agent_flow',
          subject_status: 'unconfigured',
          current_subject_id: null,
          current_draft_id: null
        },
        api: {
          status: 'planned',
          credential_kind: 'application_api_key',
          invoke_routing_mode: 'api_key_bound_application',
          invoke_path_template: null,
          api_capability_status: 'planned',
          credentials_status: 'planned'
        },
        logs: {
          status: 'planned',
          runs_capability_status: 'planned',
          run_object_kind: 'application_run',
          log_retention_status: 'planned'
        },
        monitoring: {
          status: 'planned',
          metrics_capability_status: 'planned',
          metrics_object_kind: 'application_metrics',
          tracing_config_status: 'planned'
        }
      }
    });
    applicationApi.fetchApplicationEnvironmentVariables.mockReset();
    applicationApi.fetchApplicationEnvironmentVariables.mockResolvedValue([]);
    applicationApi.replaceApplicationEnvironmentVariables.mockReset();
    applicationApi.replaceApplicationEnvironmentVariables.mockResolvedValue([]);
    publicApi.fetchApplicationApiKeys.mockReset();
    publicApi.fetchApplicationApiKeys.mockResolvedValue([]);
    publicApi.fetchApplicationApiMapping.mockReset();
    publicApi.fetchApplicationApiMapping.mockResolvedValue({
      input: {
        query_target: 'node-start.query',
        model_target: 'node-start.model',
        inputs_target: 'node-start',
        history_target: 'node-start.history',
        attachments_target: 'node-start.files'
      },
      output: {
        answer_selector: 'node-answer.answer',
        usage_selector: null,
        files_selector: null,
        error_selector: null
      }
    });
    publicApi.fetchApplicationApiPublication.mockReset();
    publicApi.fetchApplicationApiPublication.mockResolvedValue(null);
    publicApi.publishApplicationApiVersion.mockReset();
    publicApi.setApplicationApiEnabled.mockReset();
    publicApi.fetchWorkflowScheduleTrigger.mockReset();
    publicApi.fetchWorkflowScheduleTrigger.mockResolvedValue(null);
    publicApi.saveWorkflowScheduleTrigger.mockReset();
    publicApi.fetchApplicationApiDocsCatalog.mockReset();
    publicApi.fetchApplicationApiDocsCatalog.mockResolvedValue({
      categories: []
    });
    publicApi.fetchApplicationApiDocsCategoryOperations.mockReset();
    publicApi.fetchApplicationApiDocsCategoryOperations.mockResolvedValue({
      operations: []
    });
    publicApi.fetchApplicationApiDocsOperationSpec.mockReset();
    publicApi.fetchApplicationApiDocsOperationSpec.mockResolvedValue({});
    publicApi.getApplicationApiDocsLocale.mockReset();
    publicApi.getApplicationApiDocsLocale.mockReturnValue('zh_Hans');
    orchestrationApi.fetchOrchestrationState.mockReset();
    orchestrationApi.fetchOrchestrationState.mockResolvedValue({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-15T09:00:00Z',
        document: {
          schemaVersion: '1flowbase.flow/v2',
          meta: {
            flowId: 'flow-1',
            name: 'Untitled agentFlow',
            description: '',
            tags: []
          },
          graph: {
            nodes: [],
            edges: []
          },
          editor: {
            viewport: { x: 0, y: 0, zoom: 1 },
            annotations: [],
            activeContainerPath: []
          }
        }
      },
      versions: [],
      autosave_interval_seconds: 30
    });
    nodeContributionsApi.fetchNodeContributions.mockReset();
    nodeContributionsApi.fetchNodeContributions.mockResolvedValue([]);
    vi.spyOn(runtimeApi, 'fetchDebugVariableSnapshot').mockResolvedValue({
      variable_cache: {}
    });
    orchestrationApi.saveDraft.mockReset();
    orchestrationApi.saveDraft.mockResolvedValue({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-15T09:10:00Z',
        document: {
          schemaVersion: '1flowbase.flow/v2',
          meta: {
            flowId: 'flow-1',
            name: 'Untitled agentFlow',
            description: '',
            tags: []
          },
          graph: {
            nodes: [],
            edges: []
          },
          editor: {
            viewport: { x: 0, y: 0, zoom: 1 },
            annotations: [],
            activeContainerPath: []
          }
        }
      },
      versions: [],
      autosave_interval_seconds: 30
    });
  });

  test('redirects /applications/:id to orchestration', async () => {
    window.history.pushState({}, '', '/applications/app-1');
    renderApplicationRouter();

    await waitFor(() => {
      expect(window.location.pathname).toBe('/applications/app-1/orchestration');
    });
  });

  test('renders section navigation for the API route', async () => {
    window.history.pushState({}, '', '/applications/app-1/api');
    renderApplicationRouter();

    expect(
      await screen.findByRole('heading', { name: 'Support Agent', level: 4 })
    ).toBeInTheDocument();
    const sectionNavigation = screen.getByRole('navigation', {
      name: 'Section navigation'
    });
    expect(sectionNavigation).toBeInTheDocument();
    expect(within(sectionNavigation).getByRole('link', { name: 'API' })).toHaveAttribute(
      'href',
      '/applications/app-1/api'
    );
    expect(applicationApi.fetchApplicationDetail).toHaveBeenCalledWith('app-1');
  });

  test('renders workflow sections without the AgentFlow API section', async () => {
    applicationApi.fetchApplicationDetail.mockResolvedValue(
      createWorkflowApplicationDetail()
    );

    window.history.pushState({}, '', '/applications/app-1/logs');
    renderApplicationRouter();

    expect(
      await screen.findByRole('heading', { name: 'Ticket Workflow', level: 4 })
    ).toBeInTheDocument();
    const sectionNavigation = screen.getByRole('navigation', {
      name: 'Section navigation'
    });

    expect(
      within(sectionNavigation).getByRole('link', { name: '工作流' })
    ).toHaveAttribute('href', '/applications/app-1/orchestration');
    expect(
      within(sectionNavigation).getByRole('link', { name: '日志' })
    ).toHaveAttribute('href', '/applications/app-1/logs');
    expect(
      within(sectionNavigation).getByRole('link', { name: '监控' })
    ).toHaveAttribute('href', '/applications/app-1/monitoring');
    expect(
      within(sectionNavigation).queryByRole('link', { name: 'API' })
    ).not.toBeInTheDocument();
  });

  test('redirects workflow API section requests back to the workflow editor', async () => {
    applicationApi.fetchApplicationDetail.mockResolvedValue(
      createWorkflowApplicationDetail()
    );

    window.history.pushState({}, '', '/applications/app-1/api');
    renderApplicationRouter();

    await waitFor(() => {
      expect(window.location.pathname).toBe('/applications/app-1/orchestration');
    });
    expect(publicApi.fetchApplicationApiDocsCatalog).not.toHaveBeenCalled();
  });

  test('renders the editor page inside orchestration', async () => {
    const desktopBreakpoints = vi
      .spyOn(Grid, 'useBreakpoint')
      .mockReturnValue({ lg: true } as never);

    window.history.pushState({}, '', '/applications/app-1/orchestration');

    try {
      renderApplicationRouter();

      expect(
        await screen.findByRole('button', { name: '保存' }, ROUTE_EDITOR_WAIT_OPTIONS)
      ).toBeInTheDocument();
      expect(screen.queryByText('30 秒自动保存')).not.toBeInTheDocument();
      expect(screen.getByRole('button', { name: 'Issues' })).toBeInTheDocument();
    } finally {
      desktopBreakpoints.mockRestore();
    }
  }, ROUTE_EDITOR_TEST_TIMEOUT);

  test('keeps orchestration draft save enabled when application api capability is planned', async () => {
    const desktopBreakpoints = vi
      .spyOn(Grid, 'useBreakpoint')
      .mockReturnValue({ lg: true } as never);

    window.history.pushState({}, '', '/applications/app-1/orchestration');

    try {
      renderApplicationRouter();

      fireEvent.click(
        await screen.findByRole('button', { name: '保存' }, ROUTE_EDITOR_WAIT_OPTIONS)
      );

      await waitFor(() => {
        expect(orchestrationApi.saveDraft).toHaveBeenCalledTimes(1);
      });
    } finally {
      desktopBreakpoints.mockRestore();
    }
  }, ROUTE_EDITOR_TEST_TIMEOUT);

  test('renders formal 403 state for inaccessible applications', async () => {
    applicationApi.fetchApplicationDetail.mockRejectedValue(
      new ApiClientError({ status: 403, message: 'forbidden' })
    );

    window.history.pushState({}, '', '/applications/app-1/logs');
    renderApplicationRouter();

    expect(await screen.findByText('无权限访问')).toBeInTheDocument();
  });
});
