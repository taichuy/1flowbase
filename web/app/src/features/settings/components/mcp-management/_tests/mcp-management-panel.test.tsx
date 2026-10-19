import { expandTreeRootIfCollapsed, mcpManagementApi, renderPanelWithMountedTool, selectAntdOption, setupMcpManagementPanelFixture } from './mcp-management-panel/fixtures/panel';
import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

describe('McpManagementPanel', () => {
  setupMcpManagementPanelFixture();

  test('keeps mount paths in binding management instead of the tool table', async () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'Tool 配置' }));
    const toolsPanel = screen.getByRole('tabpanel', { name: 'Tool 配置' });

    expect(
      within(toolsPanel).queryByPlaceholderText('group_path')
    ).not.toBeInTheDocument();
    expect(
      within(toolsPanel).queryByRole('columnheader', { name: 'group_path' })
    ).not.toBeInTheDocument();
    expect(
      within(toolsPanel).queryByText('/ops/customer')
    ).not.toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'Tool 名称' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'tool_id' })
    ).toBeInTheDocument();
    expect(within(toolsPanel).getByText('Search customer')).toBeInTheDocument();
    expect(within(toolsPanel).getByText('search_customer')).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'operation' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: 'Tool 类型' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByRole('columnheader', { name: '执行来源' })
    ).toBeInTheDocument();
    expect(
      within(toolsPanel).getByText('POST /api/console/apps')
    ).toBeInTheDocument();
    expect(within(toolsPanel).getByText('create_app')).toBeInTheDocument();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    expect(
      within(instancesPanel).queryByLabelText('挂载路径')
    ).not.toBeInTheDocument();
    expect(
      within(instancesPanel).queryByRole('columnheader', { name: '挂载路径' })
    ).not.toBeInTheDocument();
    expect(
      within(instancesPanel).queryByText('/ops/customer')
    ).not.toBeInTheDocument();

    const directoryEditorButton = within(instancesPanel).getByRole('button', {
      name: '目录编辑'
    });
    expect(directoryEditorButton).toBeEnabled();
    fireEvent.click(directoryEditorButton);

    const dialog = await screen.findByRole('dialog', { name: '目录编辑' });
    const modalShell = dialog.closest('.ant-modal');
    const modalScrollBody = screen.getByTestId(
      'fixed-height-modal-scroll-body'
    );

    expect(modalShell).toHaveStyle({ width: '840px' });
    expect(modalScrollBody).toHaveClass('mcp-management__directory-modal');
    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('路径')).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('combobox', { name: '路径' })
    ).not.toBeInTheDocument();
    expect(within(dialog).getByLabelText('挂载路径')).not.toBeVisible();
    expect(within(dialog).getByRole('tree')).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(
      within(dialog).getByRole('heading', { name: '新建 Tool 挂载' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('路径')).not.toBeVisible();
    expect(within(dialog).getByLabelText('挂载路径')).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('columnheader', { name: '挂载路径' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole('columnheader', { name: 'display_alias' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getAllByLabelText('编辑 Tool 挂载').length
    ).toBeGreaterThan(0);
  });

  test('shows instance name and instance_id in separate columns with matching action icons', () => {
    renderPanelWithMountedTool();

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    expect(
      within(instancesPanel)
        .getAllByRole('columnheader')
        .slice(0, 2)
        .map((header) => header.textContent)
    ).toEqual(['instance_id', '实例名称']);

    const directoryEditorButton = within(instancesPanel).getByRole('button', {
      name: '目录编辑'
    });
    const editButton = within(instancesPanel).getByRole('button', {
      name: '编辑'
    });

    expect(
      editButton.compareDocumentPosition(directoryEditorButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);

    expect(
      directoryEditorButton.querySelector('.anticon-setting')
    ).toBeInTheDocument();
    expect(editButton.querySelector('.anticon-edit')).toBeInTheDocument();
  });

  test('enables authenticated WebMCP exposure from the instance editor', async () => {
    mcpManagementApi.updateSettingsMcpInstance.mockResolvedValue({});
    const dispatchEvent = vi.spyOn(window, 'dispatchEvent');
    renderPanelWithMountedTool();

    const instancesPanel = screen.getByRole('tabpanel');
    fireEvent.click(
      within(instancesPanel).getByRole('button', {
        name: /^(?:编辑|auto\.edit)$/
      })
    );

    const dialog = await screen.findByRole('dialog');
    const webMcpSwitch = within(dialog).getByRole('switch', {
      name: 'WebMCP'
    });
    expect(webMcpSwitch).not.toBeChecked();

    fireEvent.click(webMcpSwitch);
    fireEvent.click(within(dialog).getByRole('button', { name: /确\s*定/u }));

    await waitFor(() => {
      expect(mcpManagementApi.updateSettingsMcpInstance).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({
          webmcp_exposure: 'authenticated_session'
        }),
        'csrf-123'
      );
    });
    expect(dispatchEvent).toHaveBeenCalledWith(
      expect.objectContaining({
        type: '1flowbase:webmcp-registrations-changed'
      })
    );
  });

  test('keeps three primary instance actions and moves the rest into more actions', async () => {
    mcpManagementApi.exportSettingsMcpInstanceBundle.mockResolvedValue({
      blob: new Blob(['bundle'], { type: 'application/zip' }),
      filename: 'mcp-instance-ops_mcp.zip'
    });
    Object.defineProperty(window.URL, 'createObjectURL', {
      configurable: true,
      value: vi.fn(() => 'blob:mcp-instance')
    });
    Object.defineProperty(window.URL, 'revokeObjectURL', {
      configurable: true,
      value: vi.fn()
    });
    vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
    renderPanelWithMountedTool();

    const instancesPanel = screen.getByRole('tabpanel', {
      name: 'MCP 实例'
    });
    expect(
      within(instancesPanel).getByRole('button', { name: '编辑' })
    ).toBeInTheDocument();
    expect(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    ).toBeInTheDocument();
    expect(
      within(instancesPanel).getByRole('button', { name: '连接客户端' })
    ).toBeInTheDocument();
    expect(
      within(instancesPanel).queryByRole('button', { name: '目录发现配置' })
    ).not.toBeInTheDocument();
    expect(
      within(instancesPanel).queryByRole('button', { name: '导出此实例' })
    ).not.toBeInTheDocument();

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '更多操作' })
    );
    const menu = await screen.findByRole('menu');
    expect(
      within(menu).getByRole('menuitem', { name: /目录发现配置$/ })
    ).toBeInTheDocument();
    expect(
      within(menu).getByRole('menuitem', { name: /复制实例$/ })
    ).toBeInTheDocument();
    expect(
      within(menu).getByRole('menuitem', { name: /删除$/ })
    ).toBeInTheDocument();
    fireEvent.click(
      within(menu).getByRole('menuitem', { name: /导出此实例$/ })
    );
    const dialog = await screen.findByRole('dialog', {
      name: '导出 MCP 实例配置包'
    });
    expect(within(dialog).getByLabelText('bundle_id')).toHaveValue('ops_mcp');

    fireEvent.click(within(dialog).getByRole('button', { name: '导出此实例' }));
    await waitFor(() => {
      expect(
        mcpManagementApi.exportSettingsMcpInstanceBundle
      ).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({ bundle_id: 'ops_mcp' }),
        expect.any(String)
      );
    });
  });

  test('copies an instance after requiring a new id and name', async () => {
    mcpManagementApi.copySettingsMcpInstance.mockResolvedValue({
      id: 'instance-copy-id',
      instance_id: 'ops_mcp_copy',
      name: 'Ops MCP Copy',
      description_short: 'Mounted instance',
      status: 'draft',
      default_entry_path: '/ops'
    });
    renderPanelWithMountedTool();

    const instancesPanel = screen.getByRole('tabpanel', {
      name: 'MCP 实例'
    });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '更多操作' })
    );
    const menu = await screen.findByRole('menu');
    fireEvent.click(within(menu).getByRole('menuitem', { name: /复制实例$/ }));
    const copyDialog = await screen.findByRole('dialog', {
      name: '复制 MCP 实例'
    });
    fireEvent.change(within(copyDialog).getByLabelText('instance_id'), {
      target: { value: 'ops_mcp_copy' }
    });
    fireEvent.change(within(copyDialog).getByLabelText('实例名称'), {
      target: { value: 'Ops MCP Copy' }
    });
    fireEvent.click(
      within(copyDialog).getByRole('button', { name: /复\s*制/ })
    );
    await waitFor(() => {
      expect(mcpManagementApi.copySettingsMcpInstance).toHaveBeenCalledWith(
        'ops_mcp',
        { instance_id: 'ops_mcp_copy', name: 'Ops MCP Copy' },
        expect.any(String)
      );
    });
  });

  test('hides the edit binding selector when there are no existing tool bindings', () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).queryAllByLabelText('编辑 Tool 挂载')).toHaveLength(
      0
    );
    expect(within(dialog).getByLabelText('挂载路径')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
  });

  test('localizes directory editor field labels while keeping tool_id raw', () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });

    expect(within(dialog).getByLabelText('路径')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('显示名称')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('简短描述')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('启用')).toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('display_name')
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('description_short')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('enabled')).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
    expect(within(dialog).getByLabelText('可见')).toBeInTheDocument();
    expect(within(dialog).queryByLabelText('显示别名')).not.toBeInTheDocument();
    expect(
      within(dialog).queryByLabelText('display_alias')
    ).not.toBeInTheDocument();
    expect(within(dialog).queryByLabelText('visible')).not.toBeInTheDocument();
  });

  test('does not expose or preserve display alias when saving a tool binding', async () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(within(dialog).queryByLabelText('显示别名')).not.toBeInTheDocument();

    await waitFor(() => {
      expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
    });

    fireEvent.mouseDown(
      within(dialog).getByRole('combobox', { name: 'tool_id' })
    );
    await selectAntdOption('Search customer');
    fireEvent.click(within(dialog).getByRole('button', { name: /保存/ }));

    await waitFor(() => {
      expect(
        mcpManagementApi.createSettingsMcpToolBinding
      ).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({
          tool_id: 'search_customer',
          display_alias: null
        }),
        expect.any(String)
      );
    });
    expect(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).toBeInTheDocument();
  });

  test('keeps the directory editor open and selects the saved group', async () => {
    mcpManagementApi.upsertSettingsMcpGroup.mockResolvedValue({
      id: 'group-customer-ops',
      instance_record_id: 'instance-record-1',
      path: '/customer_ops',
      display_name: 'customer_ops',
      description_short: null,
      enabled: true,
      sort_order: 0
    });
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    fireEvent.click(rootLabel);
    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));
    fireEvent.change(within(dialog).getByLabelText('路径'), {
      target: { value: 'customer_ops' }
    });
    fireEvent.click(within(dialog).getByRole('button', { name: /保存/ }));

    await waitFor(() => {
      expect(mcpManagementApi.upsertSettingsMcpGroup).toHaveBeenCalledWith(
        'ops_mcp',
        expect.objectContaining({
          path: '/customer_ops',
          display_name: null
        }),
        expect.any(String)
      );
    });
    expect(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
      'customer_ops'
    );
  });

  test('exposes explicit creation actions beside the directory tree', () => {
    renderPanelWithMountedTool({ includeBinding: false });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('button', { name: '新增' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).getByRole('heading', { name: '新建分组' })
    ).toBeInTheDocument();
  });

  test('starts the directory tree fully collapsed', () => {
    renderPanelWithMountedTool({ includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const tree = within(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).getByRole('tree');
    expect(within(tree).getAllByRole('treeitem')[0]).toHaveAttribute(
      'aria-expanded',
      'false'
    );
    expect(within(tree).queryByText('ops')).not.toBeInTheDocument();
  });

  test('starts a child path from the selected group', async () => {
    renderPanelWithMountedTool({ includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    fireEvent.click(within(dialog).getByText('ops'));
    await waitFor(() => {
      expect(
        within(dialog)
          .getByText('ops')
          .closest('.ant-tree-node-content-wrapper')
      ).toHaveClass('ant-tree-node-selected');
    });
    fireEvent.click(within(dialog).getByRole('button', { name: '新建分组' }));

    await waitFor(() => {
      expect(within(dialog).getByLabelText('路径')).toHaveValue('');
    });
    expect(within(dialog).getByLabelText('新增至父目录：')).toHaveValue(
      '/ops/'
    );
    expect(within(dialog).getByText(/新增至父目录/)).toBeInTheDocument();
    expect(within(dialog).getByText('目标目录： /ops')).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '取消子分组新建' })
    ).toBeInTheDocument();
  });

  test('mounts a Tool under the selected group without losing tree selection', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);

    const groupLabel = within(dialog).getByText('ops');
    fireEvent.click(groupLabel);
    await waitFor(() => {
      expect(
        within(dialog)
          .getByText('ops')
          .closest('.ant-tree-node-content-wrapper')
      ).toHaveClass('ant-tree-node-selected');
    });

    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    await waitFor(() => {
      expect(
        within(dialog).getByRole('heading', { name: '新建 Tool 挂载' })
      ).toBeInTheDocument();
      expect(within(dialog).getByLabelText('挂载路径')).toHaveValue('/ops');
      expect(
        within(dialog)
          .getByText('ops')
          .closest('.ant-tree-node-content-wrapper')
      ).toHaveClass('ant-tree-node-selected');
    });
  });

  test('edits a group directly without changing the selected directory', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);

    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    const rootNode = rootLabel.closest('.ant-tree-node-content-wrapper');
    const groupLabel = within(dialog).getByText('ops');
    const groupNode = groupLabel.closest('.ant-tree-node-content-wrapper');
    expect(rootNode).toHaveClass('ant-tree-node-selected');
    expect(groupNode).not.toHaveClass('ant-tree-node-selected');

    fireEvent.mouseEnter(groupNode as HTMLElement);
    fireEvent.click(
      within(groupNode as HTMLElement).getByRole('button', { name: '编辑' })
    );

    const status = dialog.querySelector(
      '.mcp-management__directory-editor-status'
    );
    expect(status).toBeInstanceOf(HTMLElement);
    await waitFor(() =>
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue('ops')
    );
    expect(within(dialog).queryByLabelText('路径')).not.toBeInTheDocument();
    await waitFor(() =>
      expect(within(dialog).getByText('/ops')).toBeInTheDocument()
    );
    expect(rootNode).toHaveClass('ant-tree-node-selected');
    expect(groupNode).not.toHaveClass('ant-tree-node-selected');
    expect(status).toHaveTextContent('已保存');
    expect(status).toHaveTextContent('分组');
  });

  test('shows unsaved after editing and returns to saved after saving', async () => {
    mcpManagementApi.upsertSettingsMcpGroup.mockResolvedValue(undefined);
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);
    fireEvent.click(within(dialog).getByText('ops'));

    const status = dialog.querySelector(
      '.mcp-management__directory-editor-status'
    );
    await waitFor(() => expect(status).toHaveTextContent('已保存'));

    fireEvent.change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'updated_ops' }
    });
    await waitFor(() => expect(status).toHaveTextContent('未保存'));

    fireEvent.click(within(dialog).getByRole('button', { name: '保存分组' }));
    await waitFor(() => expect(status).toHaveTextContent('已保存'));

    fireEvent.click(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    );
    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '目录编辑' })
      ).not.toBeInTheDocument();
    });
    expect(screen.queryByText('放弃未保存的更改？')).not.toBeInTheDocument();
  });

  test('selects a group node and opens that group for editing', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    expandTreeRootIfCollapsed(tree);
    fireEvent.click(within(dialog).getByText('ops'));

    await waitFor(() => {
      expect(
        within(dialog)
          .getByText('ops')
          .closest('.ant-tree-node-content-wrapper')
      ).toHaveClass('ant-tree-node-selected');
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue('ops');
      expect(
        within(dialog).getByRole('button', { name: '保存分组' })
      ).toBeInTheDocument();
    });
  });

  test('closes the directory editor after an empty Tool mount validation failure', async () => {
    renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    fireEvent.click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    fireEvent.click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    fireEvent.click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    await waitFor(() => {
      expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
    });
    const visibleSwitch = within(dialog).getByRole('switch', { name: '可见' });
    fireEvent.click(visibleSwitch);
    fireEvent.click(visibleSwitch);
    fireEvent.click(
      within(dialog).getByRole('button', { name: '保存 Tool 挂载' })
    );
    await waitFor(() => {
      expect(within(dialog).getByLabelText('tool_id')).toHaveAttribute(
        'aria-invalid',
        'true'
      );
    });

    fireEvent.click(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    );

    await waitFor(() => {
      expect(
        screen.queryByRole('dialog', { name: '目录编辑' })
      ).not.toBeInTheDocument();
    });
  });
});
