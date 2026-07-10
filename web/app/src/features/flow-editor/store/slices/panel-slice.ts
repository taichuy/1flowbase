export interface PanelSlice {
  issuesOpen: boolean;
  historyOpen: boolean;
  nodeDetailWidth: number;
  nodePickerState: {
    open: boolean;
    anchorNodeId: string | null;
    anchorEdgeId: string | null;
    anchorCanvasPosition: { x: number; y: number } | null;
  };
}
