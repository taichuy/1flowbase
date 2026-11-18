import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';
import { describe, expect, test } from 'vitest';

import { createFlowEditorStore, type FlowEditorState } from '../store';
import type { SelectionSlice } from '../store/slices/selection-slice';

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

function selectionStore() {
  const document = createDefaultAgentFlowDocument({ flowId: 'selection-flow' });
  return createFlowEditorStore({
    flow_id: 'selection-flow', messages: [],
    draft: {
      id: 'selection-draft', flow_id: 'selection-flow',
      updated_at: '2026-11-17T00:00:00Z', document
    },
    analysis: { diagnostics: [], node_outputs: [], variable_catalogs: [] },
    autosave_interval_seconds: 30, user_protection_limit: 10, versions: []
  });
}

test('selection updates cannot publish structurally compatible business fields or arbitrary extra keys', () => {
  const store = selectionStore();
  const document = store.getState().workingDocument;
  store.getState().setWorkingDocument({
    ...document, meta: { ...document.meta, name: 'First selection fixture edit' }
  });
  store.getState().setWorkingDocument({
    ...document, meta: { ...document.meta, name: 'Second selection fixture edit' }
  });
  store.getState().restorePreviousDocument();
  store.getState().setPanelState({ issuesOpen: true, historyOpen: true });
  store.getState().setViewportState({ isFittingView: true });
  const before = store.getState();
  expect(before.undoStack).toHaveLength(1);
  expect(before.redoStack).toHaveLength(1);
  expect(before.isDirty).toBe(true);
  const selection: SelectionSlice = {
    selectedNodeId: 'node-answer', selectedNodeIds: ['node-answer', 'node-llm'],
    selectedEdgeId: null, selectionMode: 'multiple',
    focusedFieldKey: 'bindings.answer_template', openInspectorSectionKey: 'outputs'
  };
  const foreignDocument = createDefaultAgentFlowDocument({ flowId: 'foreign-flow' });
  const symbolExtra = Symbol('selection-payload-extra');
  // A variable with extra fields is legally assignable to Partial<SelectionSlice>.
  // The owner must enforce its partition at runtime, without relying on literal checks.
  const structuralPayload = {
    ...selection,
    workingDocument: foreignDocument,
    lastSavedDocument: foreignDocument,
    undoStack: [foreignDocument], redoStack: [foreignDocument],
    draftMeta: { draftId: 'foreign-draft', flowId: 'foreign-flow', updatedAt: 'foreign' },
    isDirty: false, issuesOpen: false, historyOpen: false, isFittingView: false,
    viewport: { x: 999, y: 999, zoom: 3 },
    arbitraryExtra: 'must not enter the store',
    [symbolExtra]: 'must not enter the store either'
  };
  const notifications: Array<{ next: FlowEditorState; previous: FlowEditorState }> = [];
  const unsubscribe = store.subscribe((next, previous) => {
    notifications.push({ next, previous });
  });
  try {
    before.setSelection(structuralPayload);
  } finally {
    unsubscribe();
  }

  const after = store.getState();
  expect(after).toMatchObject(selection);
  expect(after.selectedNodeIds).toBe(selection.selectedNodeIds);
  expect(notifications).toEqual([{ next: after, previous: before }]);
  const selectionKeys = new Set<keyof SelectionSlice>([
    'selectedNodeId', 'selectedEdgeId', 'selectedNodeIds',
    'selectionMode', 'focusedFieldKey', 'openInspectorSectionKey'
  ]);
  for (const key of Object.keys(before) as Array<keyof FlowEditorState>) {
    if (!selectionKeys.has(key as keyof SelectionSlice)) {
      expect(after[key], key).toBe(before[key]);
    }
  }
  expect(Reflect.ownKeys(after)).toEqual(Reflect.ownKeys(before));
  expect(after).not.toHaveProperty('arbitraryExtra');
  expect(Object.prototype.hasOwnProperty.call(after, symbolExtra)).toBe(false);
});

test('selection patches preserve omitted fields, explicit empty/null/undefined values and own-property semantics atomically', () => {
  const store = selectionStore();
  store.getState().setSelection({
    selectedNodeId: 'node-answer', selectedNodeIds: ['node-answer', 'node-llm'],
    selectedEdgeId: 'edge-selected', selectionMode: 'multiple',
    focusedFieldKey: 'bindings.answer_template', openInspectorSectionKey: 'outputs'
  });
  const before = store.getState();
  const notifications: FlowEditorState[] = [];
  const unsubscribe = store.subscribe((next) => notifications.push(next));
  try {
    const partial: Partial<SelectionSlice> = {
      selectedNodeId: null, selectedNodeIds: [], focusedFieldKey: ''
    };
    before.setSelection(partial);
    const cleared = store.getState();
    expect(cleared).toMatchObject({
      selectedNodeId: null, selectedNodeIds: [], focusedFieldKey: '',
      selectedEdgeId: 'edge-selected', selectionMode: 'multiple',
      openInspectorSectionKey: 'outputs'
    });
    expect(cleared.selectedNodeIds).toBe(partial.selectedNodeIds);
    expect(notifications).toEqual([cleared]);

    const suppliedUndefined: Partial<SelectionSlice> = { focusedFieldKey: undefined };
    cleared.setSelection(suppliedUndefined);
    const undefinedField = store.getState();
    expect(undefinedField.focusedFieldKey).toBeUndefined();
    expect(undefinedField.openInspectorSectionKey).toBe('outputs');
    expect(notifications).toEqual([cleared, undefinedField]);

    class SelectionWithInheritedNode {
      selectionMode: SelectionSlice['selectionMode'] = 'single';
      get selectedNodeId() { return 'must-not-read-inherited-selection'; }
    }
    undefinedField.setSelection(new SelectionWithInheritedNode());
    const ownFieldsOnly = store.getState();
    expect(ownFieldsOnly.selectedNodeId).toBeNull();
    expect(ownFieldsOnly.selectionMode).toBe('single');
    expect(ownFieldsOnly.focusedFieldKey).toBeUndefined();
    expect(ownFieldsOnly.selectedNodeIds).toBe(partial.selectedNodeIds);
    expect(notifications).toEqual([cleared, undefinedField, ownFieldsOnly]);

    ownFieldsOnly.setSelection({});
    const emptyPatch = store.getState();
    expect(emptyPatch).toEqual(ownFieldsOnly);
    expect(notifications).toEqual([cleared, undefinedField, ownFieldsOnly, emptyPatch]);
    expect(emptyPatch.workingDocument).toBe(before.workingDocument);
    expect(emptyPatch.lastSavedDocument).toBe(before.lastSavedDocument);
    expect(emptyPatch.undoStack).toBe(before.undoStack);
    expect(emptyPatch.redoStack).toBe(before.redoStack);
    expect(emptyPatch.isDirty).toBe(before.isDirty);
  } finally {
    unsubscribe();
  }
});
