import { useNavigate } from '@tanstack/react-router';
import { Flex, Tabs, Typography } from 'antd';
import { useTranslation } from 'react-i18next';

import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { UiComponentCatalogContent } from '../ui-management/UiComponentCatalogContent';

export function UiComponentCatalogPanel({ canManage }: { canManage: boolean }) {
  const navigate = useNavigate();
  const { t: settingsT } = useTranslation('settings');
  const { t } = useTranslation('settingsUiManagement');

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey="ui-components"
          tabBarExtraContent={
            <Typography.Link href="/settings/ui-management/components">
              {t('go_to_ui_management')}
            </Typography.Link>
          }
          onChange={(category) =>
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category },
              search: { q: undefined, cursor: undefined }
            })
          }
          items={[
            {
              key: 'installed',
              label: settingsT('auto.installed_extensions')
            },
            { key: 'agent-flow', label: 'agent-flow' },
            { key: 'capability-plugins', label: 'capability-plugins' },
            { key: 'host-extensions', label: 'host-extensions' },
            { key: 'i18n', label: 'i18n' },
            { key: 'mcp', label: 'mcp' },
            { key: 'runtime-extensions', label: 'runtime-extensions' },
            { key: 'ui-components', label: t('extension_center_tab') },
            {
              key: 'model-pricing',
              label: settingsT('auto.billing_vendor_model_pricing')
            }
          ]}
        />
        <UiComponentCatalogContent canManage={canManage} />
      </Flex>
    </SettingsSectionSurface>
  );
}
