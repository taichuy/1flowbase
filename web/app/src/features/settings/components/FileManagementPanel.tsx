import { useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Button,
  Input,
  Modal,
  Space,
  Table,
  Tabs,
  Tag,
  Tooltip,
  App
} from 'antd';
import {
  DeleteOutlined,
  EditOutlined,
  EyeOutlined,
  PlusOutlined,
  ReloadOutlined
} from '@ant-design/icons';
import type { ColumnsType } from 'antd/es/table';

import { useAuthStore } from '../../../state/auth-store';
import {
  deleteSettingsFileStorage,
  deleteSettingsFileTable,
  fetchSettingsFileStorages,
  fetchSettingsFileTables,
  settingsFileStoragesQueryKey,
  settingsFileTablesQueryKey,
  updateSettingsFileTableBinding,
  type SettingsFileStorage,
  type SettingsFileTable
} from '../api/file-management';
import { FileStorageDrawer } from './FileStorageDrawer';
import { FileTableDrawer } from './FileTableDrawer';
import { SettingsSectionSurface } from './SettingsSectionSurface';
import './file-management-panel.css';
import { i18nText } from '../../../shared/i18n/text';

interface FileManagementPanelProps {
  isRoot: boolean;
  canViewTables: boolean;
  canCreateTables: boolean;
}

type DrawerMode = 'create' | 'view' | 'edit';

type StorageDrawerState =
  | { open: false; mode: null; record: null }
  | { open: true; mode: DrawerMode; record: SettingsFileStorage | null };

type TableDrawerState =
  | { open: false; mode: null; record: null }
  | { open: true; mode: DrawerMode; record: SettingsFileTable | null };

function getHealthStatusMeta(status: string) {
  switch (status) {
    case 'ready':
      return { color: 'green', label: 'ready' };
    case 'failed':
      return { color: 'red', label: 'failed' };
    default:
      return { color: 'default', label: status || 'unknown' };
  }
}

