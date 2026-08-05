import { Alert, Space } from 'antd';
import { useQuery } from '@tanstack/react-query';

import {
  fetchSettingsHostInfrastructureProviders,
  settingsHostInfrastructureProvidersQueryKey
} from '../../api/host-infrastructure';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { HostInfrastructureProviderTable } from './HostInfrastructureProviderTable';
import './host-infrastructure-panel.css';
import { i18nText } from '../../../../shared/i18n/text';

export function HostInfrastructurePanel({ canManage }: { canManage: boolean }) {
  const providersQuery = useQuery({
    queryKey: settingsHostInfrastructureProvidersQueryKey,
    queryFn: fetchSettingsHostInfrastructureProviders
  });

  return (
    <SettingsSectionSurface heightMode="fill">
      <Space
        orientation="vertical"
        size={16}
        className="host-infrastructure-panel"
      >
        <Alert
          type="info"
          showIcon
          title={i18nText("settings", "auto.installation_configuration_activation_saved_pending_changes_take_effect_restarting_api")}
        />
        <HostInfrastructureProviderTable
          providers={providersQuery.data ?? []}
          loading={providersQuery.isLoading}
          canManage={canManage}
        />
      </Space>
    </SettingsSectionSurface>
  );
}
