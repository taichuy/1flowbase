import { lazy, Suspense } from 'react';
import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';

import { SettingsModelProvidersSection } from './SettingsModelProvidersSection';
import { i18nText } from '../../../../shared/i18n/text';
import './model-provider-settings-tabs.css';

const PricingRulesPanel = lazy(() =>
  import('../../components/billing/PricingRulesPanel').then(
    ({ PricingRulesPanel }) => ({ default: PricingRulesPanel })
  )
);
const ModelProviderRequestLogsPanel = lazy(() =>
  import('../../components/model-provider-request-logs/ModelProviderRequestLogsPanel').then(
    ({ ModelProviderRequestLogsPanel }) => ({ default: ModelProviderRequestLogsPanel })
  )
);

export type ModelProviderSettingsTab = 'providers' | 'pricing' | 'request-logs';

export function ModelProviderSettingsTabs({
  activeTab
}: {
  activeTab: ModelProviderSettingsTab;
  canManage?: boolean;
}) {
  const navigate = useNavigate();
  return (
    <Tabs
      className="model-provider-settings-tabs"
      activeKey={activeTab}
      onChange={(key) => navigate({ to: `/settings/model-providers/${key}` })}
      items={[
        {
          key: 'providers',
          label: i18nText('settings', 'auto.model_providers'),
          children: activeTab === 'providers' ? <SettingsModelProvidersSection /> : null
        },
        {
          key: 'pricing',
          label: i18nText('settings', 'auto.billing_pricing_rules'),
          children: activeTab === 'pricing' ? (
            <Suspense fallback={<LoadingState compact />}>
              <PricingRulesPanel />
            </Suspense>
          ) : null
        },
        {
          key: 'request-logs',
          label: i18nText('settings', 'auto.model_provider_tab_request_logs'),
          children: activeTab === 'request-logs' ? (
            <Suspense fallback={<LoadingState compact />}>
              <ModelProviderRequestLogsPanel />
            </Suspense>
          ) : null
        }
      ]}
    />
  );
}
