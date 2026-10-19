import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import {
  setupExtensionCenterFixture,
  extensionsApi,
  i18nCatalogApi,
  installedEntry,
  renderSection,
  click
} from './fixture';

describe('SettingsExtensionCenterSection', () => {
  setupExtensionCenterFixture();

  test('D6-AC-002 reuses the complete MCP bundle review and applies exact preview decisions', async () => {
    const mcpEntry = {
      ...installedEntry,
      id: 'mcp-installation-1',
      category: 'mcp' as const,
      catalog_id: 'mcp:taichuy/sample',
      artifact_id: 'sample',
      application_action: 'import_mcp' as const,
      application_status: 'not_applied' as const
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [mcpEntry]
    });
    extensionsApi.previewSettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: 'mcp-installation-1',
      artifact_installation_status: 'installed',
      workspace_application_status: 'ready_to_import',
      required_conflict_resolution: null,
      integrity_warnings: [
        {
          code: 'checksum_mismatch',
          message: '本地产物校验值与安装记录不一致。',
          overridable: true
        }
      ],
      required_integrity_override: {
        warnings: [
          {
            code: 'checksum_mismatch',
            message: '本地产物校验值与安装记录不一致。',
            overridable: true
          }
        ],
        compatibility: null
      },
      preview: {
        manifest: {
          organization: 'taichuy',
          bundle_id: 'sample',
          bundle_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '*',
          exported_from_system_version: '1.0.0'
        },
        current_system_version: '1.0.0',
        version_status: 'same_system_version',
        effect_summary: {
          changes: 1,
          already_present: 0,
          conflicts: 0,
          unavailable: 0,
          failed: 0
        },
        tools: [
          {
            id: 'tool.weather',
            effect: 'create',
            result: 'imported',
            reason: null
          }
        ],
        instances: [],
        connections: [],
        shared_tool_impacts: []
      }
    });
    extensionsApi.applySettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: 'mcp-installation-1',
      artifact_installation_status: 'installed',
      workspace_application_status: 'imported',
      integrity_warnings: [],
      import_report: {
        manifest: {
          organization: 'taichuy',
          bundle_id: 'sample',
          bundle_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '*',
          exported_from_system_version: '1.0.0'
        },
        current_system_version: '1.0.0',
        version_status: 'same_system_version',
        status: 'completed',
        effect_summary: {
          changes: 1,
          already_present: 0,
          conflicts: 0,
          unavailable: 0,
          failed: 0
        },
        tools: [
          {
            id: 'tool.weather',
            effect: 'create',
            result: 'imported',
            reason: null
          }
        ],
        instances: [],
        connections: [],
        shared_tool_impacts: []
      }
    });
    await renderSection('installed');
    const row = await screen.findByRole('row', { name: /sample/ });
    await click(within(row).getByRole('button', { name: '应用到工作区' }));
    await waitFor(() => {
      expect(
        extensionsApi.previewSettingsInstalledMcpExtension
      ).toHaveBeenCalledWith('mcp-installation-1', 'csrf-123', undefined);
    });
    expect(
      await screen.findByText('本地产物校验值与安装记录不一致。')
    ).toBeInTheDocument();
    expect(screen.getByText('tool.weather')).toBeInTheDocument();
    await click(screen.getByRole('button', { name: /导入/ }));
    await waitFor(() => {
      expect(
        extensionsApi.applySettingsInstalledMcpExtension
      ).toHaveBeenCalledWith('mcp-installation-1', 'csrf-123', {
        integrity_override: {
          reason: 'user_confirmed',
          acknowledged_warnings: ['checksum_mismatch']
        }
      });
    });
  });

  test('AC-002 reconciles an already-present MCP configuration and shows the changed application state', async () => {
    const mcpEntry = {
      ...installedEntry,
      id: 'mcp-installation-present',
      category: 'mcp' as const,
      catalog_id: 'mcp:taichuy/present',
      artifact_id: 'present',
      application_action: 'import_mcp' as const,
      application_status: 'not_applied' as const
    };
    const review = {
      manifest: {
        organization: 'taichuy',
        bundle_id: 'present',
        bundle_version: '1.0.0',
        locale: 'zh_Hans' as const,
        minimum_host_version: '*',
        exported_from_system_version: '1.0.0'
      },
      current_system_version: '1.0.0',
      version_status: 'same_system_version' as const,
      effect_summary: {
        changes: 0,
        already_present: 1,
        conflicts: 0,
        unavailable: 0,
        failed: 0
      },
      tools: [
        {
          id: 'tool.weather',
          effect: 'already_present' as const,
          result: 'already_present' as const,
          reason: null
        }
      ],
      instances: [],
      connections: [],
      shared_tool_impacts: []
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [mcpEntry]
    });
    extensionsApi.previewSettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: mcpEntry.id,
      artifact_installation_status: 'installed',
      workspace_application_status: 'already_present',
      required_conflict_resolution: null,
      integrity_warnings: [],
      required_integrity_override: null,
      preview: review
    });
    extensionsApi.applySettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: mcpEntry.id,
      artifact_installation_status: 'installed',
      workspace_application_status: 'imported',
      integrity_warnings: [],
      import_report: { ...review, status: 'already_applied' }
    });

    await renderSection('installed');
    const row = await screen.findByRole('row', { name: /present/ });
    await click(within(row).getByRole('button', { name: '应用到工作区' }));
    expect(await screen.findByText('tool.weather')).toBeInTheDocument();
    await click(screen.getByRole('button', { name: '确认导入并覆盖' }));

    await waitFor(() =>
      expect(
        extensionsApi.applySettingsInstalledMcpExtension
      ).toHaveBeenCalledWith(mcpEntry.id, 'csrf-123', {})
    );
    expect(
      await screen.findByText('配置已存在，扩展应用状态已同步')
    ).toBeInTheDocument();
  });

  test('AC-001/003 confirms instance overwrite without keep_existing conflict resolution', async () => {
    const mcpEntry = {
      ...installedEntry,
      id: 'mcp-installation-conflict',
      category: 'mcp' as const,
      catalog_id: 'mcp:taichuy/conflict',
      artifact_id: 'conflict',
      application_action: 'import_mcp' as const,
      application_status: 'not_applied' as const
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [mcpEntry]
    });
    extensionsApi.previewSettingsInstalledMcpExtension.mockResolvedValue({
      extension_installation_id: mcpEntry.id,
      artifact_installation_status: 'installed',
      workspace_application_status: 'confirmation_required',
      required_conflict_resolution: 'keep_existing',
      integrity_warnings: [],
      required_integrity_override: null,
      preview: {
        manifest: {
          organization: 'taichuy',
          bundle_id: 'conflict',
          bundle_version: '1.0.0',
          locale: 'zh_Hans',
          minimum_host_version: '*',
          exported_from_system_version: '1.0.0'
        },
        current_system_version: '1.0.0',
        version_status: 'same_system_version',
        effect_summary: {
          changes: 0,
          already_present: 0,
          conflicts: 1,
          unavailable: 0,
          failed: 0
        },
        tools: [
          {
            id: 'tool.weather',
            effect: 'conflict',
            result: 'skipped',
            reason: 'tool_id_conflict'
          }
        ],
        instances: [],
        connections: [],
        shared_tool_impacts: []
      }
    });

    await renderSection('installed');
    const row = await screen.findByRole('row', { name: /conflict/ });
    await click(within(row).getByRole('button', { name: '应用到工作区' }));

    expect(
      await screen.findByText(
        '导入会按 instance_id 创建或原子覆盖实例；模板同步不会修改已导入实例。'
      )
    ).toBeInTheDocument();
    await click(screen.getByRole('button', { name: '确认导入并覆盖' }));
    await waitFor(() =>
      expect(
        extensionsApi.applySettingsInstalledMcpExtension
      ).toHaveBeenCalledWith(mcpEntry.id, 'csrf-123', {})
    );
  });

  test('D6-AC-003 previews and activates the installed local i18n catalog', async () => {
    // Verify the real activation flow without timing cold coverage instrumentation.
    await import('../../components/i18n-catalog/I18nCatalogActivationFlow');
    const i18nRow = {
      ...installedEntry,
      id: 'i18n-installation-1',
      category: 'i18n' as const,
      artifact_id: 'platform',
      application_action: 'activate_i18n' as const,
      application_status: 'not_applied' as const
    };
    extensionsApi.fetchSettingsInstalledExtensions.mockResolvedValue({
      limit: 20,
      total_entries: 1,
      next_cursor: null,
      entries: [i18nRow]
    });
    i18nCatalogApi.previewSettingsInstalledI18nCatalog.mockResolvedValue({
      extension_installation_id: 'i18n-installation-1',
      application_status: 'not_applied',
      active_catalog_version: '2.0.0',
      installed_catalog_version: '2.0.1',
      revision: 4,
      integrity_warnings: [],
      required_integrity_override: null
    });
    i18nCatalogApi.activateSettingsInstalledI18nCatalog.mockResolvedValue({
      status: 'activated',
      catalog_version: '2.0.1',
      revision: 5
    });
    await renderSection('installed');
    const row = await screen.findByRole('row', { name: /platform/ });
    await click(within(row).getByRole('button', { name: '激活' }));
    const activationTitle = await screen.findByText('激活多语言目录');
    const dialog = activationTitle.closest('[role="dialog"]');
    expect(dialog).not.toBeNull();
    const activationDialog = dialog as HTMLElement;
    expect(
      await within(activationDialog).findByText('2.0.0')
    ).toBeInTheDocument();
    expect(
      await within(activationDialog).findByText('2.0.1')
    ).toBeInTheDocument();
    await click(
      within(activationDialog).getByRole('button', { name: /激\s*活/ })
    );
    await waitFor(() => {
      expect(
        i18nCatalogApi.activateSettingsInstalledI18nCatalog
      ).toHaveBeenCalledWith(
        'i18n-installation-1',
        { expected_revision: 4 },
        'csrf-123'
      );
    });
  });
});
