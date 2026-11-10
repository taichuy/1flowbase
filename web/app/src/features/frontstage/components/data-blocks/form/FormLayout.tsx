import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import { BlockDesignButton } from '../../../lib/native-modules/block-design/runtime';
import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode
} from 'react';
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
  type DragEndEvent
} from '@dnd-kit/core';
import { CSS } from '@dnd-kit/utilities';
import { Dropdown } from 'antd';
import { HolderOutlined } from '@ant-design/icons';
import { i18nText } from '../../../../../shared/i18n/text';
import type { DataFormItem } from '../../../lib/data-blocks/types';
import { moveFormItem } from './form-state';

type Placement = 'before' | 'after' | 'beside';
type Props = {
  items: DataFormItem[];
  design: boolean;
  saving: boolean;
  renderItem(item: DataFormItem): ReactNode;
  onItemsChange(items: DataFormItem[]): void;
  onSettings(item: DataFormItem): void;
};

export function FormLayout({
  items,
  design,
  saving,
  renderItem,
  onItemsChange,
  onSettings
}: Props) {
  const [dragging, setDragging] = useState(false);
  const reportDragActive = useDataBlockHost()?.setDragActive;
  const releaseTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const clearReleaseTimer = useCallback(() => {
    if (releaseTimer.current !== null) {
      clearTimeout(releaseTimer.current);
      releaseTimer.current = null;
    }
  }, []);
  const changeDragActivity = useCallback(
    (active: boolean) => {
      clearReleaseTimer();
      setDragging(active);
      reportDragActive?.(active);
    },
    [clearReleaseTimer, reportDragActive]
  );
  const cancelDrag = useCallback(() => {
    setDragging(false);
    clearReleaseTimer();
    // DndKit cancels on document keydown before the container's window Escape
    // handler. Keep its keyboard lock until this native event finishes dispatching.
    releaseTimer.current = setTimeout(() => {
      releaseTimer.current = null;
      reportDragActive?.(false);
    }, 0);
  }, [clearReleaseTimer, reportDragActive]);
  useEffect(
    () => () => {
      clearReleaseTimer();
      reportDragActive?.(false);
    },
    [clearReleaseTimer, reportDragActive]
  );
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor)
  );
  const rows = new Map<string, DataFormItem[]>();
  for (const item of items)
    rows.set(item.row, [...(rows.get(item.row) ?? []), item]);
  const drop = ({ active, over }: DragEndEvent) => {
    changeDragActivity(false);
    if (!over || saving) return;
    const target = over.data.current as
      | { itemId?: string; placement?: Placement }
      | undefined;
    if (target?.itemId && target.placement)
      onItemsChange(
        moveFormItem(items, String(active.id), target.itemId, target.placement)
      );
  };
  return (
    <DndContext
      sensors={sensors}
      onDragStart={() => changeDragActivity(true)}
      onDragCancel={cancelDrag}
      onDragEnd={drop}
    >
      <div className={`data-form-grid${dragging ? ' is-dragging' : ''}`}>
        {[...rows].map(([row, rowItems]) => (
          <div key={row} className="data-form-row">
            {rowItems.map((item) => (
              <FormLayoutItem
                key={item.id}
                item={item}
                design={design}
                saving={saving}
                dragging={dragging}
                onSettings={() => onSettings(item)}
                onResize={(span) =>
                  onItemsChange(
                    items.map((candidate) =>
                      candidate.id === item.id
                        ? { ...candidate, span }
                        : candidate
                    )
                  )
                }
                onRemove={() =>
                  onItemsChange(
                    items.filter((candidate) => candidate.id !== item.id)
                  )
                }
              >
                {renderItem(item)}
              </FormLayoutItem>
            ))}
          </div>
        ))}
      </div>
    </DndContext>
  );
}

function DropZone({
  itemId,
  placement,
  enabled
}: {
  itemId: string;
  placement: Placement;
  enabled: boolean;
}) {
  const { setNodeRef, isOver } = useDroppable({
    id: `${itemId}:${placement}`,
    data: { itemId, placement },
    disabled: !enabled
  });
  return (
    <div
      ref={setNodeRef}
      className={`data-form-drop data-form-drop-${placement}${isOver ? ' is-over' : ''}`}
      aria-hidden="true"
    />
  );
}

