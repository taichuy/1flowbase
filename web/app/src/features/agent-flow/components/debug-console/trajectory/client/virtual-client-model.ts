import type { ClientTrajectoryIndexItem } from '@1flowbase/api-client';

export function indexLane(step: Pick<ClientTrajectoryIndexItem, 'category'>) {
  return step.category.startsWith('tool')
    ? 'tool'
    : ['request', 'user', 'system'].includes(step.category)
      ? 'input'
      : 'model';
}

export const clientRowKey = (row: { id: string }) => row.id;
export const virtualClientKey = (slot: { key: string }) => slot.key;
export const virtualClientEventKey = (slot: {
  kind: string;
  step?: { id: string };
  root?: { id: string };
}) =>
  slot.kind === 'step'
    ? slot.step?.id
    : slot.kind === 'request'
      ? slot.root?.id
      : undefined;
