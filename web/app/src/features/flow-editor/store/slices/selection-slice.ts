export interface SelectionSlice {
  selectedNodeId: string | null;
  selectedEdgeId: string | null;
  selectedNodeIds: string[];
  selectionMode: 'single' | 'multiple';
  focusedFieldKey: string | null;
  openInspectorSectionKey: string | null;
}
