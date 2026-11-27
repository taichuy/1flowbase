import { McpManagementPanel } from '../../components/mcp-management/McpManagementPanel';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

export function SettingsMcpManagementSection() {
  return (
    <SettingsSectionSurface heightMode="fill">
      <McpManagementPanel />
    </SettingsSectionSurface>
  );
}
