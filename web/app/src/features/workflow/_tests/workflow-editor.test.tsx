import { fireEvent, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { createDefaultWorkflowDocument } from '@1flowbase/flow-schema';
import { renderReactFlowScene } from '../../../test/renderers/render-react-flow-scene';
import { WorkflowEditorAssembly } from '../components/WorkflowEditorAssembly';

function createWorkflowInitialState() {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-15T09:00:00Z',
      document: createDefaultWorkflowDocument({ flowId: 'flow-1' })
    },
    versions: [],
    autosave_interval_seconds: 30,
    user_protection_limit: 10
  };
}

const workflowTriggerContext = {
  applicationId: 'app-1',
  triggerType: 'schedule' as const,
  mapping: undefined,
  schedule: null,
  workflowStartFieldContract: undefined
};

describe('WorkflowEditor assembly', () => {
  test('AC-001/002 renders workflow entry and terminal nodes with directional connectors', () => {
    renderReactFlowScene(
      <WorkflowEditorAssembly
        applicationId="app-1"
        applicationName="Ticket Workflow"
        workflowTriggerContext={workflowTriggerContext}
        initialState={createWorkflowInitialState()}
      />
    );

    const startNode = screen
      .getByText('Workflow Start', { selector: '.agent-flow-node-card__title' })
      .closest('.react-flow__node');
    const endNode = screen
      .getByText('Workflow End', { selector: '.agent-flow-node-card__title' })
      .closest('.react-flow__node');

    expect(startNode).not.toBeNull();
    expect(endNode).not.toBeNull();
    expect(
      startNode?.querySelector('.agent-flow-node-handle--target')
    ).not.toBeInTheDocument();
    expect(
      startNode?.querySelector('.agent-flow-node-handle--source')
    ).toBeInTheDocument();
    expect(
      endNode?.querySelector('.agent-flow-node-handle--target')
    ).toBeInTheDocument();
    expect(
      endNode?.querySelector('.agent-flow-node-handle--source')
    ).not.toBeInTheDocument();
  });

  test('keeps the workflow canvas as the layout root', () => {
    renderReactFlowScene(
      <WorkflowEditorAssembly
        applicationId="app-1"
        applicationName="Ticket Workflow"
        workflowTriggerContext={workflowTriggerContext}
        initialState={createWorkflowInitialState()}
      />
    );

    expect(
      screen.getByRole('region', { name: 'Ticket Workflow workflow editor' })
    ).toHaveClass('agent-flow-editor');
    expect(
      screen.queryByTestId('workflow-editor-assembly')
    ).not.toBeInTheDocument();
  });

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

  test('reuses the standard node detail shell without node preview actions', () => {
    renderReactFlowScene(
      <WorkflowEditorAssembly
        applicationId="app-1"
        applicationName="Ticket Workflow"
        workflowTriggerContext={workflowTriggerContext}
        initialState={createWorkflowInitialState()}
      />
    );

    fireEvent.click(screen.getByText('Workflow Start'));

    expect(screen.getByTestId('node-detail-header')).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '设置' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: '上次运行' })).toBeInTheDocument();
    expect(
      screen.getByRole('separator', { name: '调整节点详情宽度' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '执行此节点' })
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '调试此节点' })
    ).not.toBeInTheDocument();
  });
});
