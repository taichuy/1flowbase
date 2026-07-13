import { useState } from 'react';

import {
  Button,
  Checkbox,
  Flex,
  Space,
  Table,
  Tag,
  Typography
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import {
  CloudServerOutlined,
  DatabaseOutlined,
  PlusOutlined,
  ReloadOutlined
} from '@ant-design/icons';

import type {
  CreateSettingsDataSourceInput,
  SettingsDataSource,
  SettingsDataSourceCatalogEntry
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';
import { DataSourceCreateDrawer } from './DataSourceCreateDrawer';

function defaultApiPolicyLabel(source: SettingsDataSource) {
  return source.default_data_model_status === 'published'
    ? i18nText('settings', 'auto.default_api_open')
    : i18nText('settings', 'auto.default_api_closed');
}

function dataSourceTypeLabel(source: SettingsDataSource) {
  return source.backend.kind === 'core'
    ? i18nText('settings', 'auto.built_in_data_source')
    : source.backend.source_code;
}

export function DataSourcePanel({
  dataSources,
  catalog,
  loading,
  creating,
  creationErrorMessage,
  canManage,
  onRefresh,
  onOpenDataSource,
  onCreateDataSource
}: {
  dataSources: SettingsDataSource[];
  catalog: SettingsDataSourceCatalogEntry[];
  loading: boolean;
  creating: boolean;
  creationErrorMessage: string | null;
  canManage: boolean;
  onRefresh: () => Promise<void>;
  onOpenDataSource: (dataSourceId: string) => void;
  onCreateDataSource: (input: CreateSettingsDataSourceInput) => Promise<void>;
}) {
  const [createDrawerOpen, setCreateDrawerOpen] = useState(false);
  const columns: ColumnsType<SettingsDataSource> = [
    {
      title: i18nText('settings', 'auto.data_source_name'),
      key: 'display_name',
      render: (_, dataSource) => (
        <Space size={12}>
          <div
            className={`data-model-panel__source-icon-wrapper ${dataSource.backend.kind}`}
          >
            {dataSource.backend.kind === 'core' ? (
              <DatabaseOutlined
                aria-hidden="true"
                className="data-model-panel__source-icon"
              />
            ) : (
              <CloudServerOutlined
                aria-hidden="true"
                className="data-model-panel__source-icon"
              />
            )}
          </div>
          <Typography.Text strong>{dataSource.display_name}</Typography.Text>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.kind'),
      key: 'type',
      width: 160,
      render: (_, dataSource) => <Tag>{dataSourceTypeLabel(dataSource)}</Tag>
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'status',
      key: 'status',
      width: 120,
      render: (status: string) => (
        <Tag color={status === 'ready' ? 'success' : 'default'}>
          {status === 'ready'
            ? i18nText('settings', 'auto.ready')
            : status}
        </Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.enabled_alt'),
      dataIndex: 'enabled',
      key: 'enabled',
      width: 100,
      align: 'center',
      render: (enabled: boolean, dataSource) => (
        <Checkbox
          aria-label={`${dataSource.display_name} ${i18nText('settings', 'auto.enabled_alt')}`}
          checked={enabled}
          disabled
        />
      )
    },
    {
      title: i18nText('settings', 'auto.default_policy'),
      key: 'default_policy',
      width: 180,
      render: (_, dataSource) => (
        <Tag>{defaultApiPolicyLabel(dataSource)}</Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'actions',
      width: 100,
      align: 'right',
      render: (_, dataSource) => (
        <Button
          type="link"
          aria-label={i18nText('settings', 'auto.view')}
          onClick={(event) => {
            event.stopPropagation();
            onOpenDataSource(dataSource.id);
          }}
        >
          {i18nText('settings', 'auto.view')}
        </Button>
      )
    }
  ];

  return (
    <Flex vertical gap={16} className="data-model-panel__sources">
      <Flex align="center" justify="space-between" gap={12} wrap="wrap">
        <Typography.Title level={4} style={{ margin: 0 }}>
          {i18nText('settings', 'auto.data_source')}
        </Typography.Title>
        <Space>
          <Button
            icon={<ReloadOutlined aria-hidden="true" />}
            onClick={onRefresh}
          >
            {i18nText('settings', 'auto.refresh')}
          </Button>
          <Button
            type="primary"
            icon={<PlusOutlined aria-hidden="true" />}
            disabled={!canManage || catalog.length === 0}
            onClick={() => setCreateDrawerOpen(true)}
          >
            {i18nText('settings', 'auto.add_data_source')}
          </Button>
        </Space>
      </Flex>

      <Table
        aria-label={i18nText('settings', 'auto.data_source_list')}
        rowKey="id"
        size="middle"
        loading={loading}
        columns={columns}
        dataSource={dataSources}
        pagination={false}
        scroll={{ x: 900 }}
        onRow={(dataSource) => ({
          onClick: () => onOpenDataSource(dataSource.id),
          style: { cursor: 'pointer' }
        })}
      />

      <DataSourceCreateDrawer
        open={createDrawerOpen}
        catalog={catalog}
        saving={creating}
        errorMessage={creationErrorMessage}
        onClose={() => setCreateDrawerOpen(false)}
        onCreate={async (input) => {
          await onCreateDataSource(input);
          setCreateDrawerOpen(false);
        }}
      />
    </Flex>
  );
}
