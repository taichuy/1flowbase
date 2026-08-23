import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render, screen, waitFor, within } from '@testing-library/react';
import { App } from 'antd';
import type { ReactNode } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const networkCenterApi = vi.hoisted(() => ({
  fetchSettingsNetworkEgressOfficialPluginCatalog: vi.fn(),
  fetchSettingsNetworkEgressPluginFamilies: vi.fn(),
  fetchSettingsNetworkEgressProviderTypes: vi.fn(),
  installSettingsNetworkEgressOfficialPlugin: vi.fn(),
  settingsNetworkEgressOfficialPluginsQueryKey: [
    'settings',
    'network-center',
    'proxy-plugins',
    'official-catalog'
  ],
  settingsNetworkEgressPluginFamiliesQueryKey: [
    'settings',
    'network-center',
    'proxy-plugins',
    'families'
  ],
  settingsNetworkEgressProviderTypesQueryKey: [
    'settings',
    'network-center',
    'provider-types'
  ],
  uploadSettingsNetworkEgressPluginPackage: vi.fn(),
  switchSettingsNetworkEgressPluginVersion: vi.fn(),
  uninstallSettingsNetworkEgressPluginFamily: vi.fn(),
  uninstallSettingsNetworkEgressPluginVersion: vi.fn()
}));
vi.mock('../../../api/network-center', () => networkCenterApi);

import { AppI18nProvider } from '../../../../../app/AppI18nProvider';
import { NetworkEgressProvidersPanel } from '../NetworkEgressProvidersPanel';

function renderPanel() {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const wrapper = ({ children }: { children: ReactNode }) => <AppI18nProvider><App><QueryClientProvider client={client}>{children}</QueryClientProvider></App></AppI18nProvider>;
  return render(<NetworkEgressProvidersPanel />, { wrapper });
}

