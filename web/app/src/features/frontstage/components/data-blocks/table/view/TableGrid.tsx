import type { Key, ReactNode } from 'react';
import {
  Button,
  Empty,
  Space,
  Table,
  Tooltip,
  Typography,
  type TableColumnsType
} from 'antd';
import { DndContext, closestCenter } from '@dnd-kit/core';
import {
  SortableContext,
  horizontalListSortingStrategy
} from '@dnd-kit/sortable';
import type { ConsoleRuntimeModelRecord } from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';
import { dataFieldText } from '../../../../lib/data-blocks/fields';
import { moveTableColumn, tableColumnEntries } from '../table-columns';
import { tableActionAllowed, tableRecordId } from '../table-model';
import {
  SortableTableColumnTitle,
  TableActionMenu
} from '../TableDesignControls';
import { TableColumnSettings } from '../TableSettings';
import { useTableDragSensors } from '../use-table-drag-sensors';
import { TableAction } from './TableAction';
import type { useTableConfigController } from '../use-table-config-controller';
import type { useTableSession } from '../runtime/use-table-session';
import type { useTableActions } from '../runtime/use-table-actions';

export function TableGrid({
  configuration,
  session,
  actions,
  disabled,
  design,
  fieldMenu
}: {
  configuration: ReturnType<typeof useTableConfigController>;
  session: ReturnType<typeof useTableSession>;
  actions: ReturnType<typeof useTableActions>;
  disabled: boolean;
  design: boolean;
  fieldMenu: ReactNode;
}) {
  const { config, saveConfig, saveAction, saveColumn } = configuration;
  const {
    page,
    pageSize,
    sort,
    selected,
    fieldsByCode,
    capabilities,
    records,
    data,
    idField,
    canAddressRecords,
    displayKeys
  } = session;
  const sensors = useTableDragSensors();
  const runAction = actions.runAction;
  const visibleColumns = config.columns.filter(
    (column) => !column.hidden && fieldsByCode.has(column.field_code)
  );
  const columns: TableColumnsType<ConsoleRuntimeModelRecord> = [];
  if (config.show_index)
    columns.push({
      key: '__index',
      title: '#',
      width: 50,
      render: (_, __, index) => (page - 1) * pageSize + index + 1
    });
  const rowActions = config.row_actions.filter((action) => action.enabled);
  const actionEntry = tableColumnEntries(config).find(
    (entry) => entry.kind === 'actions'
  );
  const showActionColumn =
    actionEntry?.kind === 'actions' && !actionEntry.hidden;
  if (
    showActionColumn &&
    (design ||
      (canAddressRecords &&
        rowActions.some((action) => tableActionAllowed(action, capabilities))))
  )
    columns.push({
      key: '__actions',
      width: 160,
      title: (
        <SortableTableColumnTitle
          fieldCode="actions"
          disabled={!design || disabled}
          tools={
            <TableActionMenu
              actions={config.row_actions}
              kinds={['view', 'edit', 'delete', 'open']}
              disabled={disabled}
              variant="settings"
              label={i18nText(
                'frontstage',
                'data_blocks.form.configure_actions'
              )}
              onChange={(action) => saveAction(action, true)}
            />
          }
        >
          <span>{i18nText('frontstage', 'files.actions')}</span>
        </SortableTableColumnTitle>
      ),
      render: (_, record) => (
        <Space size={0} wrap>
          {rowActions.map((action) => (
            <TableAction
              key={action.id}
              action={action}
              row
              record={record}
              session={session}
              actions={actions}
              configuration={configuration}
              disabled={disabled}
              design={design}
            />
          ))}
        </Space>
      )
    });
  for (const column of visibleColumns) {
    const field = fieldsByCode.get(column.field_code)!;
    const title = column.title || field.title;
    columns.push({
      key: column.field_code,
      dataIndex: column.field_code,
      width: column.width,
      fixed: column.fixed,
      ellipsis: column.ellipsis,
      sorter: column.sortable ? true : undefined,
      sortOrder:
        sort?.field === column.field_code
          ? sort.direction === 'asc'
            ? 'ascend'
            : 'descend'
          : null,
      title: (
        <SortableTableColumnTitle
          fieldCode={`field:${column.field_code}`}
          disabled={!design || disabled}
          tools={
            <TableColumnSettings
              column={column}
              title={title}
              disabled={disabled}
              onSave={saveColumn}
            />
          }
        >
          <Tooltip title={column.tooltip || field.description}>
            <span>{title}</span>
          </Tooltip>
        </SortableTableColumnTitle>
      ),
      render: (value, record) => {
        const text = dataFieldText(field, value);
        return canAddressRecords &&
          column.action &&
          tableActionAllowed(column.action, capabilities) ? (
          <Button
            type="link"
            size="small"
            className="data-table-cell-link"
            onClick={() => void runAction(column.action!, record)}
          >
            {text || '—'}
          </Button>
        ) : (
          text || '—'
        );
      }
    });
  }
  const order = tableColumnEntries(config).map((entry) =>
    entry.kind === 'actions' ? '__actions' : entry.column.field_code
  );
  columns.sort(
    (a, b) =>
      (a.key === '__index' ? -1 : order.indexOf(String(a.key))) -
      (b.key === '__index' ? -1 : order.indexOf(String(b.key)))
  );
  if (design)
    columns.push({
      key: '__fields',
      title: fieldMenu,
      width: 128,
      align: 'right'
    });

  const sortableIds: string[] = [];
  for (const entry of tableColumnEntries(config)) {
    if (entry.kind === 'actions' ? !entry.hidden : !entry.column.hidden)
      sortableIds.push(entry.id);
  }
  return (
    <>
      {!visibleColumns.length && !showActionColumn && (
        <div className="data-table-empty-columns">
          <Typography.Text type="secondary">
            {i18nText('frontstage', 'data_blocks.table.empty_columns')}
          </Typography.Text>
          {fieldMenu}
        </div>
      )}
      <DndContext
        sensors={sensors}
        collisionDetection={closestCenter}
        onDragEnd={({ active, over }) => {
          if (!design || disabled || !over || active.id === over.id) return;
          void saveConfig(
            moveTableColumn(config, String(active.id), String(over.id))
          );
        }}
      >
        <SortableContext
          items={sortableIds}
          strategy={horizontalListSortingStrategy}
        >
          <Table<ConsoleRuntimeModelRecord>
            size={config.density}
            columns={columns}
            dataSource={data}
            rowKey={(record) =>
              canAddressRecords
                ? tableRecordId(record, idField)!
                : displayKeys.get(record)!
            }
            loading={records.isPending && records.isFetching}
            scroll={{
              x: 'max-content',
              ...(config.height ? { y: config.height } : {})
            }}
            rowSelection={
              canAddressRecords
                ? {
                    selectedRowKeys: selected,
                    preserveSelectedRowKeys: true,
                    onChange: (keys: Key[]) => session.select(keys.map(String))
                  }
                : undefined
            }
            pagination={{
              current: page,
              pageSize,
              total: records.data?.total ?? 0,
              showSizeChanger: true,
              pageSizeOptions: [10, 20, 50, 100],
              size: 'small',
              showTotal: (total) =>
                i18nText('frontstage', 'data_blocks.table.total', { total })
            }}
            onChange={(pagination, _, sorter, extra) => {
              if (extra.action === 'paginate') {
                session.paginate(
                  pagination.current ?? 1,
                  pagination.pageSize ?? config.page_size
                );
              }
              if (extra.action === 'sort') {
                const current = Array.isArray(sorter) ? sorter[0] : sorter;
                session.sortBy(
                  current?.order
                    ? {
                        field: String(current.columnKey),
                        direction: current.order === 'ascend' ? 'asc' : 'desc'
                      }
                    : null
                );
              }
            }}
            locale={{
              emptyText: (
                <Empty
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  description={
                    visibleColumns.length
                      ? i18nText(
                          'frontstage',
                          'data_blocks.table.empty_records'
                        )
                      : i18nText(
                          'frontstage',
                          'data_blocks.table.empty_columns'
                        )
                  }
                >
                  {!visibleColumns.length && fieldMenu}
                </Empty>
              )
            }}
          />
        </SortableContext>
      </DndContext>
    </>
  );
}
