import { Alert, Typography } from 'antd';

import { i18nText } from '../../../../shared/i18n/text';
import type {
  SettingsOfficialPluginCatalogEntry,
  SettingsPluginFamilyEntry
} from '../../api/plugins';

export function OfficialPluginInstallConfirmContent({
  entry,
  family,
  belowMinimumHostVersion
}: {
  entry: SettingsOfficialPluginCatalogEntry;
  family: SettingsPluginFamilyEntry | undefined;
  belowMinimumHostVersion: boolean;
}) {
  return (
    <div className="model-provider-panel__install-confirm">
      <div className="model-provider-panel__install-confirm-card">
        <Typography.Title level={5}>{entry.display_name}</Typography.Title>
        <Typography.Paragraph type="secondary">
          {family
            ? i18nText(
                'settings',
                'auto.workspace_s_upgraded_latest_official_version_completion_all_instances_supplier',
                {
                  value1: entry.display_name,
                  value2: entry.latest_version
                }
              )
            : i18nText(
                'settings',
                'auto.latest_official_version_about_installed_completion_automatically_enabled_workspace',
                { value1: entry.latest_version }
              )}
        </Typography.Paragraph>
        <div className="model-provider-panel__catalog-item-meta">
          <span>
            {i18nText('settings', 'auto.agreement')}
            {entry.protocol}
          </span>
          <span>
            {i18nText('settings', 'auto.discovery_mode')}
            {entry.model_discovery_mode}
          </span>
        </div>
        {belowMinimumHostVersion ? (
          <Alert
            type="warning"
            showIcon
            title={i18nText(
              'settings',
              'auto.host_version_below_minimum_warning'
            )}
            description={
              <div className="model-provider-panel__install-warning-detail">
                <Typography.Text>
                  {i18nText('settings', 'auto.current_host_version_value', {
                    value1: entry.current_host_version
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText('settings', 'auto.minimum_host_version_value', {
                    value1: entry.minimum_host_version
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText('settings', 'auto.plugin_version_value', {
                    value1: entry.latest_version
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText('settings', 'auto.possible_risk_value', {
                    value1: i18nText(
                      'settings',
                      'auto.host_version_below_minimum_risk'
                    )
                  })}
                </Typography.Text>
                <Typography.Text>
                  {i18nText(
                    'settings',
                    'auto.upgrade_one_flowbase_before_continuing'
                  )}
                </Typography.Text>
              </div>
            }
          />
        ) : null}
      </div>
    </div>
  );
}
