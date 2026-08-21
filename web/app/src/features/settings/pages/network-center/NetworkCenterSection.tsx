import { useQuery } from '@tanstack/react-query';

import {
  fetchSettingsNetworkEgressProviders,
  settingsNetworkEgressProvidersQueryKey
} from '../../api/network-center';
import { NetworkEgressPoolsPanel } from '../../network-center/pools/NetworkEgressPoolsPanel';
import { NetworkEgressProvidersPanel } from '../../network-center/providers/NetworkEgressProvidersPanel';
import { NetworkEgressRoutesPanel } from '../../network-center/routes/NetworkEgressRoutesPanel';

export type NetworkCenterPage = 'providers' | 'pools' | 'routes';

export function NetworkCenterSection({ page }: { page: NetworkCenterPage }) {
  const providersQuery = useQuery({
    queryKey: settingsNetworkEgressProvidersQueryKey,
    queryFn: fetchSettingsNetworkEgressProviders,
    enabled: page === 'pools'
  });

  switch (page) {
    case 'pools':
      return <NetworkEgressPoolsPanel providers={providersQuery.data ?? []} />;
    case 'routes':
      return <NetworkEgressRoutesPanel />;
    case 'providers':
    default:
      return <NetworkEgressProvidersPanel />;
  }
}
