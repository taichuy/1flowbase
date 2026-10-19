import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import { Modal } from 'antd';
import type { ReactNode } from 'react';
import { ApiClientError } from '@1flowbase/api-client';
import {
  setupExtensionCenterFixture,
  extensionsApi,
  i18nCatalogApi,
  installedEntry,
  catalogEntry,
  installableCatalogEntry,
  renderSection,
  deferred,
  renderSettled,
  click
} from './fixture';

describe('SettingsExtensionCenterSection', () => {
  setupExtensionCenterFixture();

  test('publisher_cutover shows the dedicated artifact download failure message', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [installableCatalogEntry]
    });
    extensionsApi.installSettingsExtension.mockRejectedValue(
      new ApiClientError({
        status: 502,
        code: 'extension_artifact_not_published',
        message: 'release asset was not found'
      })
    );
    await renderSection('runtime-extensions');
    const row = await screen.findByRole(
      'row',
      { name: /OpenAI Provider/ },
      { timeout: 5000 }
    );

    await click(within(row).getByRole('button', { name: '安装' }));

    expect(
      await screen.findByText('发布方尚未完成该版本发布，请稍后重试')
    ).toBeInTheDocument();
  });

  test('publisher_cutover keeps the generic message for unknown mutation errors', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [installableCatalogEntry]
    });
    extensionsApi.installSettingsExtension.mockRejectedValue(
      new Error('unknown failure')
    );
    await renderSection('runtime-extensions');
    const row = await screen.findByRole(
      'row',
      { name: /OpenAI Provider/ },
      { timeout: 5000 }
    );

    await click(within(row).getByRole('button', { name: '安装' }));

    expect(await screen.findByText('扩展操作失败')).toBeInTheDocument();
  });

  test('Root-AC-003 manages executable plugin activation from the installed table', async () => {
    await renderSection('installed');

    const row = await screen.findByRole('row', { name: /openai/ });
    const activation = within(row).getByRole('switch', {
      name: 'openai 启用'
    });
    expect(activation).toBeChecked();

    await click(activation);
    await waitFor(() => {
      expect(
        extensionsApi.disableSettingsInstalledExtension
      ).toHaveBeenCalledWith('extension-installation-1', 'csrf-123');
    });
  });

  test('AC-001 exposes direct uninstall for a runtime extension and hides legacy version deletion', async () => {
    await renderSection();

    const row = await screen.findByRole('row', { name: /openai/ });
    await click(within(row).getByRole('button', { name: '卸载' }));
    await waitFor(() => {
      expect(
        extensionsApi.deleteSettingsInstalledExtension
      ).toHaveBeenCalledWith('extension-installation-1', 'csrf-123');
    });

    await click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');
    expect(
      within(drawer).queryByRole('button', { name: '删除' })
    ).not.toBeInTheDocument();
  });

  test('switches an installed runtime version from the details drawer', async () => {
    extensionsApi.switchSettingsPluginFamilyVersion.mockResolvedValue(
      installedEntry
    );
    await renderSection();
    const row = await screen.findByRole('row', { name: /openai/ });
    await click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');
    const buttons = within(drawer).getAllByRole('button', {
      name: '切换版本并重启插件'
    });
    expect(buttons[0]).toBeDisabled();
    await click(buttons[1]);
    await act(async () => {
      await vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0].onOk?.();
    });
    await waitFor(() =>
      expect(
        extensionsApi.switchSettingsPluginFamilyVersion
      ).toHaveBeenCalledWith('openai', 'extension-installation-0', 'csrf-123')
    );
  });

  test('activates a selected managed version when the service is disabled', async () => {
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [
        {
          ...installedEntry,
          contract_version: '1flowbase.extension-bus/v1',
          desired_state: 'disabled'
        }
      ]
    });
    await renderSection();
    const row = await screen.findByRole('row', { name: /openai/ });
    await click(within(row).getByRole('button', { name: '查看' }));
    const drawer = await screen.findByRole('dialog');
    await click(
      within(drawer).getAllByRole('button', { name: '切换版本并重启插件' })[1]
    );
    await act(async () => {
      await vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0].onOk?.();
    });
    await waitFor(() =>
      expect(
        extensionsApi.enableSettingsInstalledExtension
      ).toHaveBeenCalledWith('extension-installation-0', 'csrf-123')
    );
    expect(
      extensionsApi.switchSettingsPluginFamilyVersion
    ).not.toHaveBeenCalled();
  });

  test('shows backend activation errors instead of a generic failure', async () => {
    extensionsApi.disableSettingsInstalledExtension.mockRejectedValueOnce(
      new ApiClientError({
        status: 409,
        code: 'managed_conflict',
        message: 'Plugin has in-progress operations'
      })
    );
    await renderSection();
    const row = await screen.findByRole('row', { name: /openai/ });
    await click(within(row).getByRole('switch'));
    expect(
      await screen.findByText('Plugin has in-progress operations')
    ).toBeInTheDocument();
  });

  test('AC-001 exposes direct uninstall for a capability plugin', async () => {
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValueOnce({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [
        {
          ...installedEntry,
          id: 'capability-installation-1',
          category: 'capability-plugins',
          catalog_id: 'capability-plugins:taichuy/openai',
          application_action: 'none',
          application_status: 'not_required'
        }
      ]
    });
    await renderSection('capability-plugins');

    const row = await screen.findByRole('row', { name: /openai/ });
    await click(within(row).getByRole('button', { name: '卸载' }));
    await waitFor(() => {
      expect(
        extensionsApi.deleteSettingsInstalledExtension
      ).toHaveBeenCalledWith('capability-installation-1', 'csrf-123');
    });
  });

  test('AC-1785 projects a missing runtime artifact as uninstalled and reinstalls through the install path', async () => {
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValueOnce({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [
        {
          ...installedEntry,
          status: 'uninstalled',
          availability_status: 'artifact_missing'
        }
      ]
    });
    extensionsApi.fetchSettingsExtensionCatalogEntry.mockResolvedValueOnce({
      ...catalogEntry,
      installation_status: 'uninstalled'
    });
    await renderSection();

    const row = await screen.findByRole('row', { name: /openai/ });
    expect(within(row).getByText('已卸载')).toBeInTheDocument();
    expect(
      within(row).queryByRole('button', { name: '卸载' })
    ).not.toBeInTheDocument();
    expect(
      within(row).queryByRole('button', { name: '同步最新版本' })
    ).not.toBeInTheDocument();

    await click(screen.getByRole('button', { name: '检查更新' }));
    expect(extensionsApi.checkSettingsExtensionUpdates).not.toHaveBeenCalled();

    await click(within(row).getByRole('button', { name: '重新安装' }));
    await waitFor(() => {
      expect(
        extensionsApi.fetchSettingsExtensionCatalogEntry
      ).toHaveBeenCalledWith(
        'runtime-extensions',
        'runtime-extensions:taichuy/openai'
      );
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        expect.objectContaining({ id: 'runtime-extensions:taichuy/openai' }),
        'csrf-123',
        {},
        false
      );
    });
  });

  test('D4-AC-013 keeps legacy version deletion for non-runtime extension categories', async () => {
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValueOnce({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [
        {
          ...installedEntry,
          category: 'agent-flow',
          catalog_id: 'agent-flow:taichuy/openai',
          application_action: 'import_agent_flow',
          application_status: 'not_applied'
        }
      ]
    });
    await renderSection();

    const openaiRows = await screen.findAllByRole('row', { name: /openai/ });
    expect(openaiRows).toHaveLength(1);
    await click(
      within(openaiRows[0]).getByRole('button', { name: '查看' })
    );
    const drawer = await screen.findByRole('dialog');
    expect(within(drawer).getByText('已安装版本')).toBeInTheDocument();
    expect(within(drawer).getByText('0.9.0')).toBeInTheDocument();
    expect(
      within(drawer).getByText('/api/plugins/openai/0.9.0')
    ).toBeInTheDocument();
    const deleteButtons = within(drawer).getAllByRole('button', {
      name: '删除'
    });
    expect(deleteButtons[0]).toBeDisabled();
    expect(deleteButtons[1]).toBeEnabled();
    await click(deleteButtons[1]);
    const confirm = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    await act(async () => {
      await confirm?.onOk?.();
    });
    expect(extensionsApi.deleteSettingsInstalledExtension).toHaveBeenCalledWith(
      'extension-installation-0',
      'csrf-123'
    );
  });

  test('Root-AC-004 resolves and performs an installed-row update instead of switching tabs', async () => {
    await renderSection();
    const row = await screen.findByRole('row', { name: /openai/ });
    await click(screen.getByRole('button', { name: '检查更新' }));
    await waitFor(() =>
      expect(
        within(row).getByRole('button', { name: '同步最新版本' })
      ).toBeEnabled()
    );
    await click(within(row).getByRole('button', { name: '同步最新版本' }));

    await waitFor(() => {
      expect(
        extensionsApi.fetchSettingsExtensionCatalogEntry
      ).toHaveBeenCalledWith(
        'runtime-extensions',
        'runtime-extensions:taichuy/openai'
      );
    });
    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        catalogEntry,
        'csrf-123',
        {},
        true
      );
    });
  });

  test('AC-I18N-UPDATE offers cancel, install-only, and install-and-activate', async () => {
    const i18nCatalogEntry = {
      ...catalogEntry,
      category: 'i18n' as const,
      id: 'i18n:taichuy/platform',
      name: 'Platform translations',
      artifact: 'platform',
      version: '2.0.4',
      current_version: '2.0.1',
      installation_status: 'installed' as const
    };
    const i18nInstallation = {
      ...installedEntry,
      id: 'i18n-installation-2.0.4',
      category: 'i18n' as const,
      catalog_id: 'i18n:taichuy/platform',
      artifact_id: 'platform',
      version: '2.0.4',
      application_action: 'activate_i18n' as const,
      application_status: 'not_applied' as const
    };
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'i18n',
      catalog_page: 'page-i18n',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:i18n',
      catalog_page_locator: 'i18n/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [i18nCatalogEntry]
    });
    extensionsApi.checkSettingsExtensionUpdates.mockResolvedValue({
      category: 'i18n',
      items: [
        {
          catalog_id: 'i18n:taichuy/platform',
          current_version: '2.0.1',
          latest_version: '2.0.4',
          status: 'update_available'
        }
      ]
    });
    extensionsApi.installSettingsExtension.mockResolvedValue({
      installation: i18nInstallation,
      local_artifact_was_present: false,
      node_plugin_installation_id: null,
      application_action: 'activate_i18n',
      application_status: 'not_applied'
    });
    i18nCatalogApi.previewSettingsInstalledI18nCatalog.mockResolvedValue({
      extension_installation_id: 'i18n-installation-2.0.4',
      application_status: 'not_applied',
      active_catalog_version: '2.0.1',
      installed_catalog_version: '2.0.4',
      revision: 7,
      integrity_warnings: [],
      required_integrity_override: null
    });
    i18nCatalogApi.activateSettingsInstalledI18nCatalog.mockRejectedValue(
      new Error('activation conflict')
    );

    await renderSection('i18n');
    const row = await screen.findByRole(
      'row',
      { name: /Platform translations/ },
      { timeout: 5000 }
    );
    await click(within(row).getByRole('button', { name: '更新' }));

    const dialog = (await screen.findByText('更新多语言目录')).closest(
      '[role="dialog"]'
    ) as HTMLElement;
    expect(
      within(dialog).getByRole('button', { name: /取\s*消/ })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '仅安装新版本' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '安装并激活' })
    ).toBeInTheDocument();

    await click(
      within(dialog).getByRole('button', { name: '仅安装新版本' })
    );
    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        i18nCatalogEntry,
        'csrf-123',
        {},
        true
      );
    });
    expect(screen.queryByText('激活多语言目录')).not.toBeInTheDocument();

    const refreshedRow = await screen.findByRole('row', {
      name: /Platform translations/
    });
    await click(within(refreshedRow).getByRole('button', { name: '更新' }));
    const activateDialog = (await screen.findByText('更新多语言目录')).closest(
      '[role="dialog"]'
    ) as HTMLElement;
    await click(
      within(activateDialog).getByRole('button', { name: '安装并激活' })
    );
    await waitFor(() => {
      expect(
        i18nCatalogApi.activateSettingsInstalledI18nCatalog
      ).toHaveBeenCalledWith(
        'i18n-installation-2.0.4',
        { expected_revision: 7 },
        'csrf-123'
      );
    });
    expect(
      await screen.findByText('新版本已安装，但激活失败；当前激活版本未改变')
    ).toBeInTheDocument();
  });

  test('AC-002 keeps the installed target row loading for the complete update request', async () => {
    const secondInstalledEntry = {
      ...installedEntry,
      id: 'extension-installation-2',
      catalog_id: 'runtime-extensions:taichuy/anthropic',
      artifact_id: 'anthropic',
      installed_versions: []
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 2,
      next_cursor: null,
      entries: [installedEntry, secondInstalledEntry]
    });
    const updateResult = {
      installation: installedEntry,
      local_artifact_was_present: true,
      node_plugin_installation_id: 'plugin-installation-1',
      application_action: 'configure_model_provider' as const,
      application_status: 'available' as const
    };
    const pendingUpdate = deferred<typeof updateResult>();
    extensionsApi.installSettingsExtension.mockReturnValue(
      pendingUpdate.promise
    );
    extensionsApi.checkSettingsExtensionUpdates.mockResolvedValue({
      category: 'runtime-extensions',
      items: [
        {
          catalog_id: installedEntry.catalog_id,
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        },
        {
          catalog_id: secondInstalledEntry.catalog_id,
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        }
      ]
    });

    await renderSection();
    const targetRow = await screen.findByRole('row', { name: /openai/ });
    const otherRow = await screen.findByRole('row', { name: /anthropic/ });
    const targetButton = within(targetRow).getByRole('button', {
      name: '同步最新版本'
    });
    const otherButton = within(otherRow).getByRole('button', {
      name: '同步最新版本'
    });
    await click(screen.getByRole('button', { name: '检查更新' }));
    await waitFor(() => {
      expect(targetButton).toBeEnabled();
      expect(otherButton).toBeEnabled();
    });
    await click(targetButton);

    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalled();
      expect(targetButton).toHaveClass('ant-btn-loading');
      expect(otherButton).toBeDisabled();
      expect(otherButton).not.toHaveClass('ant-btn-loading');
    });

    await pendingUpdate.resolve(updateResult);
    await waitFor(() => {
      expect(targetButton).not.toHaveClass('ant-btn-loading');
      expect(otherButton).toBeEnabled();
    });
  });

  test('Root-AC-006 lists a structured risk challenge and retries with its exact codes', async () => {
    const riskError = new Error('confirmation required');
    const challenge = {
      warnings: [
        {
          code: 'signature_invalid',
          message: '签名与产物内容不一致，但仍可继续。',
          overridable: true
        },
        {
          code: 'policy_locked',
          message: '该警告不可由用户确认覆盖。',
          overridable: false
        }
      ],
      compatibility: null
    };
    extensionsApi.installSettingsExtension
      .mockRejectedValueOnce(riskError)
      .mockResolvedValueOnce({});
    extensionsApi.getSettingsExtensionRiskChallenge.mockImplementation(
      (error: unknown) => (error === riskError ? challenge : null)
    );
    await renderSection('runtime-extensions');
    const row = await screen.findByRole(
      'row',
      { name: /OpenAI Provider/ },
      { timeout: 5000 }
    );
    await click(within(row).getByRole('button', { name: '更新' }));

    await waitFor(() => expect(Modal.confirm).toHaveBeenCalledTimes(1));
    expect(
      within(screen.getByRole('row', { name: /OpenAI Provider/ })).getByRole(
        'button',
        { name: /更新$/ }
      )
    ).toHaveClass('ant-btn-loading');
    const riskConfirmation = vi.mocked(Modal.confirm).mock.calls.at(-1)?.[0];
    await renderSettled(riskConfirmation?.content as ReactNode);
    expect(
      screen.getByText('签名与产物内容不一致，但仍可继续。')
    ).toBeInTheDocument();
    expect(screen.getByText('该警告不可由用户确认覆盖。')).toBeInTheDocument();
    await act(async () => {
      await riskConfirmation?.onOk?.();
    });

    await waitFor(() => {
      expect(extensionsApi.installSettingsExtension).toHaveBeenLastCalledWith(
        catalogEntry,
        'csrf-123',
        {
          risk_override: {
            reason: 'user_confirmed',
            acknowledged_warnings: ['signature_invalid']
          }
        },
        true
      );
    });
    await waitFor(() => {
      expect(
        within(screen.getByRole('row', { name: /OpenAI Provider/ })).getByRole(
          'button',
          { name: /更新$/ }
        )
      ).not.toHaveClass('ant-btn-loading');
    });
  });

  test('exposes the upload modal while keeping failed update status visible', async () => {
    extensionsApi.checkSettingsExtensionUpdates.mockRejectedValue(
      new Error('catalog unavailable')
    );
    const view = await renderSection();

    const row = await screen.findByRole('row', { name: /openai/ });
    await click(screen.getByRole('button', { name: '检查更新' }));
    await waitFor(() => {
      expect(
        within(row)
          .getByRole('button', { name: '同步最新版本' })
          .closest('span')?.parentElement
      ).toHaveAttribute('data-update-state', 'unknown_error');
    });

    await click(screen.getByRole('button', { name: '上传插件' }));
    expect(await screen.findByRole('dialog')).toBeInTheDocument();
    expect(document.querySelector('input[type="file"]')).not.toBeNull();
    view.unmount();
  });

  test('AC-001 loads only the generic catalog row being installed and disables the other row', async () => {
    const fusionEntry = {
      ...catalogEntry,
      id: 'capability-plugins:taichuy/fusion',
      category: 'capability-plugins' as const,
      artifact: 'fusion',
      name: 'Fusion',
      current_version: null,
      installation_status: 'not_installed'
    };
    const deepseekEntry = {
      ...fusionEntry,
      id: 'capability-plugins:taichuy/deepseek-v4',
      artifact: 'deepseek-v4',
      name: 'Deepseek V4'
    };
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'capability-plugins',
      catalog_page: 'start',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent',
      catalog_page_locator: 'capability-plugins/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 2,
      entries: [fusionEntry, deepseekEntry]
    });
    const installResult = {
      installation: { ...installedEntry, id: 'agent-installation-1' },
      local_artifact_was_present: false,
      node_plugin_installation_id: null,
      application_action: 'none' as const,
      application_status: 'not_required' as const
    };
    const pendingInstall = deferred<typeof installResult>();
    extensionsApi.installSettingsExtension.mockReturnValue(
      pendingInstall.promise
    );

    await renderSection('capability-plugins');
    const targetRow = await screen.findByRole(
      'row',
      { name: /Fusion/ },
      { timeout: 5000 }
    );
    await screen.findByRole('row', { name: /Deepseek V4/ });
    const targetButton = within(targetRow).getByRole('button', {
      name: '安装'
    });
    await click(targetButton);

    await waitFor(() => {
      const currentTargetButton = within(
        screen.getByRole('row', { name: /Fusion/ })
      ).getByRole('button', { name: /安装$/ });
      const currentOtherButton = within(
        screen.getByRole('row', { name: /Deepseek V4/ })
      ).getByRole('button', { name: '安装' });
      expect(extensionsApi.installSettingsExtension).toHaveBeenCalledWith(
        fusionEntry,
        'csrf-123',
        {},
        false
      );
      expect(currentTargetButton).toHaveClass('ant-btn-loading');
      expect(currentOtherButton).toBeDisabled();
      expect(currentOtherButton).not.toHaveClass('ant-btn-loading');
    });

    await pendingInstall.resolve(installResult);
    await waitFor(() => {
      const currentTargetButton = within(
        screen.getByRole('row', { name: /Fusion/ })
      ).getByRole('button', { name: '安装' });
      const currentOtherButton = within(
        screen.getByRole('row', { name: /Deepseek V4/ })
      ).getByRole('button', { name: '安装' });
      expect(currentTargetButton).not.toHaveClass('ant-btn-loading');
      expect(currentOtherButton).toBeEnabled();
    });
  });
});
