import type { ReactNode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { useEditorShortcuts } from '../../hooks/interactions/use-editor-shortcuts';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../store/editor/provider';

function createInitialState(
  document = createDefaultAgentFlowDocument({ flowId: 'flow-1' })
) {
  return {
    flow_id: 'flow-1',
    messages: [],
    draft: {
      id: 'draft-1',
      flow_id: 'flow-1',
      updated_at: '2026-04-16T10:00:00Z',
      document
    },
    autosave_interval_seconds: 30,
    user_protection_limit: 10,
    versions: []
  };
}

describe('useEditorShortcuts', () => {
  test('removes the selected edge when pressing Delete', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          edges: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.edges
          ),
          selectedEdgeId: useAgentFlowEditorStore(
            (state) => state.selectedEdgeId
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedEdgeId: 'edge-llm-answer',
        selectedNodeId: null,
        selectedNodeIds: []
      });
    });

    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete' }));
    });

    expect(result.current.selectedEdgeId).toBe(null);
    expect(result.current.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-llm-answer'
        })
      ])
    );
  });

  test('removes the selected node when pressing Delete', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          nodes: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.nodes
          ),
          edges: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.edges
          ),
          selectedNodeId: useAgentFlowEditorStore(
            (state) => state.selectedNodeId
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm'],
        selectedEdgeId: null
      });
    });

    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Delete' }));
    });

    expect(result.current.selectedNodeId).toBe(null);
    expect(result.current.nodes).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm'
        })
      ])
    );
    expect(result.current.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'edge-start-llm'
        }),
        expect.objectContaining({
          id: 'edge-llm-answer'
        })
      ])
    );
  });

  test('copies and pastes the selected node with keyboard shortcuts', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          nodes: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.nodes
          ),
          edges: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.edges
          ),
          selectedNodeId: useAgentFlowEditorStore(
            (state) => state.selectedNodeId
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm'],
        selectedEdgeId: null
      });
    });

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'c', ctrlKey: true })
      );
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'v', ctrlKey: true })
      );
    });

    expect(result.current.selectedNodeId).toBe('node-llm-copy');
    expect(result.current.nodes).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          id: 'node-llm-copy',
          alias: 'LLM 副本',
          position: { x: 408, y: 268 }
        })
      ])
    );
    expect(result.current.edges).not.toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          source: 'node-llm',
          target: 'node-llm-copy'
        })
      ])
    );
  });

  test('restores the previous document with Ctrl+Z', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          nodes: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.nodes
          ),
          selectedNodeId: useAgentFlowEditorStore(
            (state) => state.selectedNodeId
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm'],
        selectedEdgeId: null
      });
    });

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'c', ctrlKey: true })
      );
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'v', ctrlKey: true })
      );
    });

    expect(result.current.nodes).toEqual(
      expect.arrayContaining([expect.objectContaining({ id: 'node-llm-copy' })])
    );

    act(() => {
      window.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'z', ctrlKey: true })
      );
    });

    expect(result.current.selectedNodeId).toBe(null);
    expect(result.current.nodes).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: 'node-llm-copy' })])
    );
  });

  test('does not handle copy paste undo shortcuts from editable content', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          nodes: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.nodes
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    const input = document.createElement('input');
    document.body.appendChild(input);

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm'],
        selectedEdgeId: null
      });
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'c', ctrlKey: true, bubbles: true })
      );
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'v', ctrlKey: true, bubbles: true })
      );
      input.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'z', ctrlKey: true, bubbles: true })
      );
    });

    expect(result.current.nodes).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: 'node-llm-copy' })])
    );
    input.remove();
  });

  test('does not paste nodes when paste comes from Monaco editor content', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          nodes: useAgentFlowEditorStore(
            (state) => state.workingDocument.graph.nodes
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );
    const monacoEditor = document.createElement('div');
    const monacoLine = document.createElement('div');
    monacoEditor.className = 'monaco-editor';
    monacoLine.className = 'view-line';
    monacoEditor.appendChild(monacoLine);
    document.body.appendChild(monacoEditor);

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm'],
        selectedEdgeId: null
      });
    });

    act(() => {
      monacoLine.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'c', ctrlKey: true, bubbles: true })
      );
      monacoLine.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'v', ctrlKey: true, bubbles: true })
      );
    });

    expect(result.current.nodes).not.toEqual(
      expect.arrayContaining([expect.objectContaining({ id: 'node-llm-copy' })])
    );
    monacoEditor.remove();
  });

  test('does not close node detail when Escape comes from editable content', () => {
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        useEditorShortcuts();

        return {
          selectedNodeId: useAgentFlowEditorStore(
            (state) => state.selectedNodeId
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection)
        };
      },
      { wrapper }
    );

    const editable = document.createElement('div');
    editable.setAttribute('contenteditable', 'true');
    document.body.appendChild(editable);

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-llm',
        selectedNodeIds: ['node-llm'],
        selectedEdgeId: null
      });
    });

    act(() => {
      editable.dispatchEvent(
        new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })
      );
    });

    expect(result.current.selectedNodeId).toBe('node-llm');
    editable.remove();
  });
});
