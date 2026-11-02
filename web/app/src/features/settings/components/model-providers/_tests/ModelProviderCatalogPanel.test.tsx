import { useAuthStore } from '../../../../../state/auth-store';
import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

import type { SettingsPluginFamilyEntry } from '../../../api/extensions/plugins';
import { ModelProviderCatalogPanel } from '../ModelProviderCatalogPanel';

const installedFamily: SettingsPluginFamilyEntry = {
  provider_code: 'openai_compatible',
  display_name: 'OpenAI Compatible',
  description: null,
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
  installation_status: 'assigned',
  current_local_artifact: {
    node_id: 'node-1',
    installation_id: 'installation-1',
    local_version: '0.3.17',
    local_checksum: null,
    installed_path: '/plugins/openai-compatible',
    artifact_status: 'ready',
    runtime_status: 'active',
    checked_at: '2026-08-20T10:00:00Z',
    last_error: null
  },
  latest_version: '0.3.17',
  has_update: false,
  installed_versions: []
};

beforeEach(() => {
  useAuthStore.setState({ actor: { id: 'root', account: 'root', effective_display_role: 'root', current_workspace_id: 'workspace' }, me: null });
});

describe('ModelProviderCatalogPanel', () => {
  test('shows only installed-provider actions', () => {
    const onUninstall = vi.fn();
    render(
      <ModelProviderCatalogPanel
        overviewRows={[]}
        entries={[installedFamily]}
        currentCatalogEntries={{}}
        canManage
        onCreate={vi.fn()}
        onViewInstances={vi.fn()}
        onUpgradeLatest={vi.fn()}
        onSwitchVersion={vi.fn()}
        onUninstall={onUninstall}
      />
    );

    expect(screen.getByText('可用')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '卸载' }));
    expect(onUninstall).toHaveBeenCalledWith(installedFamily);
  });

  test('disables new instances when the current node artifact is unavailable', () => {
    const unavailableFamily: SettingsPluginFamilyEntry = {
      ...installedFamily,
      current_local_artifact: {
        ...installedFamily.current_local_artifact,
        artifact_status: 'outdated',
        runtime_status: 'inactive'
      }
    };

    render(
      <ModelProviderCatalogPanel
        overviewRows={[]}
        entries={[unavailableFamily]}
        currentCatalogEntries={{}}
        canManage
        onCreate={vi.fn()}
        onViewInstances={vi.fn()}
        onUpgradeLatest={vi.fn()}
        onSwitchVersion={vi.fn()}
        onUninstall={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: '新增' })).toBeDisabled();
    expect(screen.getByText('不可用')).toBeInTheDocument();
    expect(screen.queryByText('可用')).not.toBeInTheDocument();
  });
  test.each([
    ['readonly', [], false, false],
    ['single create', ['model_providers.instances.create'], true, false],
    ['root', [], true, true]
  ] as const)('%s preserves instance inspection and isolates create from uninstall', (_name, permissions, canCreate, canDelete) => {
    const role = _name === 'root' ? 'root' : 'readonly.audit.20261102';
    useAuthStore.setState({
      actor: { ...useAuthStore.getState().actor!, effective_display_role: role },
      me: {
        id: 'user', account: 'auditor', email: 'auditor@example.com', phone: null,
        nickname: 'Auditor', name: 'Auditor', avatar_url: null, introduction: '',
        effective_display_role: role, permissions: [...permissions]
      }
    });
    const onViewInstances = vi.fn();
    render(<ModelProviderCatalogPanel overviewRows={[]} entries={[installedFamily]} currentCatalogEntries={{}} onCreate={vi.fn()} onViewInstances={onViewInstances} onUpgradeLatest={vi.fn()} onSwitchVersion={vi.fn()} onUninstall={vi.fn()} />);
    const create = screen.getByRole('button', { name: '新增' });
    const remove = screen.getByRole('button', { name: '卸载' });
    expect(create).toHaveProperty('disabled', !canCreate);
    expect(remove).toHaveProperty('disabled', !canDelete);
    fireEvent.click(screen.getByRole('button', { name: '管理' }));
    expect(onViewInstances).toHaveBeenCalledWith(installedFamily);
  });

});
