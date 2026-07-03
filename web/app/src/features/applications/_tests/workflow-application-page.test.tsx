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
  saveApplicationApiMapping: vi.fn(),
  saveWorkflowScheduleTrigger: vi.fn()
}));

vi.mock('../api/public-api', () => publicApi);

vi.mock('../../../shared/ui/section-page-layout/SectionPageLayout', () => ({
  SectionPageLayout: ({ children }: { children: ReactNode }) => (
    <main>{children}</main>
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
      autosave_interval_seconds: 30
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

function createWorkflowApplicationDetail() {
  return {
    id: 'app-workflow',
    application_type: 'workflow',
    workflow_trigger_type: 'extension',
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
        status: 'hidden',
        credential_kind: 'api_key',
        invoke_routing_mode: 'disabled',
        invoke_path_template: null,
        api_capability_status: 'disabled',
        credentials_status: 'disabled'
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
    publicApi.fetchApplicationApiMapping.mockResolvedValue(
      createMappingWithoutExtension()
    );
    publicApi.fetchApplicationApiPublication.mockResolvedValue({
      active: false
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
});