describe('NetworkEgressProvidersPanel', () => {
  beforeEach(() => {
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockReset();
    networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog.mockReset();
    networkCenterApi.fetchSettingsNetworkEgressPluginFamilies.mockReset();
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([]);
    networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog.mockResolvedValue({
      source_kind: 'official_registry', source_label: 'official', registry_url: 'https://example.com/registry.json', source_freshness: 'fresh', entries: []
    });
    networkCenterApi.fetchSettingsNetworkEgressPluginFamilies.mockResolvedValue([]);
  });

  test('AC-GP03 is a read-only parser catalog containing built-in and installed types', async () => {
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      { installation_id: null, provider_code: 'builtin_static_http', display_name: 'HTTP proxy', form_schema: { schema_version: '1flowbase.plugin.form/v1', fields: [{ key: 'host', label: 'Hostname or IP', type: 'string' }] } },
      { installation_id: 'clash-installation', provider_code: 'clash-proxy', display_name: 'Clash / Mihomo Proxy', form_schema: { schema_version: '1flowbase.plugin.form/v1', fields: [{ key: 'subscription_url', label: 'Subscription URL', type: 'string' }] } }
    ]);
    renderPanel();
    expect(await screen.findByText('HTTP proxy')).toBeInTheDocument();
    expect(screen.getByText('Clash / Mihomo Proxy')).toBeInTheDocument();
    expect(screen.getByText('Subscription URL')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /添加代理类型|Add proxy type/ })).not.toBeInTheDocument();
  });

  test('AC-GP04 pairs the available proxy type table with the filtered proxy plugin catalog', async () => {
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      { installation_id: null, provider_code: 'builtin_static_http', display_name: 'HTTP proxy', form_schema: { schema_version: '1flowbase.plugin.form/v1', fields: [] } }
    ]);
    networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog.mockResolvedValue({
      source_kind: 'official_registry',
      source_label: 'official',
      registry_url: 'https://example.com/registry.json',
      source_freshness: 'fresh',
      entries: [{
        plugin_id: 'taichuy.clash-proxy',
        provider_code: 'clash-proxy',
        plugin_type: 'network_egress_provider',
        display_name: 'Clash / Mihomo Proxy',
        description: 'Parse a Clash subscription.',
        protocol: 'clash',
        latest_version: '0.1.0',
        selected_artifact: {},
        help_url: null,
        model_discovery_mode: 'static',
        install_status: 'not_installed',
        minimum_host_version: '0.1.0',
        current_host_version: '0.3.0',
        compatibility_status: 'compatible',
        compatibility_warning_reason: null
      }]
    });

    renderPanel();

    expect(await screen.findByText('Clash / Mihomo Proxy')).toBeInTheDocument();
    expect(screen.getByText(/代理插件|Proxy plugins/)).toBeInTheDocument();
    await waitFor(() => expect(networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog).toHaveBeenCalledWith(
      expect.objectContaining({ locale: 'zh_Hans' })
    ));
  });

  test('AC-NCP01 renders update when the installed current version is behind the official version', async () => {
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      { installation_id: 'clash-installation', provider_code: 'clash-proxy', display_name: 'Clash / Mihomo Proxy', form_schema: { schema_version: '1flowbase.plugin.form/v1', fields: [] } }
    ]);
    networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog.mockResolvedValue({
      source_kind: 'official_registry',
      source_label: 'official',
      registry_url: 'https://example.com/registry.json',
      source_freshness: 'fresh',
      entries: [{
        plugin_id: 'taichuy.clash-proxy',
        provider_code: 'clash-proxy',
        plugin_type: 'network_egress_provider',
        display_name: 'Clash / Mihomo Proxy',
        description: null,
        protocol: 'clash',
        current_version: '0.2.2',
        latest_version: '0.2.3',
        has_update: true,
        selected_artifact: {},
        help_url: null,
        model_discovery_mode: 'static',
        install_status: 'installed',
        minimum_host_version: '0.1.0',
        current_host_version: '0.3.0',
        compatibility_status: 'compatible',
        compatibility_warning_reason: null
      }]
    });
    networkCenterApi.fetchSettingsNetworkEgressPluginFamilies.mockResolvedValue([
      { provider_code: 'clash-proxy', display_name: 'Clash / Mihomo Proxy', current_installation_id: 'v022', current_version: '0.2.2', can_uninstall: true, installed_versions: [{ installation_id: 'v022', plugin_version: '0.2.2', is_current: true, can_uninstall: false }] }
    ]);

    renderPanel();

    expect(await screen.findByRole('button', { name: /更\s*新|update/i })).toBeEnabled();
  });

  test('AC-NCP02 manages an installed proxy plugin as a version family', async () => {
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      { installation_id: 'clash-installation', provider_code: 'clash-proxy', display_name: 'Clash / Mihomo Proxy', form_schema: { schema_version: '1flowbase.plugin.form/v1', fields: [] } }
    ]);
    networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog.mockResolvedValue({
      source_kind: 'official_registry', source_label: 'official', registry_url: 'https://example.com/registry.json', source_freshness: 'fresh', entries: [{
        plugin_id: 'taichuy.clash-proxy', provider_code: 'clash-proxy', plugin_type: 'network_egress_provider', display_name: 'Clash / Mihomo Proxy', description: null, protocol: 'clash', current_version: '0.2.3', latest_version: '0.2.3', has_update: false, selected_artifact: {}, help_url: null, model_discovery_mode: 'static', install_status: 'installed', minimum_host_version: '0.1.0', current_host_version: '0.3.0', compatibility_status: 'compatible', compatibility_warning_reason: null
      }]
    });
    networkCenterApi.fetchSettingsNetworkEgressPluginFamilies.mockResolvedValue([
      {
        provider_code: 'clash-proxy',
        display_name: 'Clash / Mihomo Proxy',
        current_installation_id: 'v023',
        current_version: '0.2.3',
        can_uninstall: true,
        installed_versions: [
          { installation_id: 'v023', plugin_version: '0.2.3', is_current: true, can_uninstall: false },
          { installation_id: 'v022', plugin_version: '0.2.2', is_current: false, can_uninstall: true }
        ]
      }
    ]);

    renderPanel();

    expect(await screen.findByLabelText(/Clash \/ Mihomo Proxy.*版本|Clash \/ Mihomo Proxy.*version/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /卸载.*0\.2\.2|uninstall.*0\.2\.2/i })).toBeEnabled();
  });

  test('AC-NCP03 shows an installed proxy plugin version and lifecycle actions in the proxy type table', async () => {
    networkCenterApi.fetchSettingsNetworkEgressProviderTypes.mockResolvedValue([
      { installation_id: 'clash-installation', provider_code: 'clash-proxy', display_name: 'Clash / Mihomo Proxy', form_schema: { schema_version: '1flowbase.plugin.form/v1', fields: [{ key: 'subscription_url', label: 'Subscription URL', type: 'string' }] } }
    ]);
    networkCenterApi.fetchSettingsNetworkEgressOfficialPluginCatalog.mockResolvedValue({
      source_kind: 'official_registry', source_label: 'official', registry_url: 'https://example.com/registry.json', source_freshness: 'fresh', entries: [{
        plugin_id: '1flowbase.clash-proxy', provider_code: 'clash-proxy', plugin_type: 'network_egress_provider', display_name: 'Clash / Mihomo Proxy', description: null, protocol: 'clash', current_version: '0.2.3', latest_version: '0.2.4', has_update: true, selected_artifact: {}, help_url: null, model_discovery_mode: 'static', install_status: 'installed', minimum_host_version: '0.1.0', current_host_version: '0.3.0', compatibility_status: 'compatible', compatibility_warning_reason: null
      }]
    });
    networkCenterApi.fetchSettingsNetworkEgressPluginFamilies.mockResolvedValue([
      {
        provider_code: 'clash-proxy', display_name: 'Clash / Mihomo Proxy', current_installation_id: 'v023', current_version: '0.2.3', can_uninstall: true,
        installed_versions: [{ installation_id: 'v023', plugin_version: '0.2.3', is_current: true, can_uninstall: false }]
      }
    ]);

    renderPanel();

    expect(await screen.findByRole('columnheader', { name: /版本|version/i })).toBeInTheDocument();
    expect(screen.getByRole('columnheader', { name: /操作|actions?/i })).toBeInTheDocument();
    const row = await screen.findByRole('row', { name: /Clash \/ Mihomo Proxy.*0\.2\.3/i });
    expect(within(row).getByRole('button', { name: /Clash \/ Mihomo Proxy.*更新|Clash \/ Mihomo Proxy.*update/i })).toBeEnabled();
    expect(within(row).getByRole('button', { name: /Clash \/ Mihomo Proxy.*卸载|Clash \/ Mihomo Proxy.*uninstall/i })).toBeEnabled();
  });
});
