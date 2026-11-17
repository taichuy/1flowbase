import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { describe, expect, test } from 'vitest';

import { createFlowEditorStore, type FlowEditorState } from '../store';

describe('flow editor store', () => {
  test('AC-002 owns only application-neutral editor state', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const store = createFlowEditorStore({
      flow_id: 'flow-1',
      messages: [],
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-07-10T10:00:00Z',
        document
      },
      analysis: { diagnostics: [], node_outputs: [], variable_catalogs: [] },
      autosave_interval_seconds: 30,
      user_protection_limit: 10,
      versions: []
    });
    const state = store.getState() as unknown as Record<string, unknown>;

    expect(state.workingDocument).toBe(document);
    expect(state.issuesOpen).toBe(false);
    expect(state.historyOpen).toBe(false);
    expect(state).not.toHaveProperty('debugConsoleOpen');
    expect(state).not.toHaveProperty('debugConsoleWidth');
    expect(state).not.toHaveProperty('publishConfigOpen');
    expect(state).not.toHaveProperty('nodeDetailTab');
  });

  test('clears only run-entry transients in one notification and retains document history', () => {
    const document = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const store = createFlowEditorStore({
      flow_id: 'flow-1',
      messages: [],
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-07-10T10:00:00Z',
        document
      },
      analysis: { diagnostics: [], node_outputs: [], variable_catalogs: [] },
      autosave_interval_seconds: 30,
      user_protection_limit: 10,
      versions: []
    });
    store.getState().setWorkingDocument({
      ...document,
      meta: { ...document.meta, name: 'First edit' }
    });
    store.getState().setWorkingDocument({
      ...document,
      meta: { ...document.meta, name: 'Second edit' }
    });
    store.getState().restorePreviousDocument();
    store.getState().setSelection({
      selectedNodeId: 'node-answer',
      selectedNodeIds: ['node-answer', 'node-llm'],
      selectedEdgeId: 'edge-selected',
      selectionMode: 'multiple',
      focusedFieldKey: 'bindings.answer_template',
      openInspectorSectionKey: 'outputs'
    });
    store.getState().setPanelState({
      historyOpen: true,
      issuesOpen: true,
      nodeDetailWidth: 540,
      nodePickerState: {
        open: true,
        anchorNodeId: 'node-llm',
        anchorEdgeId: 'edge-selected',
        anchorCanvasPosition: { x: 12, y: 24 }
      }
    });
    store.getState().setInteractionState({
      activeContainerPath: ['container-1'],
      pendingLocateNodeId: 'node-answer',
      connectingPayload: {
        sourceNodeId: 'node-llm',
        sourceHandleId: 'output',
        sourceNodeType: 'llm'
      },
      hoveredNodeId: 'node-answer',
      hoveredEdgeId: 'edge-hovered',
      highlightedIssueId: 'issue-1'
    });
    store.getState().setViewportState({
      viewport: { x: 20, y: 30, zoom: 0.8 },
      controlMode: 'hand',
      isFittingView: true
    });
    store.getState().setSyncState({
      autosaveStatus: 'saving',
      isRestoringVersion: true,
      isDirty: true,
      lastChangeKind: 'logical',
      lastChangeSummary: 'Keep this edit'
    });
    const before = store.getState();
    expect(before.undoStack).toHaveLength(1);
    expect(before.redoStack).toHaveLength(1);
    const notifications: FlowEditorState[] = [];
    const unsubscribe = store.subscribe((next) => notifications.push(next));
    try {
      before.clearRunEntryTransientState();
    } finally {
      unsubscribe();
    }
    const after = store.getState();
    expect(notifications).toEqual([after]);
    expect(after).toMatchObject({
      selectedNodeId: null,
      selectedNodeIds: [],
      selectedEdgeId: null,
      historyOpen: false,
      issuesOpen: false,
      nodePickerState: {
        open: false,
        anchorNodeId: null,
        anchorEdgeId: null,
        anchorCanvasPosition: null
      },
      pendingLocateNodeId: null,
      connectingPayload: {
        sourceNodeId: null,
        sourceHandleId: null,
        sourceNodeType: null
      }
    });
    const clearedFields = new Set<keyof FlowEditorState>([
      'selectedNodeId',
      'selectedNodeIds',
      'selectedEdgeId',
      'historyOpen',
      'issuesOpen',
      'nodePickerState',
      'pendingLocateNodeId',
      'connectingPayload'
    ]);
    for (const key of Object.keys(before) as Array<keyof FlowEditorState>) {
      if (!clearedFields.has(key)) expect(after[key], key).toBe(before[key]);
    }
    expect(after.workingDocument.meta.name).toBe('First edit');
    expect(after.undoStack).toBe(before.undoStack);
    expect(after.redoStack).toBe(before.redoStack);
  });
});
