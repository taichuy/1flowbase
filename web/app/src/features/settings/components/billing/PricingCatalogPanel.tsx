import { useMemo, useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Button, Empty, Flex, Input, Tag } from 'antd';
import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn
} from '../../../../shared/ui/data-table/DataTable';
import {
  DataTableFilterField,
  DataTableFilterForm,
  DataTableLayout
} from '../../../../shared/ui/data-table/DataTableLayout';
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
type PricingCatalogRule = SettingsPricingCatalog['items'][number];

export function PricingCatalogPanel() {
  const navigate = useNavigate();
  const csrf = useAuthStore((s) => s.csrfToken);
  const client = useQueryClient();
  const [page, setPage] = useState(1);
  const [providerCode, setProviderCode] = useState('');
  const [upstreamModelId, setUpstreamModelId] = useState('');
  const [appliedFilters, setAppliedFilters] = useState({
    provider_code: undefined as string | undefined,
    upstream_model_id: undefined as string | undefined
  });
  const filter = useMemo(
    () => ({ ...appliedFilters, page, page_size: PAGE_SIZE }),
    [appliedFilters, page]
  );
  const catalog = useQuery({
    queryKey: settingsPricingCatalogQueryKey(filter),
    queryFn: () => getSettingsPricingCatalog(filter)
  });
  const importing = useMutation({
    mutationFn: () => {
      if (!csrf) throw new Error('missing csrf token');
      return importSettingsPricingCatalog(
        (catalog.data?.items ?? []).map((rule) => rule.id),
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
        key: 'rating_policy',
        title: i18nText('settings', 'auto.billing_rating_policy'),
        width: 180,
        render: (_: unknown, row: PricingCatalogRule) =>
          row.rating_policy_enabled ? (
            <Tag color="blue">
              {row.rating_policy.type === 'input_token_tiers'
                ? i18nText('settings', 'auto.billing_input_token_tiers')
                : i18nText('settings', 'auto.enabled')}
            </Tag>
          ) : (
            i18nText('settings', 'auto.billing_no_rating_policy')
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
  const rows = catalog.data?.items ?? [];

  function applyFilters() {
    setPage(1);
    setAppliedFilters({
      provider_code: providerCode.trim() || undefined,
      upstream_model_id: upstreamModelId.trim() || undefined
    });
  }

  function resetFilters() {
    setProviderCode('');
    setUpstreamModelId('');
    setPage(1);
    setAppliedFilters({
      provider_code: undefined,
      upstream_model_id: undefined
    });
  }

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
              key: 'ui-components',
              label: i18nText('settings', 'auto.ui_components')
            },
            {
              key: 'model-pricing',
              label: i18nText('settings', 'auto.billing_vendor_model_pricing')
            }
          ]}
        />
        <DataTableLayout
          filters={
            <DataTableFilterForm
              ariaLabel={i18nText(
                'settings',
                'auto.translation_catalog_filter'
              )}
              resetLabel={i18nText('settings', 'auto.reset')}
              submitLabel={i18nText(
                'settings',
                'auto.translation_catalog_filter'
              )}
              onReset={resetFilters}
              onSubmit={applyFilters}
            >
              <DataTableFilterField
                label={i18nText('settings', 'auto.billing_provider_code')}
              >
                <Input
                  aria-label={i18nText(
                    'settings',
                    'auto.billing_provider_code'
                  )}
                  value={providerCode}
                  onChange={(event) => setProviderCode(event.target.value)}
                />
              </DataTableFilterField>
              <DataTableFilterField
                label={i18nText('settings', 'auto.billing_model_id')}
              >
                <Input
                  aria-label={i18nText('settings', 'auto.billing_model_id')}
                  value={upstreamModelId}
                  onChange={(event) => setUpstreamModelId(event.target.value)}
                />
              </DataTableFilterField>
            </DataTableFilterForm>
          }
        >
          {catalog.isError ? (
            <Alert
              showIcon
              type="error"
              message={i18nText(
                'settingsBilling',
                'auto.billing_remote_catalog_unavailable'
              )}
            />
          ) : null}
          {importing.data ? (
            <Alert
              showIcon
              type="success"
              message={i18nText(
                'settingsBilling',
                'auto.billing_catalog_install_complete'
              )}
              description={`${i18nText(
                'settingsBilling',
                'auto.billing_catalog_inserted'
              )}: ${importing.data.inserted}; ${i18nText(
                'settingsBilling',
                'auto.billing_catalog_skipped'
              )}: ${importing.data.skipped}`}
            />
          ) : null}
          <DataTable<PricingCatalogRule>
            columns={columns}
            configuration={tableConfiguration}
            dataSource={rows}
            emptyText={
              <Empty
                description={i18nText('settings', 'auto.billing_catalog_empty')}
              />
            }
            loading={catalog.isLoading || catalog.isFetching}
            page={page}
            pageSize={PAGE_SIZE}
            rowKey="id"
            toolbar={
              <Flex justify="flex-end" gap={8} wrap>
                <Button
                  loading={catalog.isFetching}
                  onClick={() => {
                    importing.reset();
                    void catalog.refetch();
                  }}
                >
                  {i18nText(
                    'settingsBilling',
                    'auto.billing_refresh_remote_catalog'
                  )}
                </Button>
                <Button
                  type="primary"
                  disabled={rows.length === 0}
                  loading={importing.isPending}
                  onClick={() => importing.mutate()}
                >
                  {i18nText(
                    'settingsBilling',
                    'auto.billing_install_catalog_page'
                  )}
                </Button>
                <DataTableColumnSettings
                  columns={columns}
                  configuration={tableConfiguration}
                />
              </Flex>
            }
            total={catalog.data?.total_count ?? 0}
            onPageChange={setPage}
          />
        </DataTableLayout>
      </section>
    </SettingsSectionSurface>
  );
}
