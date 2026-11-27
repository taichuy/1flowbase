import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import {
  mcpManagementApi,
  renderPanelWithMountedTool,
  setupMcpManagementPanelFixture
} from './mcp-management-panel/fixtures/panel';

describe('McpManagementPanel progressive loading', () => {
  setupMcpManagementPanelFixture();

  test('instances tab only fetches paginated instances on mount and defers policy/export/tools/capabilities', async () => {
    window.history.replaceState(
      {},
      '',
      '/settings/mcp-management?tab=instances'
    );
    renderPanelWithMountedTool({ includeGroup: true, includeBinding: true });

    expect(await screen.findByText('ops_mcp')).toBeInTheDocument();

    expect(mcpManagementApi.fetchSettingsMcpInstancesPage).toHaveBeenCalledWith(
      {
        page: 1,
        page_size: 20
      }
    );
    expect(mcpManagementApi.fetchSettingsMcpCatalog).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapabilities
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapabilitiesPage
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapability
    ).not.toHaveBeenCalled();
    expect(mcpManagementApi.fetchSettingsMcpToolsPage).not.toHaveBeenCalled();
    expect(mcpManagementApi.fetchSettingsMcpTool).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInstanceDiscoveryPolicy
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpBundleExportDefaults
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();

    // Opening discovery policy lazily fetches only that instance's policy.
    fireEvent.click(screen.getByRole('button', { name: '更多操作' }));
    const menu = await screen.findByRole('menu');
    fireEvent.click(
      within(menu).getByRole('menuitem', { name: /目录发现配置$/ })
    );
    await waitFor(() => {
      expect(
        mcpManagementApi.fetchSettingsMcpInstanceDiscoveryPolicy
      ).toHaveBeenCalledWith('ops_mcp');
    });
    expect(mcpManagementApi.fetchSettingsMcpCatalog).not.toHaveBeenCalled();

    // Opening Bundle Export modal lazily fetches export defaults.
    fireEvent.click(screen.getByRole('button', { name: /导出$/ }));
    await waitFor(() => {
      expect(
        mcpManagementApi.fetchSettingsMcpBundleExportDefaults
      ).toHaveBeenCalledTimes(1);
    });
  });

  test('tools tab only fetches paginated tool summaries and lazily loads tool/interface detail on edit', async () => {
    window.history.replaceState({}, '', '/settings/mcp-management?tab=tools');
    renderPanelWithMountedTool({ includeGroup: true, includeBinding: true });

    expect(await screen.findByText('search_customer')).toBeInTheDocument();

    expect(mcpManagementApi.fetchSettingsMcpToolsPage).toHaveBeenCalledWith(
      expect.objectContaining({
        page: 1,
        page_size: 20
      })
    );
    expect(mcpManagementApi.fetchSettingsMcpCatalog).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInstancesPage
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapabilities
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapabilitiesPage
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapability
    ).not.toHaveBeenCalled();
    expect(mcpManagementApi.fetchSettingsMcpTool).not.toHaveBeenCalled();

    // Clicking Edit lazily loads single tool detail and single interface capability detail.
    const row = screen.getByText('search_customer').closest('tr');
    expect(row).not.toBeNull();
    fireEvent.click(within(row!).getByRole('button', { name: '编辑' }));

    await waitFor(() => {
      expect(mcpManagementApi.fetchSettingsMcpTool).toHaveBeenCalledWith(
        'search_customer'
      );
      expect(
        mcpManagementApi.fetchSettingsMcpInterfaceCapability
      ).toHaveBeenCalledWith('create_app');
    });
    expect(
      mcpManagementApi.fetchSettingsMcpInterfaceCapabilities
    ).not.toHaveBeenCalled();
    expect(mcpManagementApi.fetchSettingsMcpCatalog).not.toHaveBeenCalled();
  });
});
