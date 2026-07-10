import { describe, expect, test } from 'vitest';
import { createDefaultAgentFlowDocument } from '@1flowbase/flow-schema';

import { NODE_DETAIL_DEFAULT_WIDTH } from '../../lib/detail-panel-width';
import { createAgentFlowEditorStore } from '../../store/editor';

describe('agent flow editor store', () => {
  test('seeds working document, selection, panel state and sync state from server data', () => {
    const initialDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const store = createAgentFlowEditorStore({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:00:00Z',
        document: initialDocument
      },
      autosave_interval_seconds: 30,
      versions: []
    });

    expect(store.getState().workingDocument.meta.flowId).toBe('flow-1');
    expect(store.getState().selectedNodeId).toBe(null);
    expect(store.getState().selectedNodeIds).toEqual([]);
    expect(store.getState().issuesOpen).toBe(false);
    expect(store.getState().autosaveStatus).toBe('idle');
  });

  test('replaces server state and clears scratch interaction state after restore', () => {
    const initialDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const restoredDocument = {
      ...initialDocument,
      meta: {
        ...initialDocument.meta,
        name: 'Restored flow'
      }
    };
    const store = createAgentFlowEditorStore({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:00:00Z',
        document: initialDocument
      },
      autosave_interval_seconds: 30,
      versions: []
    });

    store.getState().setPanelState({
      issuesOpen: true,
      historyOpen: true,
      nodePickerState: {
        open: true,
        anchorNodeId: 'node-llm',
        anchorEdgeId: null,
        anchorCanvasPosition: null
      }
    });
    store.getState().focusIssueField({
      nodeId: 'node-answer',
      sectionKey: 'outputs',
      fieldKey: 'bindings.answer_template'
    });

    store.getState().replaceFromServerState({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:05:00Z',
        document: restoredDocument
      },
      autosave_interval_seconds: 30,
      versions: []
    });

    expect(store.getState().workingDocument.meta.name).toBe('Restored flow');
    expect(store.getState().issuesOpen).toBe(false);
    expect(store.getState().historyOpen).toBe(false);
    expect(store.getState().nodePickerState.open).toBe(false);
    expect(store.getState().focusedFieldKey).toBe(null);
    expect(store.getState().highlightedIssueId).toBe(null);
  });

  test('syncs saved server state without changing current editor surface', () => {
    const initialDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const savedDocument = {
      ...initialDocument,
      meta: {
        ...initialDocument.meta,
        name: 'Saved flow'
      },
      editor: {
        ...initialDocument.editor,
        viewport: { x: 240, y: 120, zoom: 0.72 }
      }
    };
    const store = createAgentFlowEditorStore({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:00:00Z',
        document: initialDocument
      },
      autosave_interval_seconds: 30,
      versions: []
    });

    store.getState().setSelection({
      selectedNodeId: 'node-answer',
      selectedNodeIds: ['node-answer'],
      focusedFieldKey: 'bindings.answer_template',
      openInspectorSectionKey: 'outputs'
    });
    store.getState().setPanelState({ historyOpen: true });
    store.getState().setInteractionState({
      activeContainerPath: ['node-iteration-1'],
      highlightedIssueId: 'issue-1'
    });
    store.getState().setViewportState({
      viewport: { x: 12, y: 24, zoom: 1.1 }
    });

    store.getState().syncSavedServerState({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-2',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:05:00Z',
        document: savedDocument
      },
      autosave_interval_seconds: 45,
      versions: []
    });

    expect(store.getState().workingDocument).toBe(initialDocument);
    expect(store.getState().lastSavedDocument).toBe(savedDocument);
    expect(store.getState().draftMeta).toEqual({
      draftId: 'draft-2',
      flowId: 'flow-1',
      updatedAt: '2026-04-16T10:05:00Z'
    });
    expect(store.getState().autosaveIntervalMs).toBe(45_000);
    expect(store.getState().selectedNodeId).toBe('node-answer');
    expect(store.getState().focusedFieldKey).toBe('bindings.answer_template');
    expect(store.getState().openInspectorSectionKey).toBe('outputs');
    expect(store.getState().historyOpen).toBe(true);
    expect(store.getState().activeContainerPath).toEqual(['node-iteration-1']);
    expect(store.getState().highlightedIssueId).toBe('issue-1');
    expect(store.getState().viewport).toEqual({ x: 12, y: 24, zoom: 1.1 });
  });

  test('keeps application-specific panel state outside the shared store', () => {
    const store = createAgentFlowEditorStore({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:00:00Z',
        document: createDefaultAgentFlowDocument({ flowId: 'flow-1' })
      },
      autosave_interval_seconds: 30,
      versions: []
    });
    const state = store.getState() as unknown as Record<string, unknown>;

    expect(state.nodeDetailWidth).toBe(NODE_DETAIL_DEFAULT_WIDTH);
    expect(state).not.toHaveProperty('debugConsoleOpen');
    expect(state).not.toHaveProperty('debugConsoleWidth');
    expect(state).not.toHaveProperty('nodeDetailTab');
  });

  test('keeps node detail width when replacing from server state', () => {
    const initialDocument = createDefaultAgentFlowDocument({ flowId: 'flow-1' });
    const store = createAgentFlowEditorStore({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-1',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:00:00Z',
        document: initialDocument
      },
      autosave_interval_seconds: 30,
      versions: []
    });

    store.getState().setPanelState({ nodeDetailWidth: 560 });
    store.getState().replaceFromServerState({
      flow_id: 'flow-1',
      draft: {
        id: 'draft-2',
        flow_id: 'flow-1',
        updated_at: '2026-04-16T10:05:00Z',
        document: {
          ...initialDocument,
          meta: { ...initialDocument.meta, name: 'Server synced' }
        }
      },
      autosave_interval_seconds: 30,
      versions: []
    });

    expect(store.getState().workingDocument.meta.name).toBe('Server synced');
    expect(store.getState().nodeDetailWidth).toBe(560);
  });

});
