import { useState } from 'react';

import { Alert, Button, Empty, Input, Modal, Popconfirm, Select, Space, Table, Tag, Typography, Upload } from 'antd';
import type { UploadFile } from 'antd/es/upload/interface';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import {
  fetchSettingsNetworkEgressOfficialPluginCatalog,
  fetchSettingsNetworkEgressPluginFamilies,
  fetchSettingsNetworkEgressProviderTypes,
  installSettingsNetworkEgressOfficialPlugin,
  settingsNetworkEgressPluginFamiliesQueryKey,
  settingsNetworkEgressOfficialPluginsQueryKey,
  settingsNetworkEgressProviderTypesQueryKey,
  type SettingsNetworkEgressProviderType,
  switchSettingsNetworkEgressPluginVersion,
  uninstallSettingsNetworkEgressPluginFamily,
  uploadSettingsNetworkEgressPluginPackage
} from '../../api/network-center';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';
import { useAuthStore } from '../../../../state/auth-store';
import { i18nText } from '../../../../shared/i18n/text';
import { FALLBACK_APP_LOCALE, toAppLocale } from '../../../../shared/i18n/locales';
import './network-egress-providers.css';

const OFFICIAL_PLUGIN_RELEASES_URL = 'https://github.com/taichuy/1flowbase-official-plugins/releases';

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : null;
}

