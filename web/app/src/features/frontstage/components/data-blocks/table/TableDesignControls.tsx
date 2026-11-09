import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import { useState, type ReactNode } from 'react';
import { Button, Input, Popover, Switch, Typography } from 'antd';
import {
  HolderOutlined,
  PlusOutlined,
  SettingOutlined
} from '@ant-design/icons';
import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors
} from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { i18nText } from '../../../../../shared/i18n/text';
import type {
  DataBlockAction,
  DataBlockColumn,
  DataBlockField
} from '../../../lib/data-blocks/types';
import { actionTitle } from './table-labels';

export function useTableDragSensors() {
  return useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 6 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates })
  );
}

export function SortableTableColumnTitle({
  fieldCode,
  disabled,
  children
}: {
  fieldCode: string;
  disabled: boolean;
  children: ReactNode;
}) {
  const {
    attributes,
    listeners,
    setNodeRef,
    transform,
    transition,
    isDragging
  } = useSortable({ id: fieldCode, disabled });
  return (
    <div
      ref={setNodeRef}
      className="data-table-column-title"
      onClick={(event) => {
        if (
          event.target instanceof Element &&
          event.target.closest(
            'button, input, [role="dialog"], [role="listbox"]'
          )
        )
          event.stopPropagation();
      }}
      style={{
        transform: CSS.Translate.toString(transform),
        transition,
        opacity: isDragging ? 0.5 : 1
      }}
    >
      {!disabled && (
        <button
          className="data-table-drag-handle"
          type="button"
          {...attributes}
          {...listeners}
          aria-label={i18nText('frontstage', 'data_blocks.table.move_column')}
          onClick={(event) => event.stopPropagation()}
        >
          <HolderOutlined />
        </button>
      )}
      {children}
    </div>
  );
}

function SortableField({
  field,
  column,
  disabled,
  onToggle
}: {
  field: DataBlockField;
  column: DataBlockColumn;
  disabled: boolean;
  onToggle: () => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition } =
    useSortable({ id: field.code, disabled });
  return (
    <div
      ref={setNodeRef}
      className="data-table-field-option"
      style={{ transform: CSS.Translate.toString(transform), transition }}
    >
      <button
        type="button"
        className="data-table-drag-handle"
        {...attributes}
        {...listeners}
        disabled={disabled}
        aria-label={`${i18nText('frontstage', 'data_blocks.table.move_column')} ${field.title}`}
      >
        <HolderOutlined />
      </button>
      <span>{field.title}</span>
      <Switch
        size="small"
        aria-label={field.title}
        disabled={disabled}
        checked={!column.hidden}
        onChange={onToggle}
      />
    </div>
  );
}

