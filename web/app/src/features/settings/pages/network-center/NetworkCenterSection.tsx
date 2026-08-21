import { useQuery } from '@tanstack/react-query';
import { Alert, Descriptions, Empty, Flex, Table, Tag, Typography } from 'antd';

import {
  fetchSettingsNetworkEgressProviders,
  fetchSettingsNetworkEgressPools,
  fetchSettingsNetworkEgressRoutes,
  settingsNetworkEgressPoolsQueryKey,
  settingsNetworkEgressProvidersQueryKey,
  settingsNetworkEgressRoutesQueryKey
} from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { i18nText } from '../../../../shared/i18n/text';
import { NetworkEgressPoolsPanel } from '../../network-center/pools/NetworkEgressPoolsPanel';
import { NetworkEgressRoutesPanel } from '../../network-center/routes/NetworkEgressRoutesPanel';

export type NetworkCenterPage = 'providers' | 'pools' | 'routes';

function ProviderRegistrySummary({
  providerCount,
  isLoading
}: {
  providerCount: number;
  isLoading: boolean;
}) {
  return (
    <Descriptions
      size="small"
      column={1}
      items={[
        {
          key: 'provider-count',
          label: i18nText(
            'settings',
            'auto.network_center_registered_providers'
          ),
          children: isLoading
            ? i18nText('settings', 'auto.loading')
            : providerCount
        }
      ]}
    />
  );
}

function NetworkCenterProvidersShell({
  providerCount,
  isLoading,
  isError
}: {
  providerCount: number;
  isLoading: boolean;
  isError: boolean;
}) {
  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Typography.Title
          level={2}
          data-testid="network-center-providers-shell"
        >
          {i18nText('settings', 'auto.network_center_providers')}
        </Typography.Title>
        {isError ? (
          <Alert
            type="error"
            showIcon
            title={i18nText(
              'settings',
              'auto.network_center_providers_load_failed'
            )}
          />
        ) : (
          <ProviderRegistrySummary
            providerCount={providerCount}
            isLoading={isLoading}
          />
        )}
        {!isLoading && !isError && providerCount === 0 ? (
          <Empty
            description={i18nText(
              'settings',
              'auto.network_center_no_providers'
            )}
          />
        ) : null}
      </Flex>
    </SettingsSectionSurface>
  );
}

function NetworkCenterRoutesShell() {
  const routesQuery = useQuery({
    queryKey: settingsNetworkEgressRoutesQueryKey,
    queryFn: fetchSettingsNetworkEgressRoutes
  });
  const poolsQuery = useQuery({
    queryKey: settingsNetworkEgressPoolsQueryKey,
    queryFn: fetchSettingsNetworkEgressPools
  });
  const poolNames = new Map(
    (poolsQuery.data ?? []).map((pool) => [pool.id, pool.display_name])
  );
  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Typography.Title level={2} data-testid="network-center-routes-shell">
          {i18nText('settings', 'auto.network_center_routes')}
        </Typography.Title>
        {routesQuery.isError ? (
          <Alert
            type="error"
            showIcon
            title={i18nText(
              'settings',
              'auto.network_center_pools_load_failed'
            )}
          />
        ) : (
          <Table
            rowKey="id"
            loading={routesQuery.isLoading || poolsQuery.isLoading}
            dataSource={routesQuery.data ?? []}
            pagination={false}
            locale={{ emptyText: <Empty /> }}
            columns={[
              {
                title: i18nText(
                  'settings',
                  'auto.network_center_route_consumer'
                ),
                dataIndex: 'consumer_kind'
              },
              {
                title: i18nText('settings', 'auto.network_center_route_pool'),
                dataIndex: 'pool_id',
                render: (poolId: string) => poolNames.get(poolId) ?? poolId
              },
              {
                title: i18nText('settings', 'auto.network_center_route_status'),
                dataIndex: 'enabled',
                render: (enabled: boolean) => (
                  <Tag color={enabled ? 'green' : undefined}>
                    {i18nText(
                      'settings',
                      enabled ? 'auto.enabled' : 'auto.disabled'
                    )}
                  </Tag>
                )
              },
              {
                title: i18nText(
                  'settings',
                  'auto.network_center_route_failure_policy'
                ),
                dataIndex: 'failure_policy'
              }
            ]}
          />
        )}
      </Flex>
    </SettingsSectionSurface>
  );
}

export function NetworkCenterSection({ page }: { page: NetworkCenterPage }) {
  const providersQuery = useQuery({
    queryKey: settingsNetworkEgressProvidersQueryKey,
    queryFn: fetchSettingsNetworkEgressProviders
  });
  const providerCount = providersQuery.data?.length ?? 0;

  switch (page) {
    case 'pools':
      return <NetworkEgressPoolsPanel providers={providersQuery.data ?? []} />;
    case 'routes':
      return <NetworkEgressRoutesPanel />;
    case 'providers':
    default:
      return (
        <NetworkCenterProvidersShell
          providerCount={providerCount}
          isLoading={providersQuery.isLoading}
          isError={providersQuery.isError}
        />
      );
  }
}