/** The catalog describes parsers available to proxy creation; it never owns instances. */
export function NetworkEgressProvidersPanel() {
  const { i18n } = useTranslation();
  const queryClient = useQueryClient();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const [search, setSearch] = useState('');
  const [uploadOpen, setUploadOpen] = useState(false);
  const [uploadFiles, setUploadFiles] = useState<UploadFile[]>([]);
  const appLocale =
    toAppLocale(i18n.resolvedLanguage) ??
    toAppLocale(i18n.language) ??
    FALLBACK_APP_LOCALE;
  const types = useQuery({
    queryKey: settingsNetworkEgressProviderTypesQueryKey,
    queryFn: fetchSettingsNetworkEgressProviderTypes
  });
  const plugins = useQuery({
    queryKey: [
      ...settingsNetworkEgressOfficialPluginsQueryKey,
      appLocale,
      search
    ],
    queryFn: () => fetchSettingsNetworkEgressOfficialPluginCatalog({
      locale: appLocale,
      q: search || undefined
    })
  });
  const pluginFamilies = useQuery({
    queryKey: settingsNetworkEgressPluginFamiliesQueryKey,
    queryFn: fetchSettingsNetworkEgressPluginFamilies
  });
  const refreshTypes = () => Promise.all([
    queryClient.invalidateQueries({ queryKey: settingsNetworkEgressProviderTypesQueryKey }),
    queryClient.invalidateQueries({ queryKey: settingsNetworkEgressOfficialPluginsQueryKey }),
    queryClient.invalidateQueries({ queryKey: settingsNetworkEgressPluginFamiliesQueryKey })
  ]);
  const install = useMutation({
    mutationFn: (pluginId: string) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return installSettingsNetworkEgressOfficialPlugin(pluginId, csrfToken);
    },
    onSuccess: refreshTypes
  });
  const upload = useMutation({
    mutationFn: (file: File) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return uploadSettingsNetworkEgressPluginPackage(file, csrfToken);
    },
    onSuccess: async () => {
      setUploadFiles([]);
      setUploadOpen(false);
      await refreshTypes();
    }
  });
  const switchVersion = useMutation({
    mutationFn: ({ providerCode, installationId }: { providerCode: string; installationId: string }) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return switchSettingsNetworkEgressPluginVersion(providerCode, installationId, csrfToken);
    },
    onSuccess: refreshTypes
  });
  const uninstallFamily = useMutation({
    mutationFn: (providerCode: string) => {
      if (!csrfToken) throw new Error('Missing CSRF token');
      return uninstallSettingsNetworkEgressPluginFamily(providerCode, csrfToken);
    },
    onSuccess: refreshTypes
  });

  const pluginError = errorMessage(plugins.error) ?? errorMessage(pluginFamilies.error) ?? errorMessage(install.error) ?? errorMessage(upload.error) ?? errorMessage(switchVersion.error) ?? errorMessage(uninstallFamily.error);
  const familyByProviderCode = new Map((pluginFamilies.data ?? []).map((family) => [family.provider_code, family]));
  const officialPluginByProviderCode = new Map((plugins.data?.entries ?? []).map((plugin) => [plugin.provider_code, plugin]));

  return (
    <SettingsSectionSurface heightMode="fill">
      <div className="network-egress-providers">
        <div className="network-egress-providers__types">
          {types.isError ? (
            <Alert type="error" showIcon title={i18nText('settings', 'auto.network_center_providers_load_failed')} />
          ) : (
            <Table<SettingsNetworkEgressProviderType>
              data-testid="network-center-providers-shell"
              rowKey="provider_code"
              loading={types.isLoading}
              dataSource={types.data ?? []}
              pagination={false}
              locale={{ emptyText: <Empty description={i18nText('settings', 'auto.network_center_no_providers')} /> }}
              columns={[
                { title: i18nText('settings', 'auto.network_center_providers'), dataIndex: 'display_name', key: 'display_name' },
                { title: i18nText('settings', 'auto.network_center_proxy_type_fields'), key: 'fields', render: (_, item) => item.form_schema.fields.map((field) => <Tag key={field.key}>{field.label}</Tag>) },
                {
                  title: i18nText('settings', 'auto.version'),
                  key: 'version',
                  render: (_, item) => {
                    const family = familyByProviderCode.get(item.provider_code);
                    if (!family) return <Typography.Text type="secondary">—</Typography.Text>;
                    return <Select
                      size="small"
                      aria-label={`${item.display_name} ${i18nText('settings', 'auto.version')}`}
                      value={family.current_installation_id}
                      loading={switchVersion.isPending && switchVersion.variables?.providerCode === family.provider_code}
                      options={family.installed_versions.map((version) => ({ value: version.installation_id, label: version.plugin_version }))}
                      onChange={(installationId) => switchVersion.mutate({ providerCode: family.provider_code, installationId })}
                    />;
                  }
                },
                {
                  title: i18nText('settings', 'auto.operation'),
                  key: 'actions',
                  render: (_, item) => {
                    const family = familyByProviderCode.get(item.provider_code);
                    const officialPlugin = officialPluginByProviderCode.get(item.provider_code);
                    if (!family) return <Typography.Text type="secondary">—</Typography.Text>;
                    return <Space size={4} wrap>
                      {officialPlugin?.has_update ? <Button
                        type="link"
                        size="small"
                        aria-label={`${item.display_name} ${i18nText('settings', 'auto.update')}`}
                        loading={install.isPending && install.variables === officialPlugin.plugin_id}
                        onClick={() => install.mutate(officialPlugin.plugin_id)}
                      >
                        {i18nText('settings', 'auto.update')}
                      </Button> : null}
                      <Popconfirm
                        title={i18nText('settings', 'auto.uninstall_plugin')}
                        onConfirm={() => uninstallFamily.mutate(family.provider_code)}
                        okButtonProps={{ danger: true, loading: uninstallFamily.isPending && uninstallFamily.variables === family.provider_code }}
                      >
                        <Button
                          danger
                          type="link"
                          size="small"
                          aria-label={`${item.display_name} ${i18nText('settings', 'auto.uninstall_plugin')}`}
                          disabled={!family.can_uninstall}
                        >
                          {i18nText('settings', 'auto.uninstall_plugin')}
                        </Button>
                      </Popconfirm>
                    </Space>;
                  }
                }
              ]}
            />
          )}
        </div>
        <aside className="network-egress-providers__plugins" aria-label={i18nText('settings', 'auto.network_center_proxy_plugins')}>
          <div className="network-egress-providers__plugin-heading">
            <Typography.Title level={5}>{i18nText('settings', 'auto.network_center_proxy_plugins')}</Typography.Title>
            <Button onClick={() => setUploadOpen(true)}>{i18nText('settings', 'auto.upload_plugin')}</Button>
          </div>
          <Input.Search value={search} onChange={(event) => setSearch(event.target.value)} placeholder={i18nText('settings', 'auto.network_center_proxy_plugins_search')} />
          {pluginError ? <Alert type="error" showIcon title={pluginError} /> : null}
          {plugins.data?.entries.length === 0 && !plugins.isLoading ? <Empty description={i18nText('settings', 'auto.network_center_no_proxy_plugins')} /> : null}
          <div className="network-egress-providers__plugin-list">
            {(plugins.data?.entries ?? []).map((plugin) => (
              <article className="network-egress-providers__plugin" key={plugin.plugin_id}>
                <div className="network-egress-providers__plugin-title">
                  <Typography.Text strong>{plugin.display_name}</Typography.Text>
                  <Tag>{plugin.latest_version}</Tag>
                </div>
                {plugin.description ? <Typography.Text type="secondary">{plugin.description}</Typography.Text> : null}
                <div className="network-egress-providers__plugin-actions">
                  {plugin.help_url ? <Button onClick={() => window.open(plugin.help_url!, '_blank', 'noopener,noreferrer')}>{i18nText('settings', 'auto.documentation')}</Button> : null}
                  <Button type="primary" loading={install.isPending && install.variables === plugin.plugin_id} disabled={plugin.install_status === 'installed'} onClick={() => install.mutate(plugin.plugin_id)}>
                    {plugin.install_status === 'installed'
                      ? i18nText('settings', 'auto.network_center_proxy_plugin_installed')
                      : i18nText('settings', 'auto.install_plugin')}
                  </Button>
                </div>
              </article>
            ))}
          </div>
          <Button type="link" onClick={() => window.open(plugins.data?.registry_url ?? OFFICIAL_PLUGIN_RELEASES_URL, '_blank', 'noopener,noreferrer')}>
            {i18nText('settings', 'auto.go_warehouse_download')}
          </Button>
        </aside>
      </div>
      <Modal open={uploadOpen} title={i18nText('settings', 'auto.upload_plugin')} onCancel={() => setUploadOpen(false)} onOk={() => {
        const file = uploadFiles[0]?.originFileObj;
        if (file instanceof File) upload.mutate(file);
      }} confirmLoading={upload.isPending}>
        <Upload.Dragger beforeUpload={() => false} maxCount={1} fileList={uploadFiles} onChange={({ fileList }) => setUploadFiles(fileList.slice(-1))}>
          {i18nText('settings', 'auto.select_plug_package_upload_install')}
        </Upload.Dragger>
      </Modal>
    </SettingsSectionSurface>
  );
}
