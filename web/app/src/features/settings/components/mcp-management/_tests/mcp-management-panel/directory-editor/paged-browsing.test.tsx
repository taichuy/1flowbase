import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import {
  change, click, grantDirectoryPermissions, mcpManagementApi as api, mouseDown,
  renderPanelWithMountedTool, selectAntdOption, setupDirectoryEditorFixture, waitForRequests
} from './fixture';

async function openDirectory() {
  await click(screen.getByRole('button', { name: '目录编辑' }));
  await waitForRequests();
  return screen.getByRole('dialog', { name: '目录编辑' });
}

describe('MCP directory and table browsing', () => {
  setupDirectoryEditorFixture();

  test('exact non-root page and directory grants allow bound summaries, browsing and saving the 51st tool', async () => {
    grantDirectoryPermissions(['list_mcp_instances_page', 'list_mcp_tools_page', 'mcp.directory.view', 'mcp.directory.replace']);
    await renderPanelWithMountedTool({
      configureCatalog({ tools }) {
        const original = tools[0]!;
        for (let n = 1; n <= 51; n++) {
          tools.push({ ...original, id: `id-${n}`, tool_id: `team_${n}`, name: `Team tool ${n}` });
        }
      }
    });
    const dialog = within(await openDirectory());
    expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledExactlyOnceWith({
      page: 1, page_size: 1, tool_ids: ['search_customer']
    });
    expect(api.fetchSettingsMcpInstances).not.toHaveBeenCalled();
    const instancesReads = api.fetchSettingsMcpInstancesPage.mock.calls.length;
    await click(dialog.getByRole('button', { name: '挂载 Tool' }));
    expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledTimes(1);
    const picker = dialog.getByRole('combobox', { name: '选择 Tool' });
    await mouseDown(picker);
    const next = async () => {
      const listbox = document.getElementById(picker.getAttribute('aria-controls')!);
      const popup = listbox?.closest<HTMLElement>('.ant-select-dropdown');
      expect(popup).toBeInstanceOf(HTMLElement);
      await click(await within(popup!).findByRole('button', { name: '下一页' }));
    };
    await next();
    await waitFor(() => expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledWith({
      page: 2, page_size: 50, keyword: undefined
    }));
    expect(await screen.findByText('Team tool 51')).toBeVisible();
    await change(picker, { target: { value: 'Team' } });
    await waitFor(() => expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledWith({
      page: 1, page_size: 50, keyword: 'Team'
    }));
    await next();
    await selectAntdOption('Team tool 51');
    expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledWith({ page: 2, page_size: 50, keyword: 'Team' });
    expect(api.fetchSettingsMcpInstancesPage).toHaveBeenCalledTimes(instancesReads);
    await click(dialog.getByRole('button', { name: '保存节点' }));
    await waitFor(() => expect(dialog.queryByRole('button', { name: '保存节点' })).not.toBeInTheDocument());
    await click(dialog.getByRole('button', { name: '保存目录' }));
    await waitFor(() => expect(api.replaceSettingsMcpInstanceDirectory).toHaveBeenCalledWith(
      'ops_mcp', expect.objectContaining({ bindings: expect.arrayContaining([
        expect.objectContaining({ tool_id: 'team_51', group_path: '/' })
      ]) }), expect.any(String)
    ));
  });

  test.each([
    ['no tool operation', []],
    ['detail only', ['get_mcp_tool']],
    ['legacy profile only', ['mcp.tools.view']]
  ] as const)('directory read/write with %s cannot browse tools', async (_label, tools) => {
    grantDirectoryPermissions(['list_mcp_instances_page', 'mcp.directory.view', 'mcp.directory.replace', ...tools]);
    await renderPanelWithMountedTool();
    const dialog = within(await openDirectory());
    expect(dialog.getByText(/list_mcp_tools_page/)).toBeVisible();
    expect(dialog.getByRole('button', { name: '挂载 Tool' })).toBeDisabled();
    expect(dialog.getByRole('button', { name: '新建分组' })).toBeEnabled();
    expect(api.fetchSettingsMcpToolsPage).not.toHaveBeenCalled();
    expect(api.fetchSettingsMcpTool).not.toHaveBeenCalled();
  });

  test('the exact instance page grant can switch directories without a tool read grant', async () => {
    grantDirectoryPermissions(['list_mcp_instances_page', 'mcp.directory.view', 'mcp.directory.replace']);
    await renderPanelWithMountedTool({ secondInstance: true });
    const row = screen.getByRole('row', { name: /Ops MCP/ });
    await click(within(row).getByRole('button', { name: '目录编辑' }));
    const dialog = within(await screen.findByRole('dialog', { name: '目录编辑' }));
    await waitFor(() => expect(dialog.getByRole('button', { name: '新建分组' })).toBeEnabled());
    await click(dialog.getByRole('button', { name: '新建分组' }));
    await mouseDown(dialog.getByRole('combobox', { name: '实例名称' }));
    await selectAntdOption('Other MCP');
    expect(api.fetchSettingsMcpInstancesPage).toHaveBeenCalledWith({ page: 1, page_size: 50, keyword: undefined });
    await waitFor(() => expect(api.fetchSettingsMcpInstanceDirectory).toHaveBeenLastCalledWith('other_mcp'));
    expect(api.fetchSettingsMcpToolsPage).not.toHaveBeenCalled();
    expect(api.replaceSettingsMcpInstanceDirectory).not.toHaveBeenCalled();
  });

  test('an open directory cannot use the legacy instance-list grant to read another picker page', async () => {
    grantDirectoryPermissions(['list_mcp_instances_page', 'mcp.directory.view', 'mcp.directory.replace']);
    await renderPanelWithMountedTool();
    const dialog = within(await openDirectory());
    const reads = api.fetchSettingsMcpInstancesPage.mock.calls.length;
    grantDirectoryPermissions(['mcp.instances.view', 'mcp.directory.view', 'mcp.directory.replace']);
    await click(dialog.getByRole('button', { name: '新建分组' }));
    expect(dialog.queryByRole('combobox', { name: '实例名称' })).not.toBeInTheDocument();
    expect(api.fetchSettingsMcpInstancesPage).toHaveBeenCalledTimes(reads);
  });

  test('page grants do not authorize directory replacement', async () => {
    grantDirectoryPermissions(['list_mcp_instances_page', 'list_mcp_tools_page', 'mcp.directory.view']);
    await renderPanelWithMountedTool();
    const dialog = within(await openDirectory());
    expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledExactlyOnceWith({
      page: 1, page_size: 1, tool_ids: ['search_customer']
    });
    expect(dialog.getByRole('button', { name: '挂载 Tool' })).toBeDisabled();
    expect(dialog.getByRole('button', { name: '保存目录' })).toBeDisabled();
    expect(api.replaceSettingsMcpInstanceDirectory).not.toHaveBeenCalled();
  });

  test('the progressive API fixture distinguishes an explicit empty tool ID list from an omitted filter', async () => {
    await renderPanelWithMountedTool();
    const page = { page: 1, page_size: 50 };
    expect(await api.fetchSettingsMcpToolsPage(page)).toMatchObject({ total: 1 });
    expect(await api.fetchSettingsMcpToolsPage({ ...page, tool_ids: [] })).toMatchObject({ total: 0, items: [] });
  });

  test('a failed tool lookup displays the server error and retry fetches the same page', async () => {
    await renderPanelWithMountedTool({ includeBinding: false });
    const dialog = within(await openDirectory());
    api.fetchSettingsMcpToolsPage.mockRejectedValueOnce(new Error('403: tool access denied'));
    await click(dialog.getByRole('button', { name: '挂载 Tool' }));
    await mouseDown(dialog.getByRole('combobox', { name: '选择 Tool' }));
    expect(await screen.findByText('403: tool access denied')).toBeVisible();
    await click(screen.getByRole('button', { name: /重\s*试/ }));
    await selectAntdOption('Search customer');
    expect(api.fetchSettingsMcpToolsPage).toHaveBeenCalledTimes(2);
    expect(api.fetchSettingsMcpToolsPage.mock.calls[0]).toEqual(api.fetchSettingsMcpToolsPage.mock.calls[1]);
  });

  test('a failed bound summary read retries the bound IDs, without opening either picker', async () => {
    await renderPanelWithMountedTool();
    api.fetchSettingsMcpToolsPage.mockRejectedValueOnce(new Error('Summary read failed'));
    const dialog = within(await openDirectory());
    const error = await dialog.findByText('Summary read failed');
    expect(error).toBeVisible();
    await click(dialog.getByRole('button', { name: /重\s*试/ }));
    await waitFor(() => expect(error).not.toBeInTheDocument());
    const request = { page: 1, page_size: 1, tool_ids: ['search_customer'] };
    expect(api.fetchSettingsMcpToolsPage.mock.calls).toEqual([[request], [request]]);
    expect(api.fetchSettingsMcpInstances).not.toHaveBeenCalled();
  });

  test('tab switches retain instance page and page size without fetching the inactive list', async () => {
    await renderPanelWithMountedTool({
      configureCatalog({ instances }) {
        const original = instances[0]!;
        for (let n = 1; n <= 15; n++) {
          instances.push({ ...original, id: `id-${n}`, instance_id: `instance_${n}`, name: `Instance ${n}` });
        }
      }
    });
    expect(api.fetchSettingsMcpToolsPage).not.toHaveBeenCalled();
    const panel = () => within(screen.getByRole('tabpanel', { name: 'MCP 实例' }));
    await mouseDown(panel().getByRole('combobox'));
    await selectAntdOption('10');
    await click(await panel().findByRole('button', { name: /^(页|Page) 2$/i }));
    expect(await panel().findByText('Instance 10')).toBeVisible();
    expect(api.fetchSettingsMcpInstancesPage).toHaveBeenLastCalledWith({ page: 2, page_size: 10 });
    api.fetchSettingsMcpInstancesPage.mockClear();
    await click(screen.getByRole('tab', { name: 'Tool 配置' }));
    expect(await screen.findByText('search_customer')).toBeVisible();
    expect(api.fetchSettingsMcpInstancesPage).not.toHaveBeenCalled();
    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    expect(await panel().findByText('Instance 10')).toBeVisible();
    expect(panel().queryByText('Ops MCP')).not.toBeInTheDocument();
    expect(api.fetchSettingsMcpInstancesPage).not.toHaveBeenCalledWith({ page: 1, page_size: 20 });
  });
});
