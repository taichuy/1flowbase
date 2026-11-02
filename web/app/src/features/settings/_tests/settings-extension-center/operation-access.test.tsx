import { screen } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import { useAuthStore } from '../../../../state/auth-store';
import { setupExtensionCenterFixture, extensionsApi, installableCatalogEntry, renderSection, click } from './fixture';

function grant(permissions: string[]) {
  const state = useAuthStore.getState();
  useAuthStore.setState({ actor: { ...state.actor!, effective_display_role: 'readonly.audit.20261102' }, me: { ...state.me!, effective_display_role: 'readonly.audit.20261102', permissions } });
}

describe('Extension center exact operations', () => {
  setupExtensionCenterFixture();

  test('readonly can inspect installed metadata without activation, upload or update requests', async () => {
    grant(['extension_center.installed.view', 'extension_center.catalog.view', 'extension_center.catalog.detail']);
    await renderSection();
    expect(screen.getByRole('button', { name: '上传插件' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '检查更新' })).toBeDisabled();
    const activation = screen.getByRole('switch');
    expect(activation).toBeDisabled();
    await click(screen.getByRole('button', { name: /查\s*看/ }));
    expect(await screen.findByText('sha256:previous')).toBeInTheDocument();
    expect(extensionsApi.checkSettingsExtensionUpdates).not.toHaveBeenCalled();
    expect(extensionsApi.installSettingsExtension).not.toHaveBeenCalled();
  });

  test('one install grant leaves upload and update-check disabled', async () => {
    grant(['extension_center.install']);
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({ category: 'runtime-extensions', limit: 20, next_cursor: null, total_entries: 1, entries: [installableCatalogEntry] });
    await renderSection('runtime-extensions');
    expect(screen.getByRole('button', { name: /^安\s*装$/ })).toBeEnabled();
    expect(screen.getByRole('button', { name: '上传插件' })).toBeDisabled();
    expect(screen.getByRole('button', { name: '检查更新' })).toBeDisabled();
  });
});
