import { useQuery } from '@tanstack/react-query';
import { Alert, Spin } from 'antd';

import { McpManagementPanel } from '../../components/mcp-management/McpManagementPanel';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import {
  fetchSettingsMcpCatalog,
  fetchSettingsMcpInterfaceCapabilities,
  settingsMcpCatalogQueryKey,
  settingsMcpInterfaceCapabilitiesQueryKey
} from '../../api/mcp-management';
import { i18nText } from '../../../../shared/i18n/text';

export function SettingsMcpManagementSection({
  canManage
}: {
  canManage: boolean;
}) {
  const catalogQuery = useQuery({
    queryKey: settingsMcpCatalogQueryKey,
    queryFn: fetchSettingsMcpCatalog
  });
  const interfaceQuery = useQuery({
    queryKey: settingsMcpInterfaceCapabilitiesQueryKey,
    queryFn: fetchSettingsMcpInterfaceCapabilities
  });

  if (catalogQuery.isLoading || interfaceQuery.isLoading) {
    return (
      <SettingsSectionSurface
        heightMode="fill"
      >
        <Spin />
      </SettingsSectionSurface>
    );
  }

  if (
    catalogQuery.isError ||
    interfaceQuery.isError ||
    !catalogQuery.data ||
    !interfaceQuery.data
  ) {
    return (
      <SettingsSectionSurface
        heightMode="fill"
      >
        <Alert
          type="error"
          title={i18nText('settings', 'auto.mcp_load_failed')}
        />
      </SettingsSectionSurface>
    );
  }

  return (
    <SettingsSectionSurface
      heightMode="fill"
    >
      <McpManagementPanel
        canManage={canManage}
        catalog={catalogQuery.data}
        interfaceCapabilities={interfaceQuery.data}
      />
    </SettingsSectionSurface>
  );
}
