import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';
import { mcpManagementApi, renderPanel, setupMcpManagementPanelFixture } from './mcp-management-panel/fixtures/panel';

describe('McpManagementPanel', () => {
  setupMcpManagementPanelFixture();

  test('shows the backend system version and exports without a client-selected host floor', async () => {
    mcpManagementApi.exportSettingsMcpBundle.mockResolvedValue({
      blob: new Blob(['bundle'], { type: 'application/zip' }),
      filename: 'mcp-bundle.zip'
    });
    Object.defineProperty(window.URL, 'createObjectURL', {
      configurable: true,
      value: vi.fn(() => 'blob:mcp-bundle')
    });
    Object.defineProperty(window.URL, 'revokeObjectURL', {
      configurable: true,
      value: vi.fn()
    });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    renderPanel();

    fireEvent.click(screen.getByRole('button', { name: /导出$/ }));
    const dialog = await screen.findByRole('dialog', {
      name: '导出 MCP 配置包'
    });
    expect(
      within(dialog).queryByLabelText('minimum_host_version')
    ).not.toBeInTheDocument();
    expect(await within(dialog).findByText(/0.3.0/)).toBeInTheDocument();
    fireEvent.click(within(dialog).getByRole('button', { name: /导\s*出/u }));

    await waitFor(() => {
      expect(mcpManagementApi.exportSettingsMcpBundle).toHaveBeenCalledWith(
        expect.not.objectContaining({
          minimum_host_version: expect.anything()
        }),
        expect.any(String)
      );
    });
  });

  test('previews an MCP bundle, warns for older source and imports the remaining items', async () => {
    // AC-005, AC-007, AC-009 and AC-010.
    renderPanel();

    const file = new File(['bundle'], 'taichuy-bundle.zip', {
      type: 'application/zip'
    });
    fireEvent.change(screen.getByLabelText('选择 MCP 配置包'), {
      target: { files: [file] }
    });

    expect(
      await screen.findByText('配置包来自较低版本的 1flowbase')
    ).toBeInTheDocument();
    expect(screen.getByText('removed_tool')).toBeInTheDocument();
    expect(screen.getByText('接口不存在')).toBeInTheDocument();
    expect(
      screen.getByText('019b5f8f-0000-7000-8000-000000000001')
    ).toBeInTheDocument();
    expect(screen.getByText('缺少凭据')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '确认导入并覆盖' }));
    await waitFor(() => {
      expect(mcpManagementApi.importSettingsMcpBundle).toHaveBeenCalledWith(
        file,
        expect.any(String)
      );
    });
    expect(await screen.findByText('导入完成，但存在警告')).toBeInTheDocument();
  });

  test('reuses the local template library and import flow in MCP Management', async () => {
    renderPanel();

    expect(screen.getByRole('button', { name: /导出$/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /导入$/ }));
    const sourceDialog = await screen.findByRole('dialog', {
      name: '选择配置包来源'
    });
    expect(
      await within(sourceDialog).findByText('taichuy/1flowbase_zh_hans')
    ).toBeInTheDocument();

    const bundleRow = within(sourceDialog).getByRole('row', {
      name: /taichuy\/1flowbase_zh_hans/
    });
    fireEvent.click(within(bundleRow).getByRole('button', { name: '导入' }));
    await waitFor(() => {
      expect(
        mcpManagementApi.previewSettingsMcpTemplateLibraryBundle
      ).toHaveBeenCalledWith(
        'taichuy',
        '1flowbase_zh_hans',
        {},
        expect.any(String)
      );
    });

    fireEvent.click(
      await screen.findByRole('button', { name: '确认导入并覆盖' })
    );
    await waitFor(() => {
      expect(
        mcpManagementApi.importSettingsMcpTemplateLibraryBundle
      ).toHaveBeenCalledWith(
        'taichuy',
        '1flowbase_zh_hans',
        {},
        expect.any(String)
      );
    });
  });

});
