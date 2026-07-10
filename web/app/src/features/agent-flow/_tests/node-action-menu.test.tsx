import type { ReactNode } from 'react';
import { act, fireEvent, render, renderHook, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { NodeActionMenu } from '../components/detail/NodeActionMenu';
import { useNodeDetailActions } from '../hooks/interactions/use-node-detail-actions';
import { AgentFlowEditorStoreProvider } from '../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../store/editor/provider';

function createInitialState() {
  return {
    flow_id: 'flow-1',
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}

describe('NodeActionMenu', () => {
  test('exposes locate, copy and delete actions', async () => {
    const onDelete = vi.fn();

    render(
      <NodeActionMenu
        onLocate={vi.fn()}
        onCopy={vi.fn()}
        onDelete={onDelete}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));

    expect(await screen.findByRole('menuitem', { name: '定位节点' })).toBeInTheDocument();
    expect(screen.getByRole('menuitem', { name: '复制节点' })).toBeInTheDocument();

    fireEvent.click(screen.getByRole('menuitem', { name: '删除节点' }));

    expect(onDelete).toHaveBeenCalledTimes(1);
  });

  test('deletes the selected node from the editor store', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        const actions = useNodeDetailActions();

        return {
          ...actions,
          nodes: useAgentFlowEditorStore((state) => state.workingDocument.graph.nodes),
          selectedNodeId: useAgentFlowEditorStore((state) => state.selectedNodeId),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm']
      });
    });

    act(() => {
      result.current.deleteSelectedNode();
    });

    expect(result.current.selectedNodeId).toBe(null);
    expect(result.current.nodes).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm'
        })
      ])
    );
  });
});
