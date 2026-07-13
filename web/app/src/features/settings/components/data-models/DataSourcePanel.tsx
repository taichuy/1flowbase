import { useState } from 'react';

import { Button, Flex, Space, Table, Tag, Typography } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import {
  CloudServerOutlined,
  DatabaseOutlined,
  PlusOutlined,
  RightOutlined
} from '@ant-design/icons';

import type {
  CreateSettingsDataSourceConnectionInput,
  SettingsDataSourceCatalogEntry,
  SettingsDataSourceConnection,
  SettingsMainDataSource
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';
import { DataSourceConnectionDrawer } from './DataSourceConnectionDrawer';

function defaultApiPolicyLabel(
  source: SettingsMainDataSource | SettingsDataSourceConnection
) {
  return source.default_data_model_status === 'published'
    ? i18nText('settings', 'auto.default_api_open')
    : i18nText('settings', 'auto.default_api_closed');
}

export function DataSourcePanel({
  mainSource,
  connections,
  catalog,
  loading,
  creating,
  creationErrorMessage,
  canManage,
  onOpenMainSource,
  onOpenConnection,
  onCreateConnection
}: {
  mainSource: SettingsMainDataSource | null;
  connections: SettingsDataSourceConnection[];
  catalog: SettingsDataSourceCatalogEntry[];
  loading: boolean;
  creating: boolean;
  creationErrorMessage: string | null;
  canManage: boolean;
  onOpenMainSource: () => void;
  onOpenConnection: (connectionId: string) => void;
  onCreateConnection: (
    input: CreateSettingsDataSourceConnectionInput
  ) => Promise<void>;
}) {
  const [connectionDrawerOpen, setConnectionDrawerOpen] = useState(false);
  const columns: ColumnsType<SettingsDataSourceConnection> = [
    {
      title: i18nText('settings', 'auto.connection_name'),
      key: 'display_name',
      render: (_, connection) => (
        <Space size={12}>
          <div className="data-model-panel__source-icon-wrapper external_source">
            <CloudServerOutlined className="data-model-panel__source-icon" />
          </div>
          <Space direction="vertical" size={2}>
            <Typography.Text strong>{connection.display_name}</Typography.Text>
            <Typography.Text type="secondary">
              <code className="data-model-panel__code-badge">
                {connection.source_code}
              </code>
            </Typography.Text>
          </Space>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'status',
      key: 'status',
      width: 120,
      render: (status: string) => (
        <Tag color={status === 'ready' ? 'success' : 'default'}>{status}</Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.default_policy'),
      key: 'default_policy',
      width: 180,
      render: (_, connection) => <Tag>{defaultApiPolicyLabel(connection)}</Tag>
    },
    {
      title: '',
      key: 'actions',
      width: 72,
      align: 'right',
      render: (_, connection) => (
        <Button
          type="text"
          aria-label={i18nText('settings', 'auto.configuration_alt')}
          icon={<RightOutlined />}
          onClick={(event) => {
            event.stopPropagation();
            onOpenConnection(connection.id);
          }}
        />
      )
    }
  ];

  return (
    <Flex vertical gap={24} className="data-model-panel__sources">
      <section aria-labelledby="data-source-main-title">
        <Typography.Title id="data-source-main-title" level={5}>
          {i18nText('settings', 'auto.main_data_source')}
        </Typography.Title>
        {mainSource ? (
          <button
            type="button"
            className="data-model-panel__main-source"
            onClick={onOpenMainSource}
          >
            <Space size={12}>
              <div className="data-model-panel__source-icon-wrapper main_source">
                <DatabaseOutlined className="data-model-panel__source-icon" />
              </div>
              <Space direction="vertical" size={2}>
                <Typography.Text strong>{mainSource.display_name}</Typography.Text>
                <Typography.Text type="secondary">
                  {defaultApiPolicyLabel(mainSource)}
                </Typography.Text>
              </Space>
            </Space>
            <RightOutlined />
          </button>
        ) : null}
      </section>

      <section aria-labelledby="data-source-connections-title">
        <Flex align="center" justify="space-between" gap={12} wrap="wrap">
          <Typography.Title id="data-source-connections-title" level={5}>
            {i18nText('settings', 'auto.external_connections')}
          </Typography.Title>
          <Button
            type="primary"
            icon={<PlusOutlined />}
            aria-label={i18nText('settings', 'auto.new_connection')}
            disabled={!canManage || catalog.length === 0}
            onClick={() => setConnectionDrawerOpen(true)}
          >
            {i18nText('settings', 'auto.new_connection')}
          </Button>
        </Flex>
        <Table
          rowKey="id"
          size="middle"
          loading={loading}
          columns={columns}
          dataSource={connections}
          pagination={false}
          scroll={{ x: 680 }}
          onRow={(connection) => ({
            onClick: () => onOpenConnection(connection.id),
            style: { cursor: 'pointer' }
          })}
        />
      </section>

      <DataSourceConnectionDrawer
        open={connectionDrawerOpen}
        catalog={catalog}
        saving={creating}
        errorMessage={creationErrorMessage}
        onClose={() => setConnectionDrawerOpen(false)}
        onCreate={async (input) => {
          await onCreateConnection(input);
          setConnectionDrawerOpen(false);
        }}
      />
    </Flex>
  );
}
