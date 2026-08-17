import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Button, Empty, Flex } from 'antd';
import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import { DataTableLayout } from '../../../../shared/ui/data-table/DataTableLayout';
import { useUserPreferenceDataTableConfiguration } from '../../../../shared/ui/data-table/user-preference-data-table';
import {
  getSettingsPricingCatalog,
  importSettingsPricingCatalog,
  settingsPricingCatalogQueryKey,
  settingsPricingRulesQueryKey,
  type SettingsPricingCatalog
} from '../../api/billing';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import './pricing-catalog-panel.css';
import { formatPricingRate } from './pricing-rate-display';

const PAGE_SIZE = 20;
type PricingCatalogRule = SettingsPricingCatalog['rules'][number];

export function PricingCatalogPanel() {
  const navigate = useNavigate();
  const csrf = useAuthStore((s) => s.csrfToken);
  const client = useQueryClient();
  const [page, setPage] = useState(1);
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
  const columns = useMemo<Array<DataTableColumn<PricingCatalogRule>>>(
    () => [
      {
        key: 'provider_code',
        title: i18nText('settings', 'auto.billing_provider_code'),
        dataIndex: 'provider_code',
        width: 180
      },
      {
        key: 'upstream_model_id',
        title: i18nText('settings', 'auto.billing_model_id'),
        dataIndex: 'upstream_model_id',
        width: 220
      },
      {
        key: 'input_price',
        title: i18nText('settings', 'auto.billing_input_price'),
        width: 200,
        render: (_: unknown, row: PricingCatalogRule) =>
          formatPricingRate(
            row.input_token_unit_price,
            row.input_token_unit_size
          )
      },
      {
        key: 'output_price',
        title: i18nText('settings', 'auto.billing_output_price'),
        width: 200,
        render: (_: unknown, row: PricingCatalogRule) =>
          formatPricingRate(
            row.output_token_unit_price,
            row.output_token_unit_size
          )
      },
      {
        key: 'cache_price',
        title: i18nText('settings', 'auto.billing_cache_price'),
        width: 200,
        render: (_: unknown, row: PricingCatalogRule) =>
          formatPricingRate(
            row.cache_hit_token_unit_price,
            row.cache_hit_token_unit_size
          )
      },
      {
        key: 'source_kind',
        title: i18nText('settings', 'auto.source'),
        dataIndex: 'source_kind',
        width: 120,
        render: (value: unknown) =>
          i18nText(
            'settings',
            value === 'official'
              ? 'auto.billing_source_official'
              : 'auto.billing_source_manual'
          )
      }
    ],
    []
  );
  const tableConfiguration = useUserPreferenceDataTableConfiguration({
    columns,
    preferenceKey: 'settings.pricing_catalog'
  });
  const rows = catalog.data?.rules ?? [];
  const currentPageRows = useMemo(
    () => rows.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE),
    [page, rows]
  );

  return (
    <SettingsSectionSurface heightMode="fill">
      <section className="pricing-catalog-panel">
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
        <DataTableLayout>
          <DataTable<PricingCatalogRule>
            columns={columns}
            configuration={tableConfiguration}
            dataSource={currentPageRows}
            emptyText={
              <Empty
                description={i18nText(
                  'settings',
                  'auto.billing_catalog_empty'
                )}
              />
            }
            loading={catalog.isLoading || catalog.isFetching}
            page={page}
            pageSize={PAGE_SIZE}
            rowKey="id"
            toolbar={
              <Flex justify="flex-end" gap={8} wrap>
                <Button
                  type="primary"
                  disabled={rows.length === 0}
                  loading={importing.isPending}
                  onClick={() => importing.mutate()}
                >
                  {i18nText('settings', 'auto.billing_import_catalog')}
                </Button>
                <DataTableColumnSettings
                  columns={columns}
                  configuration={tableConfiguration}
                />
              </Flex>
            }
            total={rows.length}
            onPageChange={setPage}
          />
        </DataTableLayout>
      </section>
    </SettingsSectionSurface>
  );
}
