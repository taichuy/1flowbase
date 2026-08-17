import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Empty, Space, Table, Typography } from 'antd';
import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  getSettingsPricingCatalog,
  importSettingsPricingCatalog,
  settingsPricingCatalogQueryKey,
  settingsPricingRulesQueryKey
} from '../../api/billing';
import { SettingsSectionSurface } from '../SettingsSectionSurface';

export function PricingCatalogPanel() {
  const navigate = useNavigate();
  const csrf = useAuthStore((s) => s.csrfToken);
  const client = useQueryClient();
  const catalog = useQuery({
    queryKey: settingsPricingCatalogQueryKey,
    queryFn: () => getSettingsPricingCatalog()
  });
  const importing = useMutation({
    mutationFn: () => {
      if (!csrf) throw new Error('missing csrf token');
      return importSettingsPricingCatalog(
        (catalog.data?.rules ?? []).map((rule) => rule.id),
        csrf
      );
    },
    onSuccess: () =>
      client.invalidateQueries({ queryKey: settingsPricingRulesQueryKey })
  });
  return (
    <SettingsSectionSurface heightMode="fill">
      <Space direction="vertical" size={16} style={{ width: '100%' }}>
        <Tabs
          activeKey="model-pricing"
          onChange={(category) =>
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category },
              search: { q: undefined, cursor: undefined }
            })
          }
          items={[
            {
              key: 'installed',
              label: i18nText('settings', 'auto.installed_extensions')
            },
            { key: 'agent-flow', label: 'agent-flow' },
            { key: 'capability-plugins', label: 'capability-plugins' },
            { key: 'host-extensions', label: 'host-extensions' },
            { key: 'i18n', label: 'i18n' },
            { key: 'mcp', label: 'mcp' },
            { key: 'runtime-extensions', label: 'runtime-extensions' },
            {
              key: 'model-pricing',
              label: i18nText('settings', 'auto.billing_vendor_model_pricing')
            }
          ]}
        />
        <Alert
          type="info"
          showIcon
          title={i18nText('settings', 'auto.billing_catalog_truth')}
          description={
            catalog.data
              ? `${i18nText('settings', 'auto.translation_catalog_version')}: ${catalog.data.catalog_version}`
              : undefined
          }
        />
        <Button
          type="primary"
          disabled={!catalog.data || catalog.data.rules.length === 0}
          loading={importing.isPending}
          onClick={() => importing.mutate()}
        >
          {i18nText('settings', 'auto.billing_import_catalog')}
        </Button>
        {catalog.data?.rules.length === 0 ? (
          <Empty
            description={i18nText('settings', 'auto.billing_catalog_empty')}
          />
        ) : (
          <Table
            rowKey="id"
            dataSource={catalog.data?.rules ?? []}
            columns={[
              {
                title: i18nText('settings', 'auto.billing_provider_code'),
                dataIndex: 'provider_code'
              },
              {
                title: i18nText('settings', 'auto.billing_model_id'),
                dataIndex: 'upstream_model_id'
              },
              {
                title: i18nText('settings', 'auto.translation_catalog_version'),
                dataIndex: 'source_version'
              }
            ]}
          />
        )}
        <Typography.Text type="secondary">
          {i18nText('settings', 'auto.billing_import_behavior')}
        </Typography.Text>
      </Space>
    </SettingsSectionSurface>
  );
}
