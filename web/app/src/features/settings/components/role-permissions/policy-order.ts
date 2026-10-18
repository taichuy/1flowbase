import { arrayMove } from '@dnd-kit/sortable';

export function reorderItems<T>(
  items: T[],
  oldIndex: number,
  newIndex: number
) {
  if (
    oldIndex < 0 ||
    newIndex < 0 ||
    oldIndex >= items.length ||
    newIndex >= items.length
  ) {
    return items;
  }
  return arrayMove(items, oldIndex, newIndex);
}

export function findReorderIndices(
  itemIds: string[],
  activeId: string,
  overId: string
): [number, number] | null {
  const oldIndex = itemIds.indexOf(activeId);
  const newIndex = itemIds.indexOf(overId);
  return oldIndex >= 0 && newIndex >= 0 ? [oldIndex, newIndex] : null;
}
