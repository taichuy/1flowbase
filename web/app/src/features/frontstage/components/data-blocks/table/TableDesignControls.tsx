import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import { Fragment, useState, type ReactNode } from 'react';
import { Button, Input, Popover, Switch, Typography } from 'antd';
import { HolderOutlined, PlusOutlined, DragOutlined } from '@ant-design/icons';
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
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import { i18nText } from '../../../../../shared/i18n/text';
import type {
  DataBlockAction,
  DataBlockField,
  TableBlockConfig
} from '../../../lib/data-blocks/types';
import {
  BlockDesignButton,
  BlockDesignToolbar
} from '../../../lib/native-modules/block-design/runtime';
import { moveTableColumn, tableColumnEntries } from './table-columns';
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
  children,
  tools
}: {
  fieldCode: string;
  disabled: boolean;
  children: ReactNode;
  tools?: ReactNode;
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
      <BlockDesignToolbar
        disabled={disabled}
        tools={
          <>
            <BlockDesignButton
              className="data-table-column-move"
              icon={<DragOutlined />}
              {...attributes}
              {...listeners}
              aria-label={i18nText(
                'frontstage',
                'data_blocks.table.move_column'
              )}
              onClick={(event) => event.stopPropagation()}
            />
            {tools}
          </>
        }
      >
        {children}
      </BlockDesignToolbar>
    </div>
  );
}

function SortableField({
  id,
  title,
  checked,
  disabled,
  onToggle
}: {
  id: string;
  title: string;
  checked: boolean;
  disabled: boolean;
  onToggle: () => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition } =
    useSortable({ id, disabled });
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
        aria-label={`${i18nText('frontstage', 'data_blocks.table.move_column')} ${title}`}
      >
        <HolderOutlined />
      </button>
      <span>{title}</span>
      <Switch
        size="small"
        aria-label={title}
        disabled={disabled}
        checked={checked}
        onChange={onToggle}
      />
    </div>
  );
}

export function TableFieldMenu({
  fields,
  config,
  disabled,
  onChange,
  onCreate,
  canCreate
}: {
  fields: DataBlockField[];
  config: TableBlockConfig;
  disabled: boolean;
  onChange: (config: TableBlockConfig) => Promise<boolean>;
  onCreate: () => void;
  canCreate: boolean;
}) {
  const popupContainer = useDataBlockPopupContainer();
  const [search, setSearch] = useState('');
  const sensors = useTableDragSensors();
  const actionTitle = i18nText(
    'frontstage',
    'data_blocks.table.configure_row_actions'
  );
  const matches = (title: string) =>
    title.toLocaleLowerCase().includes(search.toLocaleLowerCase());
  const configured = tableColumnEntries(config).filter((entry) =>
    entry.kind === 'actions'
      ? matches(actionTitle)
      : fields.some(
          (field) =>
            field.code === entry.column.field_code &&
            matches(`${field.title} ${field.code}`)
        )
  );
  const available = fields
    .filter(
      (field) =>
        matches(`${field.title} ${field.code}`) &&
        !config.columns.some((column) => column.field_code === field.code)
    )
    .sort(
      (left, right) =>
        Number(Boolean(left.relation_target_model_id)) -
        Number(Boolean(right.relation_target_model_id))
    );
  return (
    <Popover
      getPopupContainer={popupContainer}
      trigger={['click']}
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
          <Typography.Text type="secondary">
            {i18nText('frontstage', 'data_blocks.table.display_fields')}
          </Typography.Text>
          <DndContext
            sensors={sensors}
            collisionDetection={closestCenter}
            onDragEnd={({ active, over }) => {
              if (over && active.id !== over.id)
                void onChange(
                  moveTableColumn(config, String(active.id), String(over.id))
                );
            }}
          >
            <SortableContext
              items={configured.map((entry) => entry.id)}
              strategy={verticalListSortingStrategy}
            >
              {configured.map((entry) => {
                const field =
                  entry.kind === 'field'
                    ? fields.find(
                        (field) => field.code === entry.column.field_code
                      )!
                    : null;
                return (
                  <SortableField
                    key={entry.id}
                    id={entry.id}
                    title={field?.title ?? actionTitle}
                    checked={
                      entry.kind === 'actions'
                        ? !entry.hidden
                        : !entry.column.hidden
                    }
                    disabled={disabled}
                    onToggle={() =>
                      void onChange(
                        entry.kind === 'actions'
                          ? {
                              ...config,
                              action_column: {
                                hidden: !entry.hidden,
                                position: config.action_column?.position ?? 0
                              }
                            }
                          : {
                              ...config,
                              columns: config.columns.map((column) =>
                                column.field_code === entry.column.field_code
                                  ? { ...column, hidden: !column.hidden }
                                  : column
                              )
                            }
                      )
                    }
                  />
                );
              })}
            </SortableContext>
          </DndContext>
          {available.map((field, index) => (
            <Fragment key={field.code}>
              {field.relation_target_model_id &&
                !available[index - 1]?.relation_target_model_id && (
                  <Typography.Text type="secondary">
                    {i18nText(
                      'frontstage',
                      'data_blocks.table.relation_fields'
                    )}
                  </Typography.Text>
                )}
              <div key={field.code} className="data-table-field-option">
                <span>{field.title}</span>
                <Switch
                  size="small"
                  checked={false}
                  disabled={disabled}
                  aria-label={field.title}
                  onChange={() =>
                    void onChange({
                      ...config,
                      columns: [...config.columns, { field_code: field.code }]
                    })
                  }
                />
              </div>
            </Fragment>
          ))}
          {config.columns.some(
            (column) =>
              !fields.some((field) => field.code === column.field_code)
          ) && (
            <Button
              size="small"
              disabled={disabled}
              onClick={() =>
                void onChange({
                  ...config,
                  columns: config.columns.filter((column) =>
                    fields.some((field) => field.code === column.field_code)
                  )
                })
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
      <BlockDesignButton variant="initializer" disabled={disabled}>
        {i18nText('frontstage', 'data_blocks.form.configure_fields')}
      </BlockDesignButton>
    </Popover>
  );
}

export function TableActionMenu({
  actions,
  kinds,
  disabled,
  onChange,
  label,
  variant = 'initializer'
}: {
  actions: DataBlockAction[];
  kinds: DataBlockAction['kind'][];
  disabled: boolean;
  onChange: (action: DataBlockAction) => Promise<boolean>;
  label: string;
  variant?: 'settings' | 'initializer';
}) {
  const popupContainer = useDataBlockPopupContainer();
  return (
    <Popover
      getPopupContainer={popupContainer}
      trigger={['click']}
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
      <BlockDesignButton variant={variant} disabled={disabled}>
        {label}
      </BlockDesignButton>
    </Popover>
  );
}
