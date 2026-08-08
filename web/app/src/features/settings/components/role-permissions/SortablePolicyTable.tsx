import {
  createContext,
  useContext,
  useMemo,
  type HTMLAttributes,
  type ReactNode
} from 'react';

import { DragOutlined } from '@ant-design/icons';
import {
  closestCenter,
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors
} from '@dnd-kit/core';
import { restrictToVerticalAxis } from '@dnd-kit/modifiers';
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Button } from 'antd';

import { i18nText } from '../../../../shared/i18n/text';

type SortableRowContextValue = Pick<
  ReturnType<typeof useSortable>,
  'attributes' | 'listeners' | 'setActivatorNodeRef'
>;

const SortableRowContext = createContext<SortableRowContextValue | null>(null);

export function SortablePolicyRow(
  props: HTMLAttributes<HTMLTableRowElement> & { 'data-row-key': string }
) {
  const sortable = useSortable({ id: String(props['data-row-key']) });
  const contextValue = useMemo(
    () => ({
      attributes: sortable.attributes,
      listeners: sortable.listeners,
      setActivatorNodeRef: sortable.setActivatorNodeRef
    }),
    [sortable.attributes, sortable.listeners, sortable.setActivatorNodeRef]
  );
  return (
    <SortableRowContext.Provider value={contextValue}>
      <tr
        {...props}
        ref={sortable.setNodeRef}
        style={{
          ...props.style,
          transform: CSS.Transform.toString(sortable.transform),
          transition: sortable.transition,
          ...(sortable.isDragging ? { position: 'relative', zIndex: 1 } : {})
        }}
      />
    </SortableRowContext.Provider>
  );
}

export function PolicyDragHandle() {
  const sortable = useContext(SortableRowContext);
  if (!sortable) return null;
  return (
    <Button
      ref={sortable.setActivatorNodeRef}
      type="text"
      size="small"
      aria-label={i18nText('settings', 'auto.drag_to_sort')}
      icon={<DragOutlined />}
      {...sortable.attributes}
      {...sortable.listeners}
    />
  );
}

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

export function SortablePolicyTable({
  itemIds,
  onReorder,
  children
}: {
  itemIds: string[];
  onReorder: (oldIndex: number, newIndex: number) => void;
  children: ReactNode;
}) {
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  return (
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={[restrictToVerticalAxis]}
      onDragEnd={(event) => {
        if (!event.over || event.active.id === event.over.id) return;
        const indices = findReorderIndices(
          itemIds,
          String(event.active.id),
          String(event.over.id)
        );
        if (indices) onReorder(...indices);
      }}
    >
      <SortableContext items={itemIds} strategy={verticalListSortingStrategy}>
        {children}
      </SortableContext>
    </DndContext>
  );
}
