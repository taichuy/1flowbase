import {
  Button,
  Grid,
  Table,
  Tag,
  Typography,
  Space,
  Flex,
  Checkbox
} from 'antd';
import type { ColumnsType } from 'antd/es/table';
import {
  DatabaseOutlined,
  CloudServerOutlined,
  RightOutlined
} from '@ant-design/icons';

import type { SettingsDataSourceInstance } from '../../api/data-models';
import { i18nText } from '../../../../shared/i18n/text';

export function DataSourcePanel({
  sources,
  loading,
  onOpenSource
}: {
  sources: SettingsDataSourceInstance[];
  loading: boolean;
  onOpenSource: (sourceId: string) => void;
}) {
  const screens = Grid.useBreakpoint();
  const useMobileList = Boolean(screens.xs && !screens.md);

  const columns: ColumnsType<SettingsDataSourceInstance> = [
    {
      title: i18nText('settings', 'auto.data_source_name'),
      key: 'display_name',
      render: (_, source) => (
        <Space size={12}>
          <div
            className={`data-model-panel__source-icon-wrapper ${source.source_kind}`}
          >
            {source.source_kind === 'main_source' ? (
              <DatabaseOutlined className="data-model-panel__source-icon" />
            ) : (
              <CloudServerOutlined className="data-model-panel__source-icon" />
            )}
          </div>
          <Space direction="vertical" size={2}>
            <Typography.Text strong className="data-model-panel__source-title">
              {source.display_name}
            </Typography.Text>
            <Typography.Text type="secondary" style={{ fontSize: 12 }}>
              {i18nText('settings', 'auto.logo')}{' '}
              <code className="data-model-panel__code-badge">
                {source.source_code}
              </code>
            </Typography.Text>
          </Space>
        </Space>
      )
    },
    {
      title: i18nText('settings', 'auto.kind'),
      dataIndex: 'source_kind',
      key: 'source_kind',
      width: 140,
      render: (value: string) => (
        <Tag
          color={value === 'main_source' ? 'blue' : 'purple'}
          style={{ borderRadius: 6, margin: 0 }}
        >
          {value === 'main_source'
            ? i18nText('settings', 'auto.built_master_data_source')
            : i18nText('settings', 'auto.external_data_source')}
        </Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.status'),
      dataIndex: 'status',
      key: 'status',
      width: 100,
      render: (value: string) => (
        <Tag
          color={value === 'ready' ? 'success' : 'default'}
          style={{ borderRadius: 12, paddingInline: 8, margin: 0 }}
        >
          {value === 'ready' ? i18nText('settings', 'auto.ready') : value}
        </Tag>
      )
    },
    {
      title: i18nText('settings', 'auto.enabled'),
      key: 'enabled',
      width: 100,
      render: (_, source) => (
        <Checkbox
          aria-label={i18nText('settings', 'auto.enable', {
            value1: source.display_name
          })}
          checked={source.status === 'ready'}
          className="data-model-panel__enabled-check"
          disabled
        />
      )
    },
    {
      title: i18nText('settings', 'auto.default_policy'),
      key: 'default_policies',
      width: 260,
      render: (_, source) => (
        <Space size={8}>
          <Tag style={{ borderRadius: 6, margin: 0 }} color="default">
            {i18nText('settings', 'auto.modeling')}
            {source.default_data_model_status}
          </Tag>
        </Space>
      )
    },
    {
      title: '',
      key: 'actions',
      width: 80,
      align: 'right',
      render: (_, source) => (
        <Button
          type="primary"
          ghost
          size="small"
          aria-label={i18nText('settings', 'auto.configuration_alt')}
          className="data-model-panel__enter-btn"
          icon={<RightOutlined aria-hidden="true" />}
          onClick={(event) => {
            event.stopPropagation();
            onOpenSource(source.id);
          }}
        />
      )
    }
  ];

  return (
    <div className="data-model-panel__sources">
      {!useMobileList ? (
        <Table
          rowKey="id"
          size="middle"
          loading={loading}
          columns={columns}
          dataSource={sources}
          pagination={false}
          scroll={{ x: 760 }}
          className="data-model-panel__sources-table"
          onRow={(record) => ({
            onClick: () => onOpenSource(record.id),
            style: { cursor: 'pointer' }
          })}
        />
      ) : null}
      {useMobileList ? (
        <div className="data-model-panel__mobile-list">
          {sources.map((source) => (
            <div
              key={source.id}
              className="data-model-panel__mobile-item data-model-panel__mobile-item--clickable"
              onClick={() => onOpenSource(source.id)}
            >
              <Flex
                align="center"
                justify="space-between"
                style={{ width: '100%' }}
              >
                <Space size={12}>
                  <div
                    className={`data-model-panel__source-icon-wrapper ${source.source_kind}`}
                  >
                    {source.source_kind === 'main_source' ? (
                      <DatabaseOutlined className="data-model-panel__source-icon" />
                    ) : (
                      <CloudServerOutlined className="data-model-panel__source-icon" />
                    )}
                  </div>
                  <Space direction="vertical" size={2}>
                    <Typography.Text strong>
                      {source.display_name}
                    </Typography.Text>
                    <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                      {source.source_code}
                    </Typography.Text>
                  </Space>
                </Space>
                <RightOutlined
                  style={{ color: 'var(--ant-color-text-tertiary)' }}
                />
              </Flex>
              <Flex gap={8} style={{ marginTop: 12 }} wrap="wrap">
                <Tag
                  color={
                    source.source_kind === 'main_source' ? 'blue' : 'purple'
                  }
                  style={{ borderRadius: 6, margin: 0 }}
                >
                  {source.source_kind === 'main_source'
                    ? i18nText('settings', 'auto.built_in_source')
                    : i18nText('settings', 'auto.external')}
                </Tag>
                <Tag
                  color={source.status === 'ready' ? 'success' : 'default'}
                  style={{ borderRadius: 12, margin: 0 }}
                >
                  {source.status === 'ready'
                    ? i18nText('settings', 'auto.ready')
                    : source.status}
                </Tag>
                <Tag style={{ borderRadius: 6, margin: 0 }} color="default">
                  {i18nText('settings', 'auto.modeling')}
                  {source.default_data_model_status}
                </Tag>
              </Flex>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  );
}
