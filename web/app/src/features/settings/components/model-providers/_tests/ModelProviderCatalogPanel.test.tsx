import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import type { SettingsPluginFamilyEntry } from '../../../api/plugins';
import { ModelProviderCatalogPanel } from '../ModelProviderCatalogPanel';

const uninstalledFamily: SettingsPluginFamilyEntry = {
  provider_code: 'openai_compatible',
  plugin_type: 'model_provider',
  namespace: 'plugin.openai_compatible',
  label_key: 'plugin.label',
  description_key: 'plugin.description',
  provider_label_key: 'provider.label',
  icon: null,
  protocol: 'openai_compatible',
  help_url: null,
  default_base_url: null,
  model_discovery_mode: 'hybrid',
  current_installation_id: 'installation-1',
  current_version: '0.3.17',
  current_local_artifact: {
    node_id: 'node-1',
    installation_id: 'installation-1',
    local_version: '0.3.17',
    local_checksum: null,
    installed_path: null,
    artifact_status: 'missing',
    runtime_status: 'inactive',
    checked_at: '2026-08-20T10:00:00Z',
    last_error: 'artifact_missing'
  },
  latest_version: '0.3.17',
  has_update: false,
  installed_versions: []
};

describe('ModelProviderCatalogPanel', () => {
  test('AC-1785 shows an uninstalled provider without an uninstall action and exposes reinstall', () => {
    const onReinstall = vi.fn();
    render(
      <ModelProviderCatalogPanel
        overviewRows={[]}
        entries={[uninstalledFamily]}
        currentCatalogEntries={{}}
        canManage
        onCreate={vi.fn()}
        onViewInstances={vi.fn()}
        onUpgradeLatest={vi.fn()}
        onSwitchVersion={vi.fn()}
        onUninstall={vi.fn()}
        onReinstall={onReinstall}
      />
    );

    expect(screen.getByText('已卸载')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '卸载' })
    ).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '重新安装' }));
    expect(onReinstall).toHaveBeenCalledWith(uninstalledFamily);
  });
});
