import type { EventCatalogEntry, EventSourceSelection } from './types';

/** Canonical catalog leaf identity, including backend-declared subtype filters. */
export function eventCatalogIdentity(entry: {
  contract_id: string;
  id?: EventCatalogEntry['id'];
  source?: { selection: EventSourceSelection };
}) {
  const selection = entry.source?.selection;
  return JSON.stringify([
    entry.contract_id,
    entry.id ?? null,
    selection?.source_id ?? null,
    selection?.scope.kind ?? null,
    selection?.scope.kind === 'workspace' ? selection.scope.workspace_id : null,
    Object.entries(selection?.filters ?? {}).sort(([left], [right]) =>
      left.localeCompare(right)
    )
  ]);
}
