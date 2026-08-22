import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';

import { NetworkEgressPoolsPanel } from '../../network-center/pools/NetworkEgressPoolsPanel';
import { NetworkEgressProvidersPanel } from '../../network-center/providers/NetworkEgressProvidersPanel';
import { NetworkEgressRoutesPanel } from '../../network-center/routes/NetworkEgressRoutesPanel';
import { i18nText } from '../../../../shared/i18n/text';
import './network-center-section.css';

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
          children: <NetworkEgressProvidersPanel />
        },
        {
          key: 'proxy-pools',
          label: i18nText('settings', 'auto.network_center_pools'),
          children: <NetworkEgressPoolsPanel />
        },
        {
          key: 'routing-rules',
          label: i18nText('settings', 'auto.network_center_routes'),
          children: <NetworkEgressRoutesPanel />
        }
      ]}
    />
  );
}
