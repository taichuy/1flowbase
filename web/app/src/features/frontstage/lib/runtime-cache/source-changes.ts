export interface FrontstageSourceChange {
  workspaceId: string;
  pageId: string;
  blockId: string;
}
const listeners = new Set<(change: FrontstageSourceChange) => void>();

export function notifyFrontstageSourceChange(
  change: FrontstageSourceChange
): void {
  for (const listener of listeners) listener(change);
}

export function subscribeFrontstageSourceChanges(
  listener: (change: FrontstageSourceChange) => void
): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
