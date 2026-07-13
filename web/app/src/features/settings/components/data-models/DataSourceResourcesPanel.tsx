import { Button, Empty, Flex, Space, Table, Tag, Typography } from 'antd';
import type { ColumnsType } from 'antd/es/table';
import {
  CheckCircleOutlined,
  EyeOutlined,
  LinkOutlined,
  ReloadOutlined
} from '@ant-design/icons';

import type {
  SettingsDataSourceConnection,
  SettingsDataSourceRemoteResource
} from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

export function DataSourceResourcesPanel({
  connection,
  resources,
  loading,
  validating,
  discovering,
  previewingResourceKey,
  mappingResourceKey,
  canManage,
  onValidate,
  onDiscover,
  onPreview,
  onMap
}: {
  connection: SettingsDataSourceConnection;
  resources: SettingsDataSourceRemoteResource[];
  loading: boolean;
  validating: boolean;
  discovering: boolean;
  previewingResourceKey: string | null;
  mappingResourceKey: string | null;
  canManage: boolean;
  onValidate: () => void;
  onDiscover: () => void;
  onPreview: (resource: SettingsDataSourceRemoteResource) => void;
  onMap: (resource: SettingsDataSourceRemoteResource) => void;
}) {
  const columns: ColumnsType<SettingsDataSourceRemoteResource> = [
    {
      title: i18nText('settings', 'auto.remote_resources'),
      key: 'display_name',
      render: (_, resource) => (
        <Space direction="vertical" size={2}>
          <Typography.Text strong>{resource.display_name}</Typography.Text>
          <Typography.Text type="secondary">
            <code className="data-model-panel__code-badge">
              {resource.resource_key}
            </code>
          </Typography.Text>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.kind'),
      dataIndex: 'resource_kind',
      key: 'resource_kind',
      width: 140,
      render: (kind: string) => <Tag>{kind}</Tag>
    },
    {
      title: i18nText('settings', 'auto.operation'),
      key: 'actions',
      width: 220,
      render: (_, resource) => (
        <Space>
          <Button
            type="link"
            icon={<EyeOutlined />}
            disabled={!canManage}
            loading={previewingResourceKey === resource.resource_key}
            onClick={() => onPreview(resource)}
          >
            {i18nText('settings', 'auto.preview')}
          </Button>
          <Button
            type="link"
            icon={<LinkOutlined />}
            disabled={!canManage}
            loading={mappingResourceKey === resource.resource_key}
            onClick={() => onMap(resource)}
          >
            {i18nText('settings', 'auto.map_to_data_model')}
          </Button>
        </Space>
      )
    }
  ];

  return (
    <section aria-labelledby="data-source-remote-resources-title">
      <Flex align="center" justify="space-between" gap={12} wrap="wrap">
        <Typography.Title
          id="data-source-remote-resources-title"
          level={5}
          style={{ margin: 0 }}
        >
          {i18nText('settings', 'auto.remote_resources')}
        </Typography.Title>
        {connection.status === 'ready' ? (
          <Button
            icon={<ReloadOutlined />}
            disabled={!canManage}
            loading={discovering}
            onClick={onDiscover}
          >
            {i18nText('settings', 'auto.discover_resources')}
          </Button>
        ) : (
          <Button
            type="primary"
            icon={<CheckCircleOutlined />}
            disabled={!canManage}
            loading={validating}
            onClick={onValidate}
          >
            {i18nText('settings', 'auto.validate_connection')}
          </Button>
        )}
      </Flex>
      {connection.status === 'ready' ? (
        <Table
          rowKey="resource_key"
          size="small"
          loading={loading}
          columns={columns}
          dataSource={resources}
          pagination={false}
          locale={{
            emptyText: (
              <Empty
                image={Empty.PRESENTED_IMAGE_SIMPLE}
                description={i18nText(
                  'settings',
                  'auto.discover_resources_empty'
                )}
              />
            )
          }}
        />
      ) : null}
    </section>
  );
}