export function TableFieldMenu({
  fields,
  columns,
  disabled,
  onChange,
  onCreate,
  canCreate
}: {
  fields: DataBlockField[];
  columns: DataBlockColumn[];
  disabled: boolean;
  onChange: (columns: DataBlockColumn[]) => Promise<boolean>;
  onCreate: () => void;
  canCreate: boolean;
}) {
  const popupContainer = useDataBlockPopupContainer();
  const [search, setSearch] = useState('');
  const sensors = useTableDragSensors();
  const matching = fields.filter((field) =>
    `${field.title} ${field.code}`
      .toLocaleLowerCase()
      .includes(search.toLocaleLowerCase())
  );
  const configured = columns.flatMap((column) => {
    const field = matching.find((item) => item.code === column.field_code);
    return field ? [{ column, field }] : [];
  });
  const available = matching.filter(
    (field) => !columns.some((column) => column.field_code === field.code)
  );
  const newFields = (relation: boolean) =>
    available
      .filter((field) => Boolean(field.relation_target_model_id) === relation)
      .map((field) => (
        <div key={field.code} className="data-table-field-option">
          <span>{field.title}</span>
          <Switch
            size="small"
            checked={false}
            disabled={disabled}
            aria-label={field.title}
            onChange={() =>
              void onChange([...columns, { field_code: field.code }])
            }
          />
        </div>
      ));
  return (
    <Popover
      getPopupContainer={popupContainer}
      trigger={['hover', 'click']}
      placement="bottomRight"
      content={
        <div className="data-table-field-menu">
          <Input
            size="small"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder={i18nText(
              'frontstage',
              'data_blocks.form.search_fields'
            )}
            aria-label={i18nText(
              'frontstage',
              'data_blocks.form.search_fields'
            )}
          />
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={({ active, over }) => {
              if (!over || active.id === over.id) return;
              const from = columns.findIndex(
                (column) => column.field_code === active.id
              );
              const to = columns.findIndex(
                (column) => column.field_code === over.id
              );
              if (from >= 0 && to >= 0)
                void onChange(arrayMove(columns, from, to));
            }}
          >
            <SortableContext
              items={configured.map(({ field }) => field.code)}
              strategy={verticalListSortingStrategy}
            >
              {configured.map(({ column, field }) => (
                <SortableField
                  key={field.code}
                  field={field}
                  column={column}
                  disabled={disabled}
                  onToggle={() =>
                    void onChange(
                      columns.map((item) =>
                        item.field_code === field.code
                          ? { ...item, hidden: !item.hidden }
                          : item
                      )
                    )
                  }
                />
              ))}
            </SortableContext>
          </DndContext>
          {newFields(false)}
          {available.some((field) => field.relation_target_model_id) && (
            <Typography.Text type="secondary">
              {i18nText('frontstage', 'data_blocks.table.relation_fields')}
            </Typography.Text>
          )}
          {newFields(true)}
          {columns.some(
            (column) =>
              !fields.some((field) => field.code === column.field_code)
          ) && (
            <Button
              size="small"
              disabled={disabled}
              onClick={() =>
                void onChange(
                  columns.filter((column) =>
                    fields.some((field) => field.code === column.field_code)
                  )
                )
              }
            >
              {i18nText(
                'frontstage',
                'data_blocks.table.remove_invalid_fields'
              )}
            </Button>
          )}
          {canCreate && (
            <Button
              block
              type="text"
              icon={<PlusOutlined />}
              disabled={disabled}
              onClick={onCreate}
            >
              {i18nText('frontstage', 'data_blocks.table.create_field')}
            </Button>
          )}
        </div>
      }
    >
      <Button
        className="data-table-design-button"
        size="small"
        icon={<SettingOutlined />}
        disabled={disabled}
      >
        {i18nText('frontstage', 'data_blocks.form.configure_fields')}
      </Button>
    </Popover>
  );
}

export function TableActionMenu({
  actions,
  kinds,
  disabled,
  onChange,
  label
}: {
  actions: DataBlockAction[];
  kinds: DataBlockAction['kind'][];
  disabled: boolean;
  onChange: (action: DataBlockAction) => Promise<boolean>;
  label: string;
}) {
  const popupContainer = useDataBlockPopupContainer();
  return (
    <Popover
      getPopupContainer={popupContainer}
      trigger={['hover', 'click']}
      placement="bottomRight"
      content={
        <div className="data-table-action-menu">
          {kinds.map((kind) => {
            const action = actions.find((item) => item.kind === kind) ?? {
              id: kind,
              kind,
              enabled: false,
              presentation: 'drawer'
            };
            return (
              <div className="data-table-field-option" key={kind}>
                <span>{actionTitle(action)}</span>
                <Switch
                  size="small"
                  checked={action.enabled}
                  disabled={disabled}
                  aria-label={actionTitle(action)}
                  onChange={(enabled) => void onChange({ ...action, enabled })}
                />
              </div>
            );
          })}
        </div>
      }
    >
      <Button
        className="data-table-design-button"
        size="small"
        icon={<SettingOutlined />}
        disabled={disabled}
      >
        {label}
      </Button>
    </Popover>
  );
}
