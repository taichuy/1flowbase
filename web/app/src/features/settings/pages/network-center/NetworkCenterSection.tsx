import { lazy, Suspense } from 'react';
import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';

import { NetworkEgressProvidersPanel } from '../../network-center/providers/NetworkEgressProvidersPanel';
import { i18nText } from '../../../../shared/i18n/text';
import './network-center-section.css';

const NetworkEgressPoolsPanel = lazy(() =>
  import('../../network-center/pools/NetworkEgressPoolsPanel').then(
    ({ NetworkEgressPoolsPanel }) => ({ default: NetworkEgressPoolsPanel })
  )
);
const NetworkEgressRoutesPanel = lazy(() =>
  import('../../network-center/routes/NetworkEgressRoutesPanel').then(
    ({ NetworkEgressRoutesPanel }) => ({ default: NetworkEgressRoutesPanel })
  )
);

export type NetworkCenterPage =
  | 'proxy-types'
  | 'proxy-pools'
  | 'routing-rules';

export function NetworkCenterSection({ page }: { page: NetworkCenterPage }) {
  const navigate = useNavigate();
  return (
    <Tabs
      className="network-center-section"
      activeKey={page}
      onChange={(key) => navigate({ to: `/settings/network-center/${key}` })}
      items={[
        {
          key: 'proxy-types',
          label: i18nText('settings', 'auto.network_center_providers'),
          children: page === 'proxy-types' ? <NetworkEgressProvidersPanel /> : null
        },
        {
          key: 'proxy-pools',
          label: i18nText('settings', 'auto.network_center_pools'),
          children: page === 'proxy-pools' ? (
            <Suspense fallback={<LoadingState compact />}>
              <NetworkEgressPoolsPanel />
            </Suspense>
          ) : null
        },
        {
          key: 'routing-rules',
          label: i18nText('settings', 'auto.network_center_routes'),
          children: page === 'routing-rules' ? (
            <Suspense fallback={<LoadingState compact />}>
              <NetworkEgressRoutesPanel />
            </Suspense>
          ) : null
        }
      ]}
    />
  );
}
