import { screen } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import * as applicationsApi from '../../applications/api/applications';
import * as publicApi from '../../applications/api/public-api';
import * as applicationNodeCatalogApi from '../../agent-flow/api/application-node-catalog';
import * as orchestrationApi from '../../agent-flow/api/orchestration';
import { renderReactFlowScene } from '../../../test/renderers/render-react-flow-scene';
import { WorkflowEditorPage } from '../pages/WorkflowEditorPage';

test('AC-002 keeps the thinking loading state while workflow data loads', () => {
  vi.spyOn(orchestrationApi, 'fetchOrchestrationState').mockImplementation(
    () => new Promise(() => undefined)
  );
  vi.spyOn(
    applicationNodeCatalogApi,
    'fetchApplicationNodeCatalog'
  ).mockResolvedValue({ nodes: [] });
  vi.spyOn(
    applicationsApi,
    'fetchApplicationEnvironmentVariables'
  ).mockResolvedValue([]);
  vi.spyOn(publicApi, 'fetchApplicationApiMapping').mockImplementation(
    () => new Promise(() => undefined)
  );
  vi.spyOn(publicApi, 'fetchWorkflowScheduleTrigger').mockImplementation(
    () => new Promise(() => undefined)
  );

  renderReactFlowScene(
    <WorkflowEditorPage
      applicationId="app-1"
      applicationName="Ticket Workflow"
      workflowTriggerType="schedule"
    />
  );

  expect(screen.getByRole('status', { name: 'thinking' })).toHaveClass(
    'loading-state--compact'
  );
  expect(screen.queryByText('正在加载编排')).not.toBeInTheDocument();
});
