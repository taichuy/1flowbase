import { useQuery } from '@tanstack/react-query';
import { Alert, Descriptions, Empty, Flex, Result, Typography } from 'antd';

import {
  fetchSettingsNetworkEgressProviders,
  settingsNetworkEgressProvidersQueryKey
} from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { i18nText } from '../../../../shared/i18n/text';
import { NetworkEgressPoolsPanel } from '../../network-center/pools/NetworkEgressPoolsPanel';

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

function NetworkCenterRoutesShell({
  providerCount,
  isLoading
}: {
  providerCount: number;
  isLoading: boolean;
}) {
  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Typography.Title level={2} data-testid="network-center-routes-shell">
          {i18nText('settings', 'auto.network_center_routes')}
        </Typography.Title>
        <ProviderRegistrySummary
          providerCount={providerCount}
          isLoading={isLoading}
        />
        <Result
          status="info"
          title={i18nText('settings', 'auto.network_center_routes_unavailable')}
        />
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
      return (
        <NetworkCenterRoutesShell
          providerCount={providerCount}
          isLoading={providersQuery.isLoading}
        />
      );
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
