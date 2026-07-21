import { render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const applicationsApi = vi.hoisted(() => ({
  applicationDetailQueryKey: (applicationId: string) => [
    'applications',
    applicationId
  ],
  fetchApplicationDetail: vi.fn()
}));

vi.mock('../api/applications', () => applicationsApi);

const publicApi = vi.hoisted(() => ({
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
  fetchApplicationApiMapping: vi.fn(),
  fetchApplicationApiPublication: vi.fn(),
  fetchWorkflowScheduleTrigger: vi.fn(),
  publishApplicationApiVersion: vi.fn(),
  unpublishApplicationApiVersion: vi.fn(),
  saveApplicationApiMapping: vi.fn(),
  saveWorkflowScheduleTrigger: vi.fn()
}));

vi.mock('../api/public-api', () => publicApi);

vi.mock('../../../shared/ui/section-page-layout/SectionPageLayout', () => ({
  SectionPageLayout: ({
    children,
    navItems
  }: {
    children: ReactNode;
    navItems: Array<{ key: string; label: string }>;
  }) => (
    <main>
      <nav aria-label="Application sections">
        {navItems.map((item) => (
          <span key={item.key}>{item.label}</span>
        ))}
      </nav>
      {children}
    </main>
  )
}));

vi.mock('../../workflow/pages/WorkflowEditorPage', async () => {
  const { createDefaultWorkflowDocument } =
    await import('@1flowbase/flow-schema');
  const { AgentFlowEditorStoreProvider } =
    await import('../../agent-flow/store/editor/AgentFlowEditorStoreProvider');

  function createInitialState() {
    return {
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-07-02T09:00:00Z',
        document: createDefaultWorkflowDocument({ flowId: 'flow-1' })
      },
      versions: [],
      autosave_interval_seconds: 30,
      user_protection_limit: 10,
    };
  }

  return {
    WorkflowEditorPage: ({
      workflowTriggerType
    }: {
      workflowTriggerType?: string | null;
    }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        <div>Workflow trigger type: {workflowTriggerType}</div>
        <div>Workflow editor shell</div>
      </AgentFlowEditorStoreProvider>
    )
  };
});

import { AppProviders } from '../../../app/AppProviders';

import { ApplicationDetailPage } from '../pages/ApplicationDetailPage';

function createWorkflowApplicationDetail(
  workflowTriggerType: 'extension' | 'schedule' = 'extension'
) {
  const apiAvailable = workflowTriggerType === 'extension';
  return {
    id: 'app-workflow',
    application_type: 'workflow',
    workflow_trigger_type: workflowTriggerType,
    name: 'Order workflow',
    description: '',
    icon: null,
    icon_type: null,
    icon_background: null,
    created_by: 'user-1',
    updated_at: '2026-07-02T09:00:00Z',
    tags: [],
    sections: {
      orchestration: {
        status: 'ready',
        subject_kind: 'flow',
        subject_status: 'draft',
        current_subject_id: 'flow-1',
        current_draft_id: 'draft-1'
      },
      api: {
        status: apiAvailable ? 'available' : 'unavailable',
        credential_kind: apiAvailable ? 'user_or_public' : 'not_applicable',
        invoke_routing_mode: apiAvailable
          ? 'published_workflow_operation'
          : 'not_available',
        invoke_path_template: apiAvailable ? '/api/ex/{operation}' : null,
        api_capability_status: apiAvailable ? 'available' : 'unavailable',
        credentials_status: apiAvailable ? 'not_required' : 'not_applicable'
      },
      logs: {
        status: 'ready',
        runs_capability_status: 'ready',
        run_object_kind: 'application_run',
        log_retention_status: 'default'
      },
      monitoring: {
        status: 'ready',
        metrics_capability_status: 'ready',
        metrics_object_kind: 'application_metrics',
        tracing_config_status: 'default'
      }
    }
  };
}

function createMappingWithoutExtension() {
  return {
    input: {
      query_target: 'node-workflow-start.query',
      model_target: null,
      inputs_target: 'node-workflow-start.inputs',
      history_target: null,
      attachments_target: null
    },
    output: {
      answer_selector: null,
      usage_selector: null,
      files_selector: null,
      error_selector: null
    }
  };
}

describe('Workflow application page', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    applicationsApi.fetchApplicationDetail.mockResolvedValue(
      createWorkflowApplicationDetail()
    );
    publicApi.fetchApplicationApiMapping.mockResolvedValue({
      ...createMappingWithoutExtension(),
      extension: {
        slug: 'orders/{order_id}',
        method: 'POST',
        response_mode: 'sync'
      }
    });
    publicApi.fetchApplicationApiPublication.mockResolvedValue({
      active: true,
      api_enabled: true,
      operation: {
        interface_id: 'published_workflow_operation:app-workflow',
        method: 'POST',
        route_template: 'orders/{order_id}',
        response_mode: 'sync',
        parameter_schema: {
          type: 'object',
          properties: {
            path: {
              type: 'object',
              properties: { order_id: { type: 'string' } }
            }
          }
        },
        result_schema: {
          type: 'object',
          properties: { accepted: { type: 'boolean' } }
        }
      }
    });
    publicApi.fetchWorkflowScheduleTrigger.mockResolvedValue(null);
  });

  test('passes workflow trigger type to the editor without a top-level trigger configuration entry', async () => {
    render(
      <AppProviders>
        <ApplicationDetailPage
          applicationId="app-workflow"
          requestedSectionKey="orchestration"
        />
      </AppProviders>
    );

    expect(
      await screen.findByText('Workflow trigger type: extension')
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '触发器配置' })
    ).not.toBeInTheDocument();
  });

  test('AC-010 extension renders the published operation and never renders AgentFlow API tooling', async () => {
    render(
      <AppProviders>
        <ApplicationDetailPage
          applicationId="app-workflow"
          requestedSectionKey="api"
        />
      </AppProviders>
    );

    expect(
      await screen.findByRole('heading', { name: '工作流扩展 API' })
    ).toBeInTheDocument();
    expect(
      await screen.findByText('published_workflow_operation:app-workflow')
    ).toBeInTheDocument();
    expect(screen.getByText('/api/ex/orders/{order_id}')).toBeInTheDocument();
    expect(screen.getByText('path.order_id')).toBeInTheDocument();
    expect(screen.getByText('accepted')).toBeInTheDocument();
    expect(screen.queryByText('访问策略')).not.toBeInTheDocument();
    expect(screen.getByText('API')).toBeInTheDocument();
    expect(screen.queryByText(/AgentFlow/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/API Key 管理/i)).not.toBeInTheDocument();
  });

  test('AC-001 schedule hides the API entry from backend capability truth', async () => {
    applicationsApi.fetchApplicationDetail.mockResolvedValue(
      createWorkflowApplicationDetail('schedule')
    );

    render(
      <AppProviders>
        <ApplicationDetailPage
          applicationId="app-workflow"
          requestedSectionKey="orchestration"
        />
      </AppProviders>
    );

    expect(await screen.findByText('Workflow editor shell')).toBeInTheDocument();
    expect(
      screen.getByRole('navigation', { name: 'Application sections' })
    ).not.toHaveTextContent('API');
  });
});
