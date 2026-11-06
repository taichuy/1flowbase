import { DataBlockModal as Modal } from '../shared/DataBlockModal';
import {
  useEffect,
  useMemo,
  useRef,
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
import {
  DeleteOutlined,
  FilterOutlined,
  PlusOutlined,
  ReloadOutlined
} from '@ant-design/icons';
import { DndContext, closestCenter } from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  horizontalListSortingStrategy
} from '@dnd-kit/sortable';
import type { ConsoleRuntimeModelRecord } from '@1flowbase/api-client';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  useDataBlockMetadata,
  useDataBlockRecords,
  useDataBlockWrites
} from '../../../api/data-blocks';
import { useDataBlockHost } from '../../../lib/data-blocks/host-context';
import { dataFieldText } from '../../../lib/data-blocks/fields';
import {
  emptyDataFilter,
  readDataBlockConfig,
  type DataBlockAction,
  type DataBlockColumn,
  type DataBlockComponentProps,
  type DataFilterGroup,
  type TableBlockConfig
} from '../../../lib/data-blocks/types';
import { DataBlockFilterEditor } from '../shared/DataBlockFilterEditor';
import { DataFieldCreateModal } from '../shared/DataFieldCreateModal';
import {
  deleteTableRecords,
  persistTableConfig,
  prepareTableAction,
  tableActionAllowed,
  tableActionInputs,
  tableActionParentContext,
  tableQueryScope,
  tableRecordId
} from './table-model';
import { actionTitle } from './table-labels';
import {
  SortableTableColumnTitle,
  TableActionMenu,
  TableFieldMenu,
  useTableDragSensors
} from './TableDesignControls';
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
  const containerRef = useRef<HTMLElement>(null);
  const { token } = theme.useToken();
  const [config, setConfig] = useState(initialConfig);
  useEffect(() => {
    setConfig(initialConfig);
  }, [initialConfig]);
  const [saving, setSaving] = useState(false);
  const savingRef = useRef(false);
  const [feedback, setFeedback] = useState<{
    type: 'error' | 'success' | 'warning';
    text: string;
  } | null>(null);
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(config.page_size);
  const [sort, setSort] = useState(config.sort);
  const [filter, setFilter] = useState(emptyDataFilter);
  const [filterDraft, setFilterDraft] = useState(emptyDataFilter);
  const [filterOpen, setFilterOpen] = useState(false);
  const [selected, setSelected] = useState<string[]>([]);
  const [deleteIds, setDeleteIds] = useState<string[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [createField, setCreateField] = useState(false);
  const design = Boolean(host?.design_mode);
  const sensors = useTableDragSensors();
  const metadata = useDataBlockMetadata(ctx, config.binding);
  const fields = metadata.data?.fields ?? [];
  const disabled = saving || Boolean(host?.saving) || !metadata.data;
  const actionParent = tableActionParentContext(config, fields, ctx.inputs);
  const capabilities = metadata.data?.record_capabilities;
  const writes = useDataBlockWrites(ctx, config.binding);
  const scope = tableQueryScope(config, filter, ctx.inputs);
  const scopeKey = JSON.stringify(scope.filter);
  const invalidFields = metadata.data
    ? config.columns.filter(
        (column) => !fields.some((field) => field.code === column.field_code)
      )
    : [];
  const filterFields = (group: DataFilterGroup): string[] =>
    group.conditions.flatMap((condition) =>
      'conditions' in condition
        ? filterFields(condition)
        : [condition.field_code]
    );
  const invalidQuery = Boolean(
    metadata.data &&
    [
      ...filterFields(config.filter),
      ...filterFields(filter),
      ...(sort ? [sort.field] : []),
      ...(config.relation ? [config.relation.field_code] : []),
      ...(config.linkage ? [config.linkage.field_code] : [])
    ].some((code) => !fields.some((field) => field.code === code))
  );
  const records = useDataBlockRecords(
    ctx,
    config.binding,
    {
      page,
      page_size: pageSize,
      filter: scope.filter,
      ...(sort ? { sort } : {})
    },
    Boolean(capabilities?.can_list && scope.enabled && !invalidQuery)
  );
  const idField = metadata.data?.record_id_field ?? '';
  const data = records.data?.items ?? [];
  const canAddressRecords = Boolean(idField);
  const missingRecordId =
    canAddressRecords &&
    data.some((record) => tableRecordId(record, idField) === null);
  // Rendering keys for unaddressable rows never enter actions or published outputs.
  const displayKeys = useMemo(
    () =>
      new Map(data.map((record, index) => [record, `display-only-${index}`])),
    [data]
  );
  const canCreateField =
    design &&
    Boolean(
      ctx.currentUser?.isRoot ||
      ctx.currentUser?.permissions?.includes('model_fields.create')
    );

  useEffect(() => {
    setPageSize(config.page_size);
    setPage(1);
  }, [config.page_size]);
  useEffect(() => {
    setSort(config.sort);
    setPage(1);
  }, [config.sort]);
  useEffect(() => {
    setPage(1);
    setSelected([]);
  }, [scopeKey, idField]);
  useEffect(() => {
    void ctx.outputs.publish({
      record_id: selected[0] ?? '',
      selected_record_ids: selected
    });
  }, [ctx.outputs, selected]);

  const commit = async (
    next: () => Promise<TableBlockConfig> | TableBlockConfig
  ) => {
    if (!host?.design_mode || savingRef.current || host.saving) return false;
    savingRef.current = true;
    setSaving(true);
    try {
      const candidate = await next();
      await persistTableConfig(host, candidate, setConfig);
      setFeedback(null);
      return true;
    } catch {
      setFeedback({
        type: 'error',
        text: i18nText('frontstage', 'data_blocks.table.save_failed')
      });
      return false;
    } finally {
      savingRef.current = false;
      setSaving(false);
    }
  };
  const saveConfig = (next: TableBlockConfig) =>
    commit(async () => {
      if (
        JSON.stringify([config.relation, config.linkage]) ===
        JSON.stringify([next.relation, next.linkage])
      )
        return next;
      const actions: DataBlockAction[] = [];
      const row_actions: DataBlockAction[] = [];
      const columns: DataBlockColumn[] = [];
      // Child targets share the host tree; prepare them sequentially.
      for (const action of next.actions)
        actions.push(await prepareTableAction(host!, next, action, fields));
      for (const action of next.row_actions)
        row_actions.push(await prepareTableAction(host!, next, action, fields));
      for (const column of next.columns)
        columns.push(
          column.action
            ? {
                ...column,
                action: await prepareTableAction(
                  host!,
                  next,
                  column.action,
                  fields
                )
              }
            : column
        );
      return { ...next, actions, row_actions, columns };
    });
  const saveAction = (action: DataBlockAction, row: boolean) =>
    commit(async () => {
      const prepared = await prepareTableAction(host!, config, action, fields);
      const key = row ? 'row_actions' : 'actions';
      const exists = config[key].some((item) => item.id === action.id);
      return {
        ...config,
        [key]: exists
          ? config[key].map((item) => (item.id === action.id ? prepared : item))
          : [...config[key], prepared]
      };
    });
  const saveColumn = (column: DataBlockColumn) =>
    commit(async () => ({
      ...config,
      columns: await Promise.all(
        config.columns.map(async (item) =>
          item.field_code === column.field_code
            ? {
                ...column,
                ...(column.action
                  ? {
                      action: await prepareTableAction(
                        host!,
                        config,
                        column.action,
                        fields
                      )
                    }
                  : {})
              }
            : item
        )
      )
    }));

  const runAction = async (
    action: DataBlockAction,
    record?: ConsoleRuntimeModelRecord
  ) => {
    if (!tableActionAllowed(action, capabilities)) return;
    const recordId = record ? tableRecordId(record, idField) : undefined;
    if (record && !recordId) return;
    if ((record || action.kind === 'delete') && !canAddressRecords) return;
    if (action.kind === 'delete') {
      const ids = recordId ? [recordId] : selected;
      if (ids.length) setDeleteIds(ids);
      return;
    }
    if (action.kind === 'refresh') {
      if (scope.enabled && !invalidQuery && capabilities?.can_list)
        await records.refetch();
      return;
    }
    if (action.kind === 'filter') {
      setFilterDraft(filter);
      setFilterOpen(true);
      return;
    }
    if (action.kind === 'create' && !actionParent.canCreate) return;
    if (!host || !action.target_block_id) {
      setFeedback({
        type: 'error',
        text: i18nText('frontstage', 'data_blocks.table.action_unavailable')
      });
      return;
    }
    try {
      await host.openAction(
        action,
        tableActionInputs(
          { ...config, relation: actionParent.relation },
          host.block_id,
          actionParent.inputs,
          recordId ?? undefined
        )
      );
    } catch {
      setFeedback({
        type: 'error',
        text: i18nText('frontstage', 'data_blocks.table.action_failed')
      });
    }
  };
  const confirmDelete = async () => {
    if (!deleteIds?.length || !capabilities?.can_delete || deleting) return;
    setDeleting(true);
    const result = await deleteTableRecords(deleteIds, (id) =>
      writes.remove.mutateAsync(id)
    );
    setSelected((current) =>
      Array.from(
        new Set([
          ...current.filter((id) => !result.deleted.includes(id)),
          ...result.failed
        ])
      )
    );
    setDeleteIds(null);
    setDeleting(false);
    setFeedback({
      type: result.failed.length ? 'warning' : 'success',
      text: result.failed.length
        ? i18nText('frontstage', 'data_blocks.table.delete_partial', {
            deleted: result.deleted.length,
            failed: result.failed.length
          })
        : i18nText('frontstage', 'data_blocks.table.delete_success', {
            count: result.deleted.length
          })
    });
    if (
      result.deleted.length &&
      data.length <= result.deleted.length &&
      page > 1
    )
      setPage(page - 1);
  };

  const visibleColumns = config.columns.filter(
    (column) =>
      !column.hidden && fields.some((field) => field.code === column.field_code)
  );
  const fieldMenu = design && (
    <TableFieldMenu
      fields={fields}
      columns={config.columns}
      disabled={disabled || !metadata.data}
      onChange={(columns) => saveConfig({ ...config, columns })}
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
      <span className="data-table-action" key={action.id}>
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
        {design && (
          <TableActionSettings
            action={action}
            disabled={disabled}
            onSave={(next) => saveAction(next, row)}
          />
        )}
      </span>
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
  if (
    design ||
    (canAddressRecords &&
      rowActions.some((action) => tableActionAllowed(action, capabilities)))
  )
    columns.push({
      key: '__actions',
      width: design ? 200 : 160,
      title: (
        <div className="data-table-row-actions-title">
          <span>{i18nText('frontstage', 'files.actions')}</span>
          {design && (
            <TableActionMenu
              actions={config.row_actions}
              kinds={['view', 'edit', 'delete', 'open']}
              disabled={disabled}
              label={i18nText(
                'frontstage',
                'data_blocks.table.configure_row_actions'
              )}
              onChange={(action) => saveAction(action, true)}
            />
          )}
        </div>
      ),
      render: (_, record) => (
        <Space size={0} wrap>
          {rowActions.map((action) => actionControl(action, true, record))}
        </Space>
      )
    });
  for (const column of visibleColumns) {
    const field = fields.find((item) => item.code === column.field_code)!;
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
          fieldCode={column.field_code}
          disabled={!design || disabled}
        >
          <Tooltip title={column.tooltip || field.description}>
            <span>{title}</span>
          </Tooltip>
          {design && (
            <TableColumnSettings
              column={column}
              title={title}
              disabled={disabled}
              onSave={saveColumn}
            />
          )}
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
  if (design) columns.push({ key: '__fields', title: fieldMenu, width: 160 });

  const filterActions = config.actions.filter(
    (action) => action.enabled && action.kind === 'filter'
  );
  const filterContent = (
    <div className="data-table-filter-panel">
      <DataBlockFilterEditor
        value={filterDraft}
        fields={fields}
        onChange={setFilterDraft}
      />
      <div className="data-table-filter-buttons">
        <Button
          size="small"
          onClick={() => {
            const empty = emptyDataFilter();
            setFilterDraft(empty);
            setFilter(empty);
            setPage(1);
          }}
        >
          {i18nText('frontstage', 'data_blocks.table.reset')}
        </Button>
        <Button
          size="small"
          type="primary"
          onClick={() => {
            setFilter(filterDraft);
            setPage(1);
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

  const providerTheme = useMemo(
    () => ({
      token: { colorPrimary: design ? token.blue : token.colorPrimary }
    }),
    [design, token.blue, token.colorPrimary]
  );
  return (
    <ConfigProvider
      theme={providerTheme}
      getPopupContainer={(trigger) =>
        trigger?.parentElement ?? containerRef.current!
      }
    >
      <section
        ref={containerRef}
        className={`data-table-block${design ? ' data-table-block-design' : ''}`}
        aria-label={
          config.title || i18nText('frontstage', 'data_blocks.table.table')
        }
        style={
          {
            background: token.colorBgContainer,
            color: token.colorText,
            borderRadius: token.borderRadiusLG,
            '--data-table-design-color': token.blue,
            '--data-table-border-color': token.colorBorderSecondary
          } as CSSProperties
        }
      >
        {(config.title || config.description || design) && (
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
            {design && host && (
              <TableBlockSettings
                config={config}
                fields={fields}
                targets={host.targets}
                blockId={host.block_id}
                disabled={disabled}
                onSave={saveConfig}
              />
            )}
          </div>
        )}
        {feedback && (
          <Alert
            showIcon
            closable
            type={feedback.type}
            title={feedback.text}
            onClose={() => setFeedback(null)}
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
                      if (open) setFilterDraft(filter);
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
        {!state && !visibleColumns.length && (
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
              const from = config.columns.findIndex(
                (column) => column.field_code === active.id
              );
              const to = config.columns.findIndex(
                (column) => column.field_code === over.id
              );
              if (from >= 0 && to >= 0)
                void saveConfig({
                  ...config,
                  columns: arrayMove(config.columns, from, to)
                });
            }}
          >
            <SortableContext
              items={visibleColumns.map((column) => column.field_code)}
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
                loading={records.isFetching}
                scroll={{
                  x: 'max-content',
                  ...(config.height ? { y: config.height } : {})
                }}
                rowSelection={
                  canAddressRecords
                    ? {
                        selectedRowKeys: selected,
                        preserveSelectedRowKeys: true,
                        onChange: (keys: Key[]) => setSelected(keys.map(String))
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
                    setPage(pagination.current ?? 1);
                    setPageSize(pagination.pageSize ?? config.page_size);
                  }
                  if (extra.action === 'sort') {
                    const current = Array.isArray(sorter) ? sorter[0] : sorter;
                    setSort(
                      current?.order
                        ? {
                            field: String(current.columnKey),
                            direction:
                              current.order === 'ascend' ? 'asc' : 'desc'
                          }
                        : null
                    );
                    setPage(1);
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
          onCancel={() => {
            if (!deleting) setDeleteIds(null);
          }}
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
