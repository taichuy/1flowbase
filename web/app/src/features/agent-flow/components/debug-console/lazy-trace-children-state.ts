export function appendTraceChildrenPage<
  TNode extends { trace_node_id: string }
>(current: TNode[], nextPageItems: TNode[]) {
  if (current.length === 0) {
    return nextPageItems;
  }

  const seenTraceNodeIds = new Set(
    current.map((childNode) => childNode.trace_node_id)
  );
  const next = [...current];

  for (const childNode of nextPageItems) {
    if (!seenTraceNodeIds.has(childNode.trace_node_id)) {
      seenTraceNodeIds.add(childNode.trace_node_id);
      next.push(childNode);
    }
  }

  return next;
}
