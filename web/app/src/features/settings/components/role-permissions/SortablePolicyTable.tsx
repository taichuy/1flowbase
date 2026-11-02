import {
  createContext,
  useContext,
  useMemo,
  type HTMLAttributes,
  type ReactNode
} from 'react';

import DragOutlined from '@ant-design/icons/es/icons/DragOutlined';
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
  SortableContext,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { Button } from 'antd';

import { findReorderIndices } from './policy-order';

import { useConsoleOperationAccess } from '../../../../state/console-operation-access';
import { i18nText } from '../../../../shared/i18n/text';

type SortableRowContextValue = Pick<
  ReturnType<typeof useSortable>,
  'attributes' | 'listeners' | 'setActivatorNodeRef'
>;

const PolicyReorderContext = createContext(false);

const SortableRowContext = createContext<SortableRowContextValue | null>(null);

export function SortablePolicyRow(
  props: HTMLAttributes<HTMLTableRowElement> & { 'data-row-key': string }
) {
  const canReorder = useContext(PolicyReorderContext);
  const sortable = useSortable({ id: String(props['data-row-key']), disabled: !canReorder });
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
  const canReorder = useContext(PolicyReorderContext);
  if (!sortable) return null;
  return (
    <Button
      ref={sortable.setActivatorNodeRef}
      type="text"
      size="small"
      aria-label={i18nText('settings', 'auto.drag_to_sort')}
      icon={<DragOutlined />}
      {...sortable.attributes}
      {...(canReorder ? sortable.listeners : {})}
      disabled={!canReorder}
    />
  );
}

export function SortablePolicyTable({
  itemIds,
  disabled = false,
  onReorder,
  children
}: {
  itemIds: string[];
  disabled?: boolean;
  onReorder: (oldIndex: number, newIndex: number) => void;
  children: ReactNode;
}) {
  const can = useConsoleOperationAccess();
  const canReorder = !disabled && can('roles.console_settings_order.replace');
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
  return (
    <PolicyReorderContext.Provider value={canReorder}>
    <DndContext
      sensors={sensors}
      collisionDetection={closestCenter}
      modifiers={[restrictToVerticalAxis]}
      onDragEnd={(event) => {
        if (!canReorder || !event.over || event.active.id === event.over.id) return;
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
    </PolicyReorderContext.Provider>
  );
}
