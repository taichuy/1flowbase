export interface InteractionSlice {
  activeContainerPath: string[];
  connectingPayload: {
    sourceNodeId: string | null;
    sourceHandleId: string | null;
    sourceNodeType: string | null;
  };
  hoveredNodeId: string | null;
  hoveredEdgeId: string | null;
  highlightedIssueId: string | null;
  pendingLocateNodeId: string | null;
}
