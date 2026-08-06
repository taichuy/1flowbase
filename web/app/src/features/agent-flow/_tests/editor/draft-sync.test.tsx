import type { ReactNode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { useDraftSync } from '../../hooks/interactions/use-draft-sync';
import { createNodeDocument } from '../../lib/document/node-factory';
import { AgentFlowEditorStoreProvider } from '../../store/editor/AgentFlowEditorStoreProvider';
import { useAgentFlowEditorStore } from '../../store/editor/provider';
import { resetAuthStore, useAuthStore } from '../../../../state/auth-store';

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

function createVariableAggregatorDocument(valueType: 'string' | 'number') {
  const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
  const aggregator = createNodeDocument(
    'variable_aggregator',
    'node-variable-aggregator'
  );

  aggregator.bindings.groups = {
    kind: 'variable_groups',
    value: [
      {
        key: 'group1',
        valueType,
        candidates: [['node-llm', 'text']]
      }
    ]
  };
  aggregator.outputs = [{ key: 'group1', title: 'group1', valueType }];
  document.graph.nodes.push(aggregator);
  document.graph.edges.push({
    id: 'edge-llm-variable-aggregator',
    source: 'node-llm',
    target: aggregator.id,
    sourceHandle: null,
    targetHandle: null,
    containerId: null,
    points: []
  });

  return document;
}

beforeEach(() => {
  resetAuthStore();
  useAuthStore.setState({
    sessionStatus: 'authenticated',
    csrfToken: 'csrf-token'
  });
});

afterEach(() => {
  vi.useRealTimers();
});

describe('useDraftSync', () => {
  test('AC-011 blocks manual save for an incompatible Variable Aggregator candidate', async () => {
    const currentDocument = createVariableAggregatorDocument('number');
    const saveDraftOverride = vi.fn(async (input) =>
      createInitialState(input.document)
    );
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );
    const { result } = renderHook(
      () =>
        useDraftSync({
          applicationId: 'app-1',
          saveDraftOverride,
          getCurrentDocument: () => currentDocument
        }),
      { wrapper }
    );

    await act(async () => {
      expect(await result.current.saveNow()).toBe(false);
    });

    expect(saveDraftOverride).not.toHaveBeenCalled();
  });

  test('AC-011 blocks autosave for an incompatible Variable Aggregator candidate', async () => {
    vi.useFakeTimers();
    const currentDocument = createVariableAggregatorDocument('number');
    const saveDraftOverride = vi.fn(async (input) =>
      createInitialState(input.document)
    );
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );
    const { result } = renderHook(
      () => ({
        draftSync: useDraftSync({
          applicationId: 'app-1',
          saveDraftOverride
        }),
        setWorkingDocument: useAgentFlowEditorStore(
          (state) => state.setWorkingDocument
        )
      }),
      { wrapper }
    );

    act(() => {
      result.current.setWorkingDocument(currentDocument);
    });
    await act(async () => {
      await vi.advanceTimersByTimeAsync(30_000);
    });

    expect(saveDraftOverride).not.toHaveBeenCalled();
  });

  test('AC-011 keeps a compatible Variable Aggregator group saveable', async () => {
    const currentDocument = createVariableAggregatorDocument('string');
    const saveDraftOverride = vi.fn(async (input) => ({
      ...createInitialState(input.document),
      draft: {
        ...createInitialState(input.document).draft,
        document: input.document
      }
    }));
    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );
    const { result } = renderHook(
      () =>
        useDraftSync({
          applicationId: 'app-1',
          saveDraftOverride,
          getCurrentDocument: () => currentDocument
        }),
      { wrapper }
    );

    await act(async () => {
      expect(await result.current.saveNow()).toBe(true);
    });

    expect(saveDraftOverride).toHaveBeenCalledOnce();
  });

  test('restores server draft and clears transient editor state', async () => {
    const restoreVersionOverride = vi.fn().mockResolvedValue({
      ...createInitialState({
        ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }),
        meta: {
          ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }).meta,
          name: 'Recovered flow'
        }
      }),
      draft: {
        ...createInitialState().draft,
        updated_at: '2026-04-16T10:20:00Z',
        document: {
          ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }),
          meta: {
            ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }).meta,
            name: 'Recovered flow'
          }
        }
      }
    });

    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        const draftSync = useDraftSync({
          applicationId: 'app-1',
          restoreVersionOverride
        });

        return {
          draftSync,
          selectedNodeId: useAgentFlowEditorStore(
            (state) => state.selectedNodeId
          ),
          activeContainerPath: useAgentFlowEditorStore(
            (state) => state.activeContainerPath
          ),
          nodePickerOpen: useAgentFlowEditorStore(
            (state) => state.nodePickerState.open
          ),
          focusedFieldKey: useAgentFlowEditorStore(
            (state) => state.focusedFieldKey
          ),
          workingDocument: useAgentFlowEditorStore(
            (state) => state.workingDocument
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection),
          setPanelState: useAgentFlowEditorStore(
            (state) => state.setPanelState
          ),
          setInteractionState: useAgentFlowEditorStore(
            (state) => state.setInteractionState
          )
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-answer',
        selectedNodeIds: ['node-answer'],
        focusedFieldKey: 'bindings.answer_template'
      });
      result.current.setPanelState({
        historyOpen: true,
        nodePickerState: {
          open: true,
          anchorNodeId: 'node-llm',
          anchorEdgeId: null,
          anchorCanvasPosition: null
        }
      });
      result.current.setInteractionState({
        activeContainerPath: ['node-iteration-1']
      });
    });

    await act(async () => {
      await result.current.draftSync.restoreVersion('version-5');
    });

    expect(restoreVersionOverride).toHaveBeenCalledWith('version-5');
    expect(result.current.draftSync.status).toBe('idle');
    expect(result.current.workingDocument.meta.name).toBe('Recovered flow');
    expect(result.current.activeContainerPath).toEqual([]);
    expect(result.current.nodePickerOpen).toBe(false);
    expect(result.current.focusedFieldKey).toBe(null);
    expect(result.current.selectedNodeId).toBe(null);
  });

  test('manual save reads the latest viewport from the provided getter', async () => {
    const latestDocumentRef = {
      current: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
    };
    const saveDraftOverride = vi.fn(async (input) => ({
      ...createInitialState(input.document),
      draft: {
        ...createInitialState(input.document).draft,
        id: 'draft-2',
        updated_at: '2026-04-16T10:10:00Z',
        document: input.document
      }
    }));

    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => ({
        draftSync: useDraftSync({
          applicationId: 'app-1',
          saveDraftOverride,
          getCurrentDocument: () => latestDocumentRef.current
        }),
        workingDocument: useAgentFlowEditorStore(
          (state) => state.workingDocument
        )
      }),
      { wrapper }
    );

    latestDocumentRef.current = {
      ...latestDocumentRef.current,
      editor: {
        ...latestDocumentRef.current.editor,
        viewport: { x: 120, y: 48, zoom: 0.85 }
      }
    };

    await act(async () => {
      await result.current.draftSync.saveNow();
    });

    expect(saveDraftOverride).toHaveBeenCalledWith(
      expect.objectContaining({
        change_kind: 'layout',
        document: expect.objectContaining({
          editor: expect.objectContaining({
            viewport: { x: 120, y: 48, zoom: 0.85 }
          })
        })
      })
    );
    expect(result.current.workingDocument.editor.viewport).toEqual({
      x: 120,
      y: 48,
      zoom: 0.85
    });
  });

  test('save sync keeps the current editor surface unchanged', async () => {
    const saveDraftOverride = vi.fn(async (input) => ({
      ...createInitialState(input.document),
      draft: {
        ...createInitialState(input.document).draft,
        id: 'draft-2',
        updated_at: '2026-04-16T10:10:00Z',
        document: input.document
      }
    }));

    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => {
        const draftSync = useDraftSync({
          applicationId: 'app-1',
          saveDraftOverride
        });

        return {
          draftSync,
          selectedNodeId: useAgentFlowEditorStore(
            (state) => state.selectedNodeId
          ),
          focusedFieldKey: useAgentFlowEditorStore(
            (state) => state.focusedFieldKey
          ),
          historyOpen: useAgentFlowEditorStore((state) => state.historyOpen),
          activeContainerPath: useAgentFlowEditorStore(
            (state) => state.activeContainerPath
          ),
          setSelection: useAgentFlowEditorStore((state) => state.setSelection),
          setPanelState: useAgentFlowEditorStore(
            (state) => state.setPanelState
          ),
          setInteractionState: useAgentFlowEditorStore(
            (state) => state.setInteractionState
          )
        };
      },
      { wrapper }
    );

    act(() => {
      result.current.setSelection({
        selectedNodeId: 'node-answer',
        selectedNodeIds: ['node-answer'],
        focusedFieldKey: 'bindings.answer_template'
      });
      result.current.setPanelState({
        historyOpen: true
      });
      result.current.setInteractionState({
        activeContainerPath: ['node-iteration-1']
      });
    });

    await act(async () => {
      await result.current.draftSync.saveNow();
    });

    expect(result.current.selectedNodeId).toBe('node-answer');
    expect(result.current.focusedFieldKey).toBe('bindings.answer_template');
    expect(result.current.historyOpen).toBe(true);
    expect(result.current.activeContainerPath).toEqual(['node-iteration-1']);
  });

  test('autosaves layout changes on interval and preserves layout classification', async () => {
    vi.useFakeTimers();
    const layoutChangedDocument = {
      ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }),
      editor: {
        ...createDefaultAgentFlowDocument({ flowId: 'flow-1' }).editor,
        viewport: { x: 120, y: 48, zoom: 0.85 }
      }
    };
    const saveDraftOverride = vi.fn(async (input) => ({
      ...createInitialState(input.document),
      draft: {
        ...createInitialState(input.document).draft,
        updated_at: '2026-04-16T10:30:00Z',
        document: input.document
      }
    }));

    const wrapper = ({ children }: { children: ReactNode }) => (
      <AgentFlowEditorStoreProvider initialState={createInitialState()}>
        {children}
      </AgentFlowEditorStoreProvider>
    );

    const { result } = renderHook(
      () => ({
        draftSync: useDraftSync({
          applicationId: 'app-1',
          saveDraftOverride
        }),
        setWorkingDocument: useAgentFlowEditorStore(
          (state) => state.setWorkingDocument
        )
      }),
      { wrapper }
    );

    act(() => {
      result.current.setWorkingDocument(layoutChangedDocument);
    });

    await act(async () => {
      await vi.advanceTimersByTimeAsync(30_000);
    });

    expect(saveDraftOverride).toHaveBeenCalledWith(
      expect.objectContaining({
        change_kind: 'layout',
        document: expect.objectContaining({
          editor: expect.objectContaining({
            viewport: { x: 120, y: 48, zoom: 0.85 }
          })
        })
      })
    );
    expect(['idle', 'saved']).toContain(result.current.draftSync.status);
  });
});
