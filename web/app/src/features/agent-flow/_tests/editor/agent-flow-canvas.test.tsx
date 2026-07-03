import { fireEvent, screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import {
  createDefaultAgentFlowDocument,
  createDefaultWorkflowDocument
} from '@1flowbase/flow-schema';
import { AgentFlowEditorShell } from '../../components/editor/AgentFlowEditorShell';
import { renderReactFlowScene } from '../../../../test/renderers/render-react-flow-scene';

function createInitialState() {
  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-15T09:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    },
    versions: [],
    autosave_interval_seconds: 30
  };
}

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
    autosave_interval_seconds: 30
  };
}

describe('AgentFlowCanvas', () => {
  test('adds a node from the plus picker after the selected node', async () => {
    renderReactFlowScene(
      <AgentFlowEditorShell
        applicationId="app-1"
        applicationName="Support Agent"
        initialState={createInitialState()}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '在 LLM 后新增节点' }));
    fireEvent.click(
      await screen.findByRole('menuitem', { name: 'Template Transform' })
    );

    expect(screen.getAllByText('Template Transform').length).toBeGreaterThan(0);
  }, 20_000);

  test('uses workflow node picker options for workflow documents', async () => {
    renderReactFlowScene(
      <AgentFlowEditorShell
        applicationId="app-1"
        applicationName="Ticket Workflow"
        applicationType="workflow"
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

  test('focuses the iteration child canvas and returns through breadcrumb', async () => {
    const baseDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const iterationState = {
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-15T09:00:00Z',
        document: {
          ...baseDocument,
          graph: {
            nodes: [
              ...baseDocument.graph.nodes,
              {
                id: 'node-iteration',
                type: 'iteration' as const,
                alias: 'Iteration',
                containerId: null,
                position: { x: 920, y: 220 },
                configVersion: 1,
                config: {},
                bindings: {},
                outputs: [
                  { key: 'result', title: '聚合输出', valueType: 'array' }
                ]
              },
              {
                id: 'node-inner-answer',
                type: 'answer' as const,
                alias: 'Inner Answer',
                containerId: 'node-iteration',
                position: { x: 360, y: 220 },
                configVersion: 1,
                config: {},
                bindings: {},
                outputs: [
                  { key: 'answer', title: '对话输出', valueType: 'string' }
                ]
              }
            ],
            edges: baseDocument.graph.edges
          }
        }
      },
      versions: [],
      autosave_interval_seconds: 30
    };

    renderReactFlowScene(
      <AgentFlowEditorShell
        applicationId="app-1"
        applicationName="Support Agent"
        initialState={iterationState}
      />
    );

    fireEvent.doubleClick(await screen.findByText('Iteration'));
    expect(
      screen.getByRole('button', { name: '返回主画布' })
    ).toBeInTheDocument();
    expect(screen.getByRole('textbox', { name: '节点别名' })).toHaveValue(
      'Inner Answer'
    );
    expect(screen.queryByText('Start')).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '返回主画布' }));
    expect(screen.getByText('Start')).toBeInTheDocument();
  }, 15_000);
});
