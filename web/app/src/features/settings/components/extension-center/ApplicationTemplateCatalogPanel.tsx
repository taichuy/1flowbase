import type { ApplicationTemplateCatalogItem } from '@1flowbase/api-client';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Empty, Flex, Table, Tabs, Typography } from 'antd';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { useSystemTemplates } from '../../api/system-templates/useSystemTemplates';
import { SettingsSectionSurface } from '../SettingsSectionSurface';
import { SystemTemplateDialog } from '../system-templates/SystemTemplateDialog';

export function ApplicationTemplateCatalogPanel() {
  const navigate = useNavigate();
  const { t } = useTranslation('settingsSystemTemplates');
  const { t: settingsT } = useTranslation('settings');
  const canManage = useAuthStore(
    (state) =>
      state.actor?.effective_display_role === 'root' ||
      (state.me?.permissions.includes(
        'settings_feature.access.system.backups'
      ) ??
        false)
  );
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const { catalog } = useSystemTemplates(canManage);
  const [selected, setSelected] = useState<{ name: string; body: unknown }>();

  return (
    <SettingsSectionSurface heightMode="fill">
      <Flex vertical gap={16}>
        <Tabs
          activeKey="application-templates"
          onChange={(category) =>
            void navigate({
              to: '/settings/extension-center/$category',
              params: { category },
              search: { q: undefined, cursor: undefined }
            })
          }
          items={[
            { key: 'installed', label: settingsT('auto.installed_extensions') },
            {
              key: 'application-templates',
              label: settingsT('auto.application_templates')
            },
            { key: 'agent-flow', label: 'agent-flow' },
            { key: 'capability-plugins', label: 'capability-plugins' },
            { key: 'host-extensions', label: 'host-extensions' },
            { key: 'i18n', label: 'i18n' },
            { key: 'mcp', label: 'mcp' },
            { key: 'runtime-extensions', label: 'runtime-extensions' },
            { key: 'ui-components', label: settingsT('auto.ui_components') },
            {
              key: 'model-pricing',
              label: settingsT('auto.billing_vendor_model_pricing')
            }
          ]}
        />
        <Typography.Title level={4}>{t('catalog_title')}</Typography.Title>
        {!canManage ? (
          <Alert type="warning" showIcon title={t('access_denied')} />
        ) : (
          <>
            {catalog.isError && (
              <Alert
                type="error"
                showIcon
                title={t('catalog_load_failed')}
                action={
                  <Button onClick={() => void catalog.refetch()}>
                    {t('catalog_retry')}
                  </Button>
                }
              />
            )}
            <Table<ApplicationTemplateCatalogItem>
              rowKey="template_id"
              loading={catalog.isLoading}
              dataSource={catalog.data?.application_templates}
              pagination={false}
              scroll={{ x: 640 }}
              locale={{ emptyText: <Empty description={t('catalog_empty')} /> }}
              columns={[
                {
                  title: t('template_name'),
                  dataIndex: 'name',
                  render: (name: string, item) => (
                    <Flex vertical>
                      <Typography.Text strong>{name}</Typography.Text>
                      <Typography.Text type="secondary">
                        {item.description}
                      </Typography.Text>
                    </Flex>
                  )
                },
                { title: t('version'), dataIndex: 'release_version' },
                {
                  title: t('installed_version'),
                  dataIndex: 'installed_release_version',
                  render: (version: number | null) =>
                    version ?? t('not_installed')
                },
                {
                  title: t('action'),
                  key: 'action',
                  render: (_, item) => (
                    <Button
                      disabled={!csrfToken}
                      onClick={() =>
                        setSelected({ name: item.name, body: item.package })
                      }
                    >
                      {t('preview_action')}
                    </Button>
                  )
                }
              ]}
            />
          </>
        )}
      </Flex>
      {selected && canManage && (
        <SystemTemplateDialog
          mode="import"
          template={selected}
          canInstall={canManage && Boolean(csrfToken)}
          onClose={() => setSelected(undefined)}
        />
      )}
    </SettingsSectionSurface>
  );
}
