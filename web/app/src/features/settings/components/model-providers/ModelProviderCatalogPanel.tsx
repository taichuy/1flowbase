import { Button, Empty, Space, Table, Tag, Typography } from 'antd';

import { ScrollableSurface } from '../../../../shared/ui/scrollable-surface/ScrollableSurface';
import { i18nText } from '../../../../shared/i18n/text';
import type { SettingsModelProviderCatalogEntry } from '../../api/model-providers';
import { ModelProviderOverviewSummary } from '../../pages/settings-page/model-providers/ModelProviderOverviewSummary';

export function ModelProviderCatalogPanel({
  overviewRows,
  entries,
  loading,
  canManage,
  onCreate,
  onViewInstances
}: {
  overviewRows: { key: string; label: string; value: string }[];
  entries: SettingsModelProviderCatalogEntry[];
  loading?: boolean;
  canManage: boolean;
  onCreate: (entry: SettingsModelProviderCatalogEntry) => void;
  onViewInstances: (entry: SettingsModelProviderCatalogEntry) => void;
}) {
  return (
    <ScrollableSurface className="model-provider-panel__catalog">
      <div className="model-provider-panel__section-head">
        <ModelProviderOverviewSummary rows={overviewRows} />
      </div>

      <Table<SettingsModelProviderCatalogEntry>
        className="model-provider-panel__catalog-table"
        rowKey="provider_code"
        size="small"
        loading={loading}
        pagination={false}
        dataSource={entries}
        scroll={{ x: 760 }}
        locale={{
          emptyText: (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                loading
                  ? i18nText('settings', 'auto.loading_supplier_catalog')
                  : i18nText('settings', 'auto.suppliers_available_yet')
              }
            />
          )
        }}
        columns={[
          ...(canManage
            ? [
                {
                  title: i18nText('settings', 'auto.operation'),
                  key: 'actions',
                  width: 150,
                  render: (
                    _: unknown,
                    entry: SettingsModelProviderCatalogEntry
                  ) => (
                    <Space
                      size={4}
                      className="model-provider-panel__catalog-actions"
                    >
                      <Button
                        type="link"
                        onClick={() => onViewInstances(entry)}
                      >
                        {i18nText(
                          'settings',
                          'auto.model_provider_manage_action'
                        )}
                      </Button>
                      <Button type="link" onClick={() => onCreate(entry)}>
                        {i18nText('settings', 'auto.new')}
                      </Button>
                    </Space>
                  )
                }
              ]
            : []),
          {
            title: i18nText('settings', 'auto.name'),
            key: 'provider',
            width: 200,
            render: (_, entry) => (
              <Typography.Text strong>{entry.display_name}</Typography.Text>
            )
          },
          {
            title: i18nText('settings', 'auto.model_discovery'),
            dataIndex: 'model_discovery_mode',
            key: 'model_discovery_mode',
            width: 160,
            render: (value: string) => <Tag>{value}</Tag>
          },
          {
            title: i18nText('settings', 'auto.description'),
            key: 'description',
            render: (_, entry) => (
              <Typography.Paragraph
                className="model-provider-panel__catalog-description-text"
                ellipsis={{
                  rows: 2,
                  tooltip:
                    entry.description_key ??
                    i18nText('settings', 'auto.no_description_provided')
                }}
              >
                {entry.description_key ??
                  i18nText('settings', 'auto.no_description_provided')}
              </Typography.Paragraph>
            )
          }
        ]}
      />
    </ScrollableSurface>
  );
}
