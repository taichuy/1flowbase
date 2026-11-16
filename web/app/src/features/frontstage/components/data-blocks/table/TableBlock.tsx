import {
  BLOCK_DESIGN_THEME,
  BLOCK_BUTTON_THEME
} from '../../../lib/native-modules/block-design/theme';
import { FRONTSTAGE_DESIGN_BLUE } from '../../../lib/design-mode-theme';
import { useDataBlockPopupContainer } from '../shared/useDataBlockPopupContainer';
import { DataBlockModal as Modal } from '../shared/DataBlockModal';
import {
  useState,
  type CSSProperties,
  type Key,
  type ReactNode
} from 'react';
import {
  Alert,
  Button,
  ConfigProvider,
  Empty,
  Popover,
  Space,
  Spin,
  Table,
  Tooltip,
  Typography,
  theme,
  type TableColumnsType
} from 'antd';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import FilterOutlined from '@ant-design/icons/es/icons/FilterOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import ReloadOutlined from '@ant-design/icons/es/icons/ReloadOutlined';
import { DndContext, closestCenter } from '@dnd-kit/core';
import {
  SortableContext,
  horizontalListSortingStrategy
} from '@dnd-kit/sortable';
import type { ConsoleRuntimeModelRecord } from '@1flowbase/api-client';
import { i18nText } from '../../../../../shared/i18n/text';
import { useDataBlockMetadata } from '../../../api/data-blocks';
import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import { dataFieldText } from '../../../lib/data-blocks/fields';
import {
  emptyDataFilter,
  readDataBlockConfig,
  type DataBlockAction,
  type DataBlockComponentProps,
  type TableBlockConfig
} from '../../../lib/data-blocks/types';
import { DataBlockFilterEditor } from '../shared/DataBlockFilterEditor';
import { DataFieldCreateModal } from '../shared/DataFieldCreateModal';
import {
  tableActionAllowed,
  tableRecordId
} from './table-model';
import { BlockDesignToolbar } from '../../../lib/native-modules/block-design/runtime';
import {
  restrictDataFilter,
  validDataFilter
} from '../../../lib/data-blocks/filter-variables';
import { moveTableColumn, tableColumnEntries } from './table-columns';
import { actionTitle } from './table-labels';
import {
  SortableTableColumnTitle,
  TableActionMenu,
  TableFieldMenu
} from './TableDesignControls';
import { useTableDragSensors } from './use-table-drag-sensors';
import { useTableConfigController } from './use-table-config-controller';
import { useTableSession } from './runtime/use-table-session';
import { useTableActions } from './runtime/use-table-actions';
import {
  TableActionSettings,
  TableBlockSettings,
  TableColumnSettings
} from './TableSettings';

export function TableBlock({ ctx }: DataBlockComponentProps) {
  const config = readDataBlockConfig(ctx.props);
  if (config?.kind !== 'table')
    return (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.invalid_config')}
      />
    );
  return (
    <ConfiguredTableBlock
      key={config.binding.model_id}
      ctx={ctx}
      initialConfig={config}
    />
  );
}

