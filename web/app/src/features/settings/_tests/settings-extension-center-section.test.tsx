import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import { SettingsExtensionCenterSection } from '../pages/settings-page/SettingsExtensionCenterSection';
import {
  setupExtensionCenterFixture,
  extensionsApi,
  applicationsApi,
  routerApi,
  catalogEntry,
  installableCatalogEntry,
  renderSection,
  deferred,
  click,
  change,
  keyDown
} from './settings-extension-center/fixture';

describe('SettingsExtensionCenterSection', () => {
  setupExtensionCenterFixture();

  test('AC-001 keeps the local category inventory visible when the remote catalog is unavailable', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockRejectedValueOnce(
      new Error('catalog snapshot temporarily inconsistent')
    );
    extensionsApi.checkSettingsExtensionUpdates.mockRejectedValueOnce(
      new Error('catalog unavailable')
    );

    await renderSection('runtime-extensions');

    expect(
      await screen.findByText('在线扩展目录暂时无法加载')
    ).toBeInTheDocument();
    expect(await screen.findByText('openai')).toBeInTheDocument();
    expect(extensionsApi.fetchSettingsInstalledExtensions).toHaveBeenCalledWith(
      undefined,
      'runtime-extensions'
    );
    await waitFor(() => {
      expect(
        screen
          .getByRole('button', { name: '同步最新版本' })
          .closest('[data-update-state]')
      ).toHaveAttribute('data-update-state', 'unknown_error');
    });

    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValueOnce({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [catalogEntry]
    });
    await click(screen.getByRole('button', { name: '重新加载目录' }));

    expect(await screen.findByText('OpenAI Provider')).toBeInTheDocument();
    expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledTimes(
      2
    );
  });

  test('AC-005 loads installed inventory without a remote update check and checks only after the explicit action', async () => {
    await renderSection();

    expect(await screen.findByText('openai')).toBeInTheDocument();
    expect(screen.getAllByRole('tab')).toHaveLength(10);
    expect(screen.getByRole('tab', { name: '应用模板' })).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: 'UI 组件' })).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '来源' })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('columnheader', { name: '可信度' })
    ).toBeInTheDocument();
    expect(screen.getByText('可配置')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '配置供应商' })
    ).toBeInTheDocument();
    expect(extensionsApi.fetchSettingsExtensionCatalog).not.toHaveBeenCalled();
    expect(extensionsApi.checkSettingsExtensionUpdates).not.toHaveBeenCalled();
    await click(screen.getByRole('button', { name: '检查更新' }));
    await waitFor(() => {
      expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledWith(
        {
          category: 'runtime-extensions',
          items: [
            {
              catalog_id: 'runtime-extensions:taichuy/openai',
              current_version: '1.0.0',
              installed_versions: ['1.0.0', '0.9.0']
            }
          ]
        },
        'csrf-123'
      );
    });
    expect(
      screen
        .getByRole('button', { name: '同步最新版本' })
        .closest('[data-update-state]')
    ).toHaveAttribute('data-update-state', 'update_available');

    await click(screen.getByRole('tab', { name: 'runtime-extensions' }));
    expect(routerApi.navigate).toHaveBeenCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'runtime-extensions' },
      search: { q: undefined, cursor: undefined }
    });
  });

  test('catalog tabs label remote versions as latest and automatically check only installed rows', async () => {
    const pendingCheck = deferred<{
      category: 'runtime-extensions';
      items: Array<{
        catalog_id: string;
        current_version: string;
        latest_version: string;
        status: 'update_available';
      }>;
    }>();
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'runtime-extensions',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:page-1',
      catalog_page_locator: 'runtime-extensions/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 2,
      entries: [
        catalogEntry,
        {
          ...installableCatalogEntry,
          id: 'runtime-extensions:taichuy/anthropic',
          name: 'Anthropic Provider',
          artifact: 'anthropic'
        }
      ]
    });
    extensionsApi.checkSettingsExtensionUpdates.mockReturnValue(
      pendingCheck.promise
    );
    await renderSection('runtime-extensions');

    const row = await screen.findByRole(
      'row',
      { name: /OpenAI Provider/ },
      { timeout: 5000 }
    );
    expect(
      screen.getByRole('columnheader', { name: '最新版本' })
    ).toBeInTheDocument();
    expect(
      screen.queryByRole('columnheader', { name: '当前版本' })
    ).not.toBeInTheDocument();
    expect(within(row).getByText('1.1.0')).toBeInTheDocument();
    expect(
      within(row)
        .getByRole('button', { name: '更新' })
        .closest('[data-update-state]')
    ).toHaveAttribute('data-update-state', 'checking');
    expect(
      within(row)
        .getByRole('button', { name: '更新' })
        .closest('[data-update-state]')
        ?.querySelector('.ant-badge-dot')
    ).toHaveStyle('background: transparent');
    expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledWith(
      {
        category: 'runtime-extensions',
        items: [
          {
            catalog_id: 'runtime-extensions:taichuy/openai',
            current_version: '1.0.0',
            installed_versions: ['1.0.0', '0.9.0']
          }
        ]
      },
      'csrf-123'
    );

    await pendingCheck.resolve({
      category: 'runtime-extensions',
      items: [
        {
          catalog_id: 'runtime-extensions:taichuy/openai',
          current_version: '1.0.0',
          latest_version: '1.1.0',
          status: 'update_available'
        }
      ]
    });
    await waitFor(() => {
      expect(
        within(row)
          .getByRole('button', { name: '更新' })
          .closest('[data-update-state]')
      ).toHaveAttribute('data-update-state', 'update_available');
    });
  });

  test('catalog tabs show a failed automatic update check as unknown_error', async () => {
    extensionsApi.checkSettingsExtensionUpdates.mockRejectedValue(
      new Error('catalog unavailable')
    );

    await renderSection('runtime-extensions');
    const row = await screen.findByRole(
      'row',
      { name: /OpenAI Provider/ },
      { timeout: 5000 }
    );

    await waitFor(() => {
      expect(
        within(row)
          .getByRole('button', { name: '更新' })
          .closest('[data-update-state]')
      ).toHaveAttribute('data-update-state', 'unknown_error');
    });
  });

  test('catalog tabs do not recheck the local inventory when the online page changes', async () => {
    const view = await renderSection('runtime-extensions');
    await waitFor(() => {
      expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledTimes(
        1
      );
    });

    view.rerender(
      <SettingsExtensionCenterSection
        category="runtime-extensions"
        cursor="cursor-2"
      />
    );

    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'runtime-extensions',
        { q: undefined, slot_code: undefined, cursor: 'cursor-2' }
      );
      expect(extensionsApi.checkSettingsExtensionUpdates).toHaveBeenCalledTimes(
        1
      );
    });
  });

  test('AC-006 exposes contextual links to the MCP and language management owners', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
      async (category: string) => ({
        category,
        catalog_page: 'page-1',
        catalog_page_number: 1,
        catalog_page_checksum: `sha256:${category}`,
        catalog_page_locator: `${category}/catalog/v1/pages/1.json`,
        limit: 20,
        next_cursor: null,
        total_entries: 0,
        entries: []
      })
    );
    const view = await renderSection('mcp');

    expect(
      await screen.findByRole('link', { name: '前往 MCP 管理' })
    ).toHaveAttribute('href', '/settings/mcp-management?tab=instances');
    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'mcp',
        { q: undefined, slot_code: undefined, cursor: undefined }
      );
    });

    view.rerender(<SettingsExtensionCenterSection category="i18n" />);
    expect(
      await screen.findByRole('link', { name: '前往多语言管理' })
    ).toHaveAttribute('href', '/settings/i18n');
  });

  test('AC-005 routes Agent Flow through the generic catalog and links to local template management', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'agent-flow',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent-flow',
      catalog_page_locator: 'agent-flow/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 1,
      entries: [
        {
          ...catalogEntry,
          category: 'agent-flow',
          id: 'agent-flow:taichuy/fusion',
          name: 'Fusion',
          current_version: null,
          installation_status: 'not_installed'
        }
      ]
    });
    await renderSection('agent-flow');
    await screen.findByText('Fusion');
    const row = screen.getByRole('row', { name: /Fusion/ });
    expect(within(row).getByRole('button', { name: '安装' })).toBeEnabled();
    expect(
      screen.getByRole('link', { name: '前往 Agent Flow 模板管理' })
    ).toHaveAttribute('href', '/templates');
  });

  test('Root-AC-004 keeps generic install for four catalog categories while MCP uses its library', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockImplementation(
      async (category: string) => ({
        category,
        catalog_page: 'page-1',
        catalog_page_number: 1,
        catalog_page_checksum: `sha256:${category}`,
        catalog_page_locator: `${category}/catalog/v1/pages/1.json`,
        limit: 20,
        next_cursor: null,
        total_entries: 1,
        entries: [
          {
            ...catalogEntry,
            category,
            id: `${category}:taichuy/sample`,
            name: `${category} Extension`,
            artifact_kind: null,
            current_version: null,
            installation_status: 'not_installed'
          }
        ]
      })
    );
    const view = await renderSection('capability-plugins');

    for (const category of [
      'capability-plugins',
      'host-extensions',
      'i18n',
      'runtime-extensions'
    ] as const) {
      view.rerender(<SettingsExtensionCenterSection category={category} />);
      const row = await screen.findByRole(
        'row',
        { name: new RegExp(`${category} Extension`) },
        { timeout: 5000 }
      );
      expect(
        within(row).getByRole('button', { name: '安装' })
      ).toBeInTheDocument();
    }

    view.rerender(<SettingsExtensionCenterSection category="mcp" />);
    expect(await screen.findByText('mcp Extension')).toBeInTheDocument();
  });

  test('D5-AC-006 restores cursor from route search and writes pagination to navigation', async () => {
    await renderSection('runtime-extensions', 'cursor-2', 'openai');
    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'runtime-extensions',
        { q: 'openai', slot_code: undefined, cursor: 'cursor-2' }
      );
    });
    await click(screen.getByRole('button', { name: '上一页' }));
    expect(routerApi.navigate).toHaveBeenCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'runtime-extensions' },
      search: { q: 'openai', cursor: undefined }
    });
  });

  test('AC-003/AC-007 submits remote search, clears its cursor, and isolates the query key without a model-provider slot', async () => {
    await renderSection('runtime-extensions', 'cursor-2', 'postgres');

    const search = await screen.findByRole('searchbox');
    expect(await screen.findByText('OpenAI Provider')).toBeInTheDocument();
    await change(search, { target: { value: '  analytics  ' } });
    await keyDown(search, { key: 'Enter', code: 'Enter' });

    expect(routerApi.navigate).toHaveBeenCalledWith({
      to: '/settings/extension-center/$category',
      params: { category: 'runtime-extensions' },
      search: { q: 'analytics', cursor: undefined }
    });
    expect(extensionsApi.settingsExtensionCatalogQueryKey).toHaveBeenCalledWith(
      'runtime-extensions',
      { q: 'postgres', slot_code: undefined, cursor: 'cursor-2' }
    );
    expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
      'runtime-extensions',
      { q: 'postgres', slot_code: undefined, cursor: 'cursor-2' }
    );
  });

  test('AC-007 keeps installed inventory local and omits the remote search control', async () => {
    await renderSection('installed');

    expect(await screen.findByText('openai')).toBeInTheDocument();
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();
    expect(extensionsApi.fetchSettingsExtensionCatalog).not.toHaveBeenCalled();
  });

  test('AC-005 exposes Agent Flow management from its generic catalog', async () => {
    extensionsApi.fetchSettingsExtensionCatalog.mockResolvedValue({
      category: 'agent-flow',
      catalog_page: 'page-1',
      catalog_page_number: 1,
      catalog_page_checksum: 'sha256:agent-flow',
      catalog_page_locator: 'agent-flow/catalog/v1/pages/1.json',
      limit: 20,
      next_cursor: null,
      total_entries: 0,
      entries: []
    });
    await renderSection('agent-flow');
    expect(
      await screen.findByRole('link', { name: '前往 Agent Flow 模板管理' })
    ).toHaveAttribute('href', '/templates');
    await waitFor(() => {
      expect(extensionsApi.fetchSettingsExtensionCatalog).toHaveBeenCalledWith(
        'agent-flow',
        { q: undefined, slot_code: undefined, cursor: undefined }
      );
    });
    expect(extensionsApi.installSettingsExtension).not.toHaveBeenCalled();
    expect(
      applicationsApi.previewInstalledApplicationExtension
    ).not.toHaveBeenCalled();
  });
});