export function FileManagementPanel({
  isRoot,
  canViewTables,
  canCreateTables
}: FileManagementPanelProps) {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);

  const [storageSearch, setStorageSearch] = useState('');
  const [tableSearch, setTableSearch] = useState('');
  const [storageDrawer, setStorageDrawer] = useState<StorageDrawerState>({
    open: false,
    mode: null,
    record: null
  });
  const [tableDrawer, setTableDrawer] = useState<TableDrawerState>({
    open: false,
    mode: null,
    record: null
  });

  const {
    data: storages = [],
    isLoading: storagesLoading,
    refetch: refetchStorages
  } = useQuery({
    queryKey: settingsFileStoragesQueryKey,
    queryFn: fetchSettingsFileStorages,
    enabled: isRoot
  });

  const {
    data: tables = [],
    isLoading: tablesLoading,
    refetch: refetchTables
  } = useQuery({
    queryKey: settingsFileTablesQueryKey,
    queryFn: fetchSettingsFileTables,
    enabled: canViewTables
  });

  const filteredStorages = useMemo(
    () =>
      storages.filter((storage) => {
        const keyword = storageSearch.trim().toLowerCase();

        return (
          keyword.length === 0 ||
          storage.code.toLowerCase().includes(keyword) ||
          storage.title.toLowerCase().includes(keyword)
        );
      }),
    [storages, storageSearch]
  );

  const filteredTables = useMemo(
    () =>
      tables.filter((table) => {
        const keyword = tableSearch.trim().toLowerCase();

        return (
          keyword.length === 0 ||
          table.code.toLowerCase().includes(keyword) ||
          table.title.toLowerCase().includes(keyword)
        );
      }),
    [tables, tableSearch]
  );

  const ensureCsrfToken = () => {
    if (!csrfToken) {
      throw new Error('missing csrf token');
    }

    return csrfToken;
  };

  const handleDeleteStorage = (record: SettingsFileStorage) => {
    Modal.confirm({
      title: i18nText('settings', 'auto.confirm_delete'),
      content: i18nText(
        'settings',
        'auto.delete_storage_configuration_content',
        { value1: record.title, value2: record.code }
      ),
      okText: i18nText('settings', 'auto.delete'),
      okType: 'danger',
      cancelText: i18nText('settings', 'auto.cancel'),
      onOk: async () => {
        try {
          await deleteSettingsFileStorage(record.id, ensureCsrfToken());
          message.success(
            i18nText('settings', 'auto.storage_configuration_deleted')
          );
          await refetchStorages();
        } catch (error) {
          message.error(
            error instanceof Error
              ? error.message
              : i18nText('settings', 'auto.delete_failed_retry')
          );
        }
      }
    });
  };

  const handleDeleteTable = (record: SettingsFileTable) => {
    Modal.confirm({
      title: i18nText('settings', 'auto.confirm_delete'),
      content: i18nText('settings', 'auto.delete_file_table_content', {
        value1: record.title,
        value2: record.code
      }),
      okText: i18nText('settings', 'auto.delete'),
      okType: 'danger',
      cancelText: i18nText('settings', 'auto.cancel'),
      onOk: async () => {
        try {
          await deleteSettingsFileTable(record.id, ensureCsrfToken());
          message.success(i18nText('settings', 'auto.file_table_deleted'));
          await refetchTables();
        } catch (error) {
          message.error(
            error instanceof Error
              ? error.message
              : i18nText('settings', 'auto.delete_failed_retry')
          );
        }
      }
    });
  };

  const handleUpdateBinding = async (tableId: string, storageId: string) => {
    await updateSettingsFileTableBinding(
      tableId,
      { bound_storage_id: storageId },
      ensureCsrfToken()
    );
  };

  const storageColumns: ColumnsType<SettingsFileStorage> = [
    {
      title: i18nText('settings', 'auto.identifier'),
      dataIndex: 'code',
      key: 'code',
      width: 160
    },
    {
      title: i18nText('settings', 'auto.name'),
      dataIndex: 'title',
      key: 'title',
      width: 180
    },
    {
      title: i18nText('settings', 'auto.driver'),
      dataIndex: 'driver_type',
      key: 'driver_type',
      width: 120,
      render: (driverType: string) => <Tag>{driverType}</Tag>
    },
    {
      title: i18nText('settings', 'auto.default'),
      dataIndex: 'is_default',
      key: 'is_default',
      width: 90,
      render: (value: boolean) =>
        value ? (
          <Tag color="green">{i18nText('settings', 'auto.yes')}</Tag>
        ) : (
          <Tag>{i18nText('settings', 'auto.no')}</Tag>
        )
    },
    {
      title: i18nText('settings', 'auto.enabled'),
      dataIndex: 'enabled',
      key: 'enabled',
      width: 90,
      render: (value: boolean) =>
        value ? (
          <Tag color="blue">{i18nText('settings', 'auto.yes')}</Tag>
        ) : (
          <Tag color="default">{i18nText('settings', 'auto.no')}</Tag>
        )
    },
    {
      title: i18nText('settings', 'auto.health_status'),
      dataIndex: 'health_status',
      key: 'health_status',
      width: 120,
      render: (status: string) => {
        const meta = getHealthStatusMeta(status);
        return <Tag color={meta.color}>{meta.label}</Tag>;
      }
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'actions',
      width: 220,
      render: (_value, record) => (
        <Space size="small">
          <Tooltip title={i18nText('settings', 'auto.view')}>
            <Button
              type="link"
              size="small"
              icon={<EyeOutlined />}
              onClick={() =>
                setStorageDrawer({ open: true, mode: 'view', record })
              }
            >
              {i18nText('settings', 'auto.view')}
            </Button>
          </Tooltip>
          <Tooltip title={i18nText('settings', 'auto.edit')}>
            <Button
              type="link"
              size="small"
              icon={<EditOutlined />}
              onClick={() =>
                setStorageDrawer({ open: true, mode: 'edit', record })
              }
            >
              {i18nText('settings', 'auto.edit')}
            </Button>
          </Tooltip>
          <Tooltip
            title={
              record.is_default
                ? i18nText('settings', 'auto.default_storage_cannot_be_deleted')
                : i18nText('settings', 'auto.delete')
            }
          >
            <Button
              type="link"
              size="small"
              danger
              disabled={record.is_default}
              icon={<DeleteOutlined />}
              onClick={() => handleDeleteStorage(record)}
            >
              {i18nText('settings', 'auto.delete')}
            </Button>
          </Tooltip>
        </Space>
      )
    }
  ];

  const tableColumns: ColumnsType<SettingsFileTable> = [
    {
      title: i18nText('settings', 'auto.identifier'),
      dataIndex: 'code',
      key: 'code',
      width: 160
    },
    {
      title: i18nText('settings', 'auto.name'),
      dataIndex: 'title',
      key: 'title',
      width: 180
    },
    {
      title: i18nText('settings', 'auto.scope'),
      dataIndex: 'scope_kind',
      key: 'scope_kind',
      width: 120,
      render: (scopeKind: string) => <Tag>{scopeKind}</Tag>
    },
    {
      title: i18nText('settings', 'auto.bound_storage'),
      dataIndex: 'bound_storage_title',
      key: 'bound_storage_title',
      width: 220,
      render: (title: string | null, record) => {
        if (title) {
          return title;
        }

        return (
          <Tag color="orange">
            {record.bound_storage_id
              ? i18nText('settings', 'auto.unnamed_storage')
              : i18nText('settings', 'auto.unbound')}
          </Tag>
        );
      }
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'status',
      key: 'status',
      width: 120,
      render: (status: string) => (
        <Tag color={status === 'active' ? 'green' : 'default'}>{status}</Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'actions',
      width: isRoot ? 220 : 96,
      render: (_value, record) => (
        <Space size="small">
          <Tooltip title={i18nText('settings', 'auto.view')}>
            <Button
              type="link"
              size="small"
              icon={<EyeOutlined />}
              onClick={() =>
                setTableDrawer({ open: true, mode: 'view', record })
              }
            >
              {i18nText('settings', 'auto.view')}
            </Button>
          </Tooltip>
          {isRoot ? (
            <Tooltip title={i18nText('settings', 'auto.edit')}>
              <Button
                type="link"
                size="small"
                icon={<EditOutlined />}
                onClick={() =>
                  setTableDrawer({ open: true, mode: 'edit', record })
                }
              >
                {i18nText('settings', 'auto.edit')}
              </Button>
            </Tooltip>
          ) : null}
          {isRoot ? (
            <Tooltip
              title={
                record.is_builtin
                  ? i18nText(
                      'settings',
                      'auto.built_in_file_table_cannot_be_deleted'
                    )
                  : i18nText('settings', 'auto.delete')
              }
            >
              <Button
                type="link"
                size="small"
                danger
                disabled={record.is_builtin}
                icon={<DeleteOutlined />}
                onClick={() => handleDeleteTable(record)}
              >
                {i18nText('settings', 'auto.delete')}
              </Button>
            </Tooltip>
          ) : null}
        </Space>
      )
    }
  ];

  const showCreateOnlyTable = !canViewTables && canCreateTables;

  const storagePanel = (
    <div className="fm-tab-panel">
      <div className="fm-tab-toolbar">
        <div className="fm-toolbar">
          <Button
            type="primary"
            size="small"
            icon={<PlusOutlined />}
            onClick={() =>
              setStorageDrawer({ open: true, mode: 'create', record: null })
            }
          >
            {i18nText('settings', 'auto.new')}
          </Button>
          <Tooltip title={i18nText('settings', 'auto.refresh')}>
            <Button
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => refetchStorages()}
            />
          </Tooltip>
          <Input.Search
            allowClear
            value={storageSearch}
            placeholder={i18nText(
              'settings',
              'auto.search_storage_placeholder'
            )}
            size="small"
            style={{ width: 220 }}
            onChange={(event) => setStorageSearch(event.target.value)}
          />
        </div>
      </div>

      <Table
        rowKey="id"
        size="small"
        pagination={false}
        loading={storagesLoading}
        columns={storageColumns}
        dataSource={filteredStorages}
      />
    </div>
  );

  const tablePanel = (
    <div className="fm-tab-panel">
      <div className="fm-tab-toolbar">
        <div className="fm-toolbar">
          {canCreateTables ? (
            <Button
              type="primary"
              size="small"
              icon={<PlusOutlined />}
              onClick={() =>
                setTableDrawer({ open: true, mode: 'create', record: null })
              }
            >
              {i18nText('settings', 'auto.new')}
            </Button>
          ) : null}
          <Tooltip title={i18nText('settings', 'auto.refresh')}>
            <Button
              size="small"
              icon={<ReloadOutlined />}
              onClick={() => refetchTables()}
            />
          </Tooltip>
          <Input.Search
            allowClear
            value={tableSearch}
            placeholder={i18nText(
              'settings',
              'auto.search_file_table_placeholder'
            )}
            size="small"
            style={{ width: 220 }}
            onChange={(event) => setTableSearch(event.target.value)}
          />
        </div>
      </div>

      <Table
        rowKey="id"
        size="small"
        pagination={false}
        loading={tablesLoading}
        columns={tableColumns}
        dataSource={filteredTables}
      />
    </div>
  );

  const managementTabs = [
    ...(isRoot
      ? [
          {
            key: 'storages',
            label: i18nText('settings', 'auto.storage_configuration'),
            children: storagePanel
          }
        ]
      : []),
    ...(canViewTables
      ? [
          {
            key: 'tables',
            label: i18nText('settings', 'auto.file_table'),
            children: tablePanel
          }
        ]
      : [])
  ];

  return (
    <SettingsSectionSurface heightMode="fill">
      <div className="file-management-panel">
        {managementTabs.length > 0 ? (
          <section className="fm-section fm-tabs-section">
            <Tabs items={managementTabs} />
          </section>
        ) : null}

        {showCreateOnlyTable ? (
          <section className="fm-section">
            <div className="fm-section-header">
              <h3>{i18nText('settings', 'auto.file_table')}</h3>
              <div className="fm-toolbar">
                <Button
                  type="primary"
                  size="small"
                  icon={<PlusOutlined />}
                  onClick={() =>
                    setTableDrawer({ open: true, mode: 'create', record: null })
                  }
                >
                  {i18nText('settings', 'auto.new')}
                </Button>
              </div>
            </div>
            <p className="fm-create-only-info">
              {i18nText(
                'settings',
                'auto.create_file_table_without_view_permission_notice'
              )}
            </p>
          </section>
        ) : null}

        <FileStorageDrawer
          open={storageDrawer.open}
          mode={storageDrawer.mode ?? 'create'}
          record={storageDrawer.record}
          onClose={() =>
            setStorageDrawer({ open: false, mode: null, record: null })
          }
          onSuccess={() => {
            refetchStorages();
          }}
        />

        <FileTableDrawer
          open={tableDrawer.open}
          mode={tableDrawer.mode ?? 'create'}
          record={tableDrawer.record}
          storages={storages}
          onClose={() =>
            setTableDrawer({ open: false, mode: null, record: null })
          }
          onSuccess={() => {
            refetchTables();
          }}
          onUpdateBinding={handleUpdateBinding}
        />
      </div>
    </SettingsSectionSurface>
  );
}