function ConfiguredTableBlock({
  ctx,
  initialConfig
}: DataBlockComponentProps & { initialConfig: TableBlockConfig }) {
  const host = useDataBlockHost();
  const popupContainer = useDataBlockPopupContainer();
  const { token } = theme.useToken();
  const [feedback, setFeedback] = useState<{
    type: 'error' | 'success' | 'warning';
    text: string;
  } | null>(null);
  const metadata = useDataBlockMetadata(ctx, initialConfig.binding);
  const fields = metadata.data?.fields ?? [];
  const { config, saving, saveConfig, saveAction, saveColumn } =
    useTableConfigController(
      initialConfig,
      host,
      fields,
      () => setFeedback(null),
      () => setFeedback({
        type: 'error',
        text: i18nText('frontstage', 'data_blocks.table.save_failed')
      })
    );
  const [filterDraft, setFilterDraft] = useState(emptyDataFilter);
  const [filterOpen, setFilterOpen] = useState(false);
  const [filterSaved, setFilterSaved] = useState(false);
  const [createField, setCreateField] = useState(false);
  const design = Boolean(host?.design_mode);
  const sensors = useTableDragSensors();
  const session = useTableSession(ctx, config, metadata);
  const {
    page, pageSize, sort, filter, selected, fieldsByCode, variables, scope,
    capabilities, invalidFields, invalidQuery, records, data, idField,
    canAddressRecords, missingRecordId, displayKeys
  } = session;
  const actions = useTableActions(ctx, host, config, session);
  const { actionParent, deleteIds, deleting, confirmDelete } = actions;
  const visibleFeedback = actions.feedback ?? feedback;
  const disabled = saving || Boolean(host?.saving) || !metadata.data;
  const canCreateField = design && Boolean(
    ctx.currentUser?.isRoot || ctx.currentUser?.permissions?.includes('model_fields.create')
  );
  const runAction = async (action: DataBlockAction, record?: ConsoleRuntimeModelRecord) => {
    if (!tableActionAllowed(action, capabilities)) return;
    if (action.kind === 'filter') {
      setFilterDraft(filter);
      setFilterOpen(true);
      return;
    }
    await actions.runAction(action, record);
  };

  const visibleColumns = config.columns.filter(
    (column) => !column.hidden && fieldsByCode.has(column.field_code)
  );
  const fieldMenu = design && (
    <TableFieldMenu
      fields={fields}
      config={config}
      disabled={disabled || !metadata.data}
      onChange={saveConfig}
      canCreate={canCreateField}
      onCreate={() => setCreateField(true)}
    />
  );
  const actionControl = (
    action: DataBlockAction,
    row: boolean,
    record?: ConsoleRuntimeModelRecord
  ) => {
    const allowed =
      tableActionAllowed(action, capabilities) &&
      (!(row || action.kind === 'delete') || canAddressRecords);
    if (!allowed && !design) return null;
    const icon = row ? undefined : action.kind === 'create' ? (
      <PlusOutlined />
    ) : action.kind === 'delete' ? (
      <DeleteOutlined />
    ) : action.kind === 'refresh' ? (
      <ReloadOutlined />
    ) : (
      <FilterOutlined />
    );
    return (
      <BlockDesignToolbar
        className="data-table-action"
        key={action.id}
        disabled={!design}
        tools={
          <TableActionSettings
            action={action}
            fields={fields}
            variables={variables}
            disabled={disabled}
            onSave={(next) => saveAction(next, row)}
          />
        }
      >
        <Button
          size="small"
          type={row ? 'link' : action.kind === 'create' ? 'primary' : 'default'}
          danger={action.kind === 'delete'}
          icon={icon}
          disabled={
            !allowed ||
            deleting ||
            (action.kind === 'delete' && !record && !selected.length) ||
            (['create', 'refresh'].includes(action.kind) && !scope.enabled) ||
            (action.kind === 'create' && !actionParent.canCreate) ||
            (action.kind === 'refresh' && invalidQuery)
          }
          loading={!row && action.kind === 'refresh' && records.isFetching}
          onClick={() => void runAction(action, record)}
        >
          {actionTitle(action)}
        </Button>
      </BlockDesignToolbar>
    );
  };
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
          {rowActions.map((action) => actionControl(action, true, record))}
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

  const filterActions = config.actions.filter(
    (action) => action.enabled && action.kind === 'filter'
  );
  const filterAction = filterActions[0];
  const filterableFields = fields.filter(
    (field) =>
      !filterAction?.filter_fields ||
      filterAction.filter_fields.includes(field.code)
  );
  const filterValid = validDataFilter(filterDraft, filterableFields, variables);
  const filterContent = (
    <div className="data-table-filter-panel">
      <DataBlockFilterEditor
        value={filterDraft}
        fields={filterableFields}
        variables={variables}
        onChange={(value) => {
          setFilterDraft(value);
          setFilterSaved(false);
        }}
      />
      {filterSaved && (
        <Typography.Text type="success">
          {i18nText('frontstage', 'data_blocks.filter.conditions_saved')}
        </Typography.Text>
      )}
      {!filterValid && (
        <Typography.Text type="secondary">
          {i18nText('frontstage', 'data_blocks.filter.incomplete')}
        </Typography.Text>
      )}
      <div className="data-table-filter-buttons">
        {design && filterAction && (
          <Button
            size="small"
            type="dashed"
            disabled={disabled || !filterValid}
            onClick={async () => {
              if (
                await saveAction(
                  { ...filterAction, default_filter: filterDraft },
                  false
                )
              ) {
                session.applyFilter(filterDraft);
                setFilterSaved(true);
              }
            }}
          >
            {i18nText('frontstage', 'data_blocks.filter.save_conditions')}
          </Button>
        )}
        <Button
          size="small"
          onClick={() => {
            const empty = filterAction?.default_filter ?? emptyDataFilter();
            setFilterDraft(empty);
            session.applyFilter(empty);
          }}
        >
          {i18nText('frontstage', 'data_blocks.table.reset')}
        </Button>
        <Button
          size="small"
          type="primary"
          disabled={!filterValid}
          onClick={() => {
            session.applyFilter(filterDraft);
            setFilterOpen(false);
          }}
        >
          {i18nText('frontstage', 'data_blocks.table.apply')}
        </Button>
      </div>
    </div>
  );
  let state: ReactNode = null;
  if (metadata.isPending) state = <Spin />;
  else if (metadata.isError)
    state = (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.metadata_failed')}
        action={
          <Button size="small" onClick={() => void metadata.refetch()}>
            {i18nText('frontstage', 'data_blocks.table.retry')}
          </Button>
        }
      />
    );
  else if (!capabilities?.can_list)
    state = (
      <Alert
        type="warning"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.list_forbidden')}
      />
    );
  else if (!scope.enabled)
    state = (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText(
          'frontstage',
          'data_blocks.table.awaiting_source'
        )}
      />
    );
  else if (invalidQuery)
    state = (
      <Alert
        type="warning"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.invalid_query_fields')}
      />
    );
  else if (records.isError)
    state = (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.records_failed')}
        action={
          <Button size="small" onClick={() => void records.refetch()}>
            {i18nText('frontstage', 'data_blocks.table.retry')}
          </Button>
        }
      />
    );
  else if (missingRecordId)
    state = (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'data_blocks.table.missing_record_id')}
      />
    );

  return (
    <ConfigProvider
      theme={design ? BLOCK_DESIGN_THEME : BLOCK_BUTTON_THEME}
      getPopupContainer={popupContainer}
    >
      <section
        className={`data-table-block${design ? ' data-table-block-design' : ''}`}
        aria-label={
          config.title || i18nText('frontstage', 'data_blocks.table.table')
        }
        style={
          {
            background: token.colorBgContainer,
            color: token.colorText,
            borderRadius: token.borderRadiusLG,
            '--data-table-design-color': FRONTSTAGE_DESIGN_BLUE.primary,
            '--data-table-border-color': token.colorBorderSecondary
          } as CSSProperties
        }
      >
        {(config.title || config.description) && (
          <div className="data-table-block-heading">
            <div>
              {config.title && (
                <Typography.Text strong>{config.title}</Typography.Text>
              )}
              {config.description && (
                <Typography.Paragraph type="secondary">
                  {config.description}
                </Typography.Paragraph>
              )}
            </div>
          </div>
        )}
        {design && host && (
          <TableBlockSettings
            config={config}
            fields={fields}
            variables={variables}
            targets={host.targets}
            blockId={host.block_id}
            disabled={disabled}
            onSave={saveConfig}
          />
        )}
        {visibleFeedback && (
          <Alert
            showIcon
            closable
            type={visibleFeedback.type}
            title={visibleFeedback.text}
            onClose={() => { setFeedback(null); actions.dismissFeedback(); }}
          />
        )}
        {metadata.data && !canAddressRecords && (
          <Alert
            type="info"
            showIcon
            title={i18nText(
              'frontstage',
              'data_blocks.table.unaddressable_records'
            )}
          />
        )}
        {invalidFields.length > 0 && (
          <Alert
            type="warning"
            showIcon
            title={i18nText('frontstage', 'data_blocks.table.invalid_columns')}
          />
        )}
        <div className="data-table-toolbar">
          <div>
            {filterActions.map(
              (action) =>
                (design || tableActionAllowed(action, capabilities)) && (
                  <Popover
                    key={action.id}
                    trigger="click"
                    placement="bottomLeft"
                    open={filterOpen}
                    onOpenChange={(open) => {
                      if (open)
                        setFilterDraft(
                          restrictDataFilter(
                            filter,
                            filterableFields.map((field) => field.code)
                          )
                        );
                      setFilterSaved(false);
                      setFilterOpen(open);
                    }}
                    content={filterContent}
                  >
                    {actionControl(action, false)}
                  </Popover>
                )
            )}
          </div>
          <Space size="small" wrap>
            {config.actions
              .filter((action) => action.enabled && action.kind !== 'filter')
              .map((action) => actionControl(action, false))}
            {design && (
              <TableActionMenu
                actions={config.actions}
                kinds={['filter', 'create', 'delete', 'refresh']}
                disabled={disabled}
                label={i18nText(
                  'frontstage',
                  'data_blocks.form.configure_actions'
                )}
                onChange={(action) => saveAction(action, false)}
              />
            )}
          </Space>
        </div>
        {!state && !visibleColumns.length && !showActionColumn && (
          <div className="data-table-empty-columns">
            <Typography.Text type="secondary">
              {i18nText('frontstage', 'data_blocks.table.empty_columns')}
            </Typography.Text>
            {fieldMenu}
          </div>
        )}
        {state ? (
          <div className="data-table-state">{state}</div>
        ) : (
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
              items={tableColumnEntries(config)
                .filter((entry) =>
                  entry.kind === 'actions'
                    ? !entry.hidden
                    : !entry.column.hidden
                )
                .map((entry) => entry.id)}
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
                    session.paginate(pagination.current ?? 1, pagination.pageSize ?? config.page_size);
                  }
                  if (extra.action === 'sort') {
                    const current = Array.isArray(sorter) ? sorter[0] : sorter;
                    session.sortBy(
                      current?.order
                        ? {
                            field: String(current.columnKey),
                            direction:
                              current.order === 'ascend' ? 'asc' : 'desc'
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
        )}
        {state && design && (
          <div className="data-table-state-config">{fieldMenu}</div>
        )}
        <Modal
          getContainer={false}
          open={deleteIds !== null}
          title={i18nText('frontstage', 'data_blocks.table.confirm_delete')}
          onCancel={actions.cancelDelete}
          confirmLoading={deleting}
          closable={!deleting}
          maskClosable={!deleting}
          cancelButtonProps={{ disabled: deleting }}
          okButtonProps={{
            danger: true,
            disabled: !deleteIds?.length || !capabilities?.can_delete
          }}
          okText={i18nText('frontstage', 'auto.delete')}
          cancelText={i18nText('frontstage', 'auto.cancel')}
          onOk={() => void confirmDelete()}
        >
          {i18nText('frontstage', 'data_blocks.table.delete_description', {
            count: deleteIds?.length ?? 0
          })}
        </Modal>
        {canCreateField && (
          <DataFieldCreateModal
            ctx={ctx}
            binding={config.binding}
            open={createField}
            onClose={() => setCreateField(false)}
            onCreated={async (fieldCode) => {
              await metadata.refetch();
              const existing = config.columns.find(
                (column) => column.field_code === fieldCode
              );
              const attached = await saveConfig({
                ...config,
                columns: existing
                  ? config.columns.map((column) =>
                      column.field_code === fieldCode
                        ? { ...column, hidden: false }
                        : column
                    )
                  : [...config.columns, { field_code: fieldCode }]
              });
              if (!attached)
                throw new Error(
                  i18nText(
                    'frontstage',
                    'data_blocks.fields.created_attach_failed'
                  )
                );
            }}
          />
        )}
      </section>
    </ConfigProvider>
  );
}
