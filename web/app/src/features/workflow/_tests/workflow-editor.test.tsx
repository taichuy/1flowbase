import { fireEvent, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';
import { renderReactFlowScene } from '../../../test/renderers/render-react-flow-scene';
import { WorkflowEditorAssembly } from '../components/WorkflowEditorAssembly';

function createWorkflowInitialState() {
  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-15T09:00:00Z',
      document: createDefaultWorkflowDocument({ flowId: 'flow-1' })
    },
    versions: [],
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
  };
}

const workflowTriggerContext = {
  applicationId: 'app-1',
  triggerType: 'schedule' as const,
  mapping: undefined,
  schedule: null
};

describe('WorkflowEditor assembly', () => {
  test('AC-001/005 uses workflow node picker options for workflow documents', async () => {
    renderReactFlowScene(
      <WorkflowEditorAssembly
        applicationId="app-1"
        applicationName="Ticket Workflow"
        workflowTriggerContext={workflowTriggerContext}
        initialState={createWorkflowInitialState()}
      />
    );

    fireEvent.click(
      screen.getByRole('button', { name: '在 Workflow Start 后新增节点' })
    );

    expect(
      await screen.findByRole('menuitem', { name: 'Workflow Start' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('menuitem', { name: 'Workflow End' })
    ).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'LLM' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: 'Code' })).toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: 'Start' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('menuitem', { name: 'Answer' })
    ).not.toBeInTheDocument();
  }, 20_000);

  test('AC-004 hides conversation tooling for workflow editors', () => {
    renderReactFlowScene(
      <WorkflowEditorAssembly
        applicationId="app-1"
        applicationName="Ticket Workflow"
        workflowTriggerContext={workflowTriggerContext}
        initialState={createWorkflowInitialState()}
      />
    );

    expect(
      screen.queryByRole('button', { name: '预览' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '会话变量' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '系统变量' })
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '环境变量' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '测试运行' })
    ).toBeInTheDocument();
    expect(screen.queryByText('执行此节点')).not.toBeInTheDocument();
  }, 20_000);
});