function FormLayoutItem({
  item,
  design,
  saving,
  dragging,
  onSettings,
  onResize,
  onRemove,
  children
}: {
  item: DataFormItem;
  design: boolean;
  saving: boolean;
  dragging: boolean;
  onSettings(): void;
  onResize(span: number): void;
  onRemove(): void;
  children: ReactNode;
}) {
  const popupContainer = useDataBlockPopupContainer();
  const { attributes, listeners, setNodeRef, transform, isDragging } =
    useDraggable({ id: item.id, disabled: !design || saving });
  const [resizeSpan, setResizeSpan] = useState<number | null>(null);
  const style = {
    '--data-form-span': resizeSpan ?? Math.min(24, Math.max(1, item.span)),
    transform: CSS.Translate.toString(transform),
    opacity: isDragging ? 0.45 : undefined
  } as CSSProperties;
  return (
    <div
      ref={setNodeRef}
      style={style}
      className={`data-form-layout-item${design ? ' is-design' : ''}`}
    >
      {design && (
        <>
          <DropZone
            itemId={item.id}
            placement="before"
            enabled={dragging && !isDragging}
          />
          <DropZone
            itemId={item.id}
            placement="after"
            enabled={dragging && !isDragging}
          />
          <DropZone
            itemId={item.id}
            placement="beside"
            enabled={dragging && !isDragging}
          />
          <div className="data-form-field-tools">
            <BlockDesignButton
              icon={<HolderOutlined />}
              {...attributes}
              {...listeners}
              disabled={saving}
              aria-label={i18nText('frontstage', 'data_blocks.form.move_field')}
            />
            <Dropdown
              trigger={['click']}
              getPopupContainer={popupContainer}
              menu={{
                items: [
                  {
                    key: 'settings',
                    label: i18nText(
                      'frontstage',
                      'data_blocks.form.field_settings'
                    ),
                    onClick: onSettings
                  },
                  {
                    key: 'remove',
                    danger: true,
                    label: i18nText(
                      'frontstage',
                      'data_blocks.form.remove_field'
                    ),
                    onClick: onRemove
                  }
                ]
              }}
            >
              <BlockDesignButton
                disabled={saving}
                aria-label={i18nText(
                  'frontstage',
                  'data_blocks.form.field_settings'
                )}
              />
            </Dropdown>
          </div>
        </>
      )}
      {children}
      {design && (
        <button
          type="button"
          className="data-form-resize"
          disabled={saving || dragging}
          aria-label={i18nText('frontstage', 'data_blocks.form.column_width')}
          role="slider"
          aria-valuemin={1}
          aria-valuemax={24}
          aria-valuenow={resizeSpan ?? item.span}
          onPointerDown={(event) => {
            event.currentTarget.setPointerCapture(event.pointerId);
            setResizeSpan(item.span);
          }}
          onPointerMove={(event) => {
            if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
            const itemRect =
              event.currentTarget.parentElement?.getBoundingClientRect();
            const rowRect =
              event.currentTarget.parentElement?.parentElement?.getBoundingClientRect();
            if (itemRect && rowRect)
              setResizeSpan(
                Math.max(
                  1,
                  Math.min(
                    24,
                    Math.round(
                      ((event.clientX - itemRect.left) / rowRect.width) * 24
                    )
                  )
                )
              );
          }}
          onPointerUp={(event) => {
            event.currentTarget.releasePointerCapture(event.pointerId);
            if (resizeSpan !== null && resizeSpan !== item.span)
              onResize(resizeSpan);
            setResizeSpan(null);
          }}
          onPointerCancel={() => setResizeSpan(null)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowLeft' || event.key === 'ArrowRight') {
              event.preventDefault();
              onResize(
                Math.max(
                  1,
                  Math.min(
                    24,
                    item.span + (event.key === 'ArrowRight' ? 1 : -1)
                  )
                )
              );
            }
          }}
        />
      )}
    </div>
  );
}
