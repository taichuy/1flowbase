import { Tabs } from 'antd';
import { useNavigate } from '@tanstack/react-router';

import { ModelProviderRequestLogsPanel } from '../../components/model-provider-request-logs/ModelProviderRequestLogsPanel';
import { SettingsModelProvidersSection } from './SettingsModelProvidersSection';
import { i18nText } from '../../../../shared/i18n/text';

export type ModelProviderSettingsTab = 'providers' | 'request-logs';

export function ModelProviderSettingsTabs({
  activeTab,
  canManage
}: {
  activeTab: ModelProviderSettingsTab;
  canManage: boolean;
}) {
  const navigate = useNavigate();
  return (
    <Tabs
      activeKey={activeTab}
      onChange={(key) => navigate({ to: `/settings/model-providers/${key}` })}
      items={[
        {
          key: 'providers',
          label: i18nText('settings', 'auto.model_providers'),
          children: <SettingsModelProvidersSection canManage={canManage} />
        },
        {
          key: 'request-logs',
          label: i18nText('settings', 'auto.model_provider_tab_request_logs'),
          children: <ModelProviderRequestLogsPanel />
        }
      ]}
    />
  );
}
