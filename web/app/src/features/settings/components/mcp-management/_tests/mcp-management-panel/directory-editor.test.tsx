import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import {
  setupDirectoryEditorFixture,
  click,
  change,
  mouseDown,
  renderPanelWithMountedTool,
  expandTreeRootIfCollapsed,
  selectAntdOption,
  mcpManagementApi
} from './directory-editor/fixture';

describe('McpManagementPanel', () => {
  setupDirectoryEditorFixture();

  test('confirms before closing an editor with unsaved changes', async () => {
    await renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    await change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'Unsaved group' }
    });
    await waitFor(() => {
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
        'Unsaved group'
      );
    });
    await click(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    );

    expect(screen.getByText('放弃未保存的更改？')).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '放弃更改' })
    ).toBeInTheDocument();
    await click(screen.getByRole('button', { name: '继续编辑' }));
    await waitFor(() =>
      expect(screen.queryByText('放弃未保存的更改？')).not.toBeInTheDocument()
    );
    expect(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).toBeInTheDocument();
  });

  test('shows explicit group and Tool creation actions without editor tabs', async () => {
    await renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });

    await click(within(dialog).getByRole('button', { name: '新建分组' }));
    expect(
      dialog.querySelector('.mcp-management__directory-editor-status')
    ).toHaveTextContent('未保存');

    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeInTheDocument();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeInTheDocument();
    expect(
      within(dialog).queryByRole('tab', { name: '分组' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole('tab', { name: 'Tool 挂载' })
    ).not.toBeInTheDocument();
  });

  test('AC-R11 hides the immutable path field while editing a group', async () => {
    await renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    await expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    await click(within(dialog).getByText('ops'));
    await waitFor(() => {
      expect(
        within(dialog).getByRole('heading', { name: '编辑分组' })
      ).toBeInTheDocument();
    });
    expect(within(dialog).queryByLabelText('路径')).not.toBeInTheDocument();

    await click(within(dialog).getByRole('button', { name: '新建分组' }));
    expect(within(dialog).getByLabelText('路径')).toBeInTheDocument();
  });

  test('does not preview a duplicate draft group when the path already exists', async () => {
    await renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    await expandTreeRootIfCollapsed(tree);

    await change(within(dialog).getByLabelText('路径'), {
      target: { value: 'ops' }
    });

    expect(within(tree).getAllByText('ops')).toHaveLength(1);
  });

  test('hides directory sort order fields because ordering is handled by tree drag', async () => {
    await renderPanelWithMountedTool();

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });

    expect(
      within(dialog).queryByLabelText('sort_order')
    ).not.toBeInTheDocument();

    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    expect(
      within(dialog).queryByLabelText('sort_order')
    ).not.toBeInTheDocument();
  });

  test('places directory cancel and save actions in the modal footer for both editor tabs', async () => {
    await renderPanelWithMountedTool();

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const modalScrollBody = screen.getByTestId(
      'fixed-height-modal-scroll-body'
    );

    const cancelButton = within(dialog).getByRole('button', {
      name: '关闭目录编辑'
    });
    const saveButton = within(dialog).getByRole('button', { name: /保存/ });

    expect(cancelButton).toBeInTheDocument();
    expect(saveButton).toBeInTheDocument();
    expect(
      cancelButton.compareDocumentPosition(saveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(modalScrollBody).queryByRole('button', { name: /取\s*消/ })
    ).not.toBeInTheDocument();
    expect(
      within(modalScrollBody).queryByRole('button', { name: /保存/ })
    ).not.toBeInTheDocument();

    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    const bindingCancelButton = within(dialog).getByRole('button', {
      name: '关闭目录编辑'
    });
    const bindingSaveButton = within(dialog).getByRole('button', {
      name: /保存/
    });

    expect(bindingCancelButton).toBeInTheDocument();
    expect(bindingSaveButton).toBeInTheDocument();
    expect(
      bindingCancelButton.compareDocumentPosition(bindingSaveButton) &
        Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      within(modalScrollBody).queryByRole('button', { name: /取\s*消/ })
    ).not.toBeInTheDocument();
    expect(
      within(modalScrollBody).queryByRole('button', { name: /保存/ })
    ).not.toBeInTheDocument();
  });

  test('renders draft group nodes with the display name and short description in the tree', async () => {
    await renderPanelWithMountedTool({ includeBinding: false });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    await expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    const rootNode = rootLabel.closest('.ant-tree-node-content-wrapper');
    expect(rootNode).toBeInstanceOf(HTMLElement);
    await click(rootLabel);
    await click(within(dialog).getByRole('button', { name: '新建分组' }));
    await waitFor(() => {
      expect(
        dialog.querySelector('.mcp-management__directory-editor-status')
      ).toHaveTextContent('未保存');
    });
    await change(within(dialog).getByLabelText('显示名称'), {
      target: { value: '客户运营' }
    });
    await change(within(dialog).getByLabelText('路径'), {
      target: { value: 'customer_ops' }
    });
    await change(within(dialog).getByLabelText('简短描述'), {
      target: { value: 'Tools for customer operations' }
    });

    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent('客户运营');
    });
    const currentTree = within(dialog).getByRole('tree');
    expect(currentTree).toHaveTextContent('Tools for customer operations');
    expect(currentTree).not.toHaveTextContent('客户运营 /customer_ops');
  });

  test('accepts Unicode display names but rejects unsafe group paths', async () => {
    await renderPanelWithMountedTool({ includeBinding: false });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    await click(within(within(dialog).getByRole('tree')).getByText('/'));
    await click(within(dialog).getByRole('button', { name: '新建分组' }));
    await change(within(dialog).getByLabelText('显示名称'), {
      target: { value: '后台设置' }
    });
    await change(within(dialog).getByLabelText('路径'), {
      target: { value: '后台设置' }
    });
    await click(within(dialog).getByRole('button', { name: /保存/ }));

    expect(
      await within(dialog).findByText(
        '当前路径段只能包含英文字母、数字、下划线和连字符'
      )
    ).toBeInTheDocument();
    expect(mcpManagementApi.upsertSettingsMcpGroup).not.toHaveBeenCalled();
  });

  test('hides the directory tree drag handle while keeping nodes draggable', async () => {
    await renderPanelWithMountedTool();

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const tree = within(
      screen.getByRole('dialog', { name: '目录编辑' })
    ).getByRole('tree');

    expect(
      tree.querySelector('.ant-tree-draggable-icon')
    ).not.toBeInTheDocument();
  });

  test('renders draft binding nodes with the tool id and short description in the tree', async () => {
    await renderPanelWithMountedTool({ includeBinding: false });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    await click(rootLabel);
    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    await waitFor(() => {
      expect(within(dialog).getByLabelText('tool_id')).toBeInTheDocument();
    });
    await mouseDown(
      within(dialog).getByRole('combobox', { name: 'tool_id' })
    );
    await selectAntdOption('Search customer');

    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent(
        'search_customer'
      );
    });
    const currentTree = within(dialog).getByRole('tree');
    expect(currentTree).toHaveTextContent('Find matching customers');
    expect(currentTree).not.toHaveTextContent(
      'Search customer search_customer'
    );
  });

  test('replaces an unsaved Tool draft when starting a group creation session', async () => {
    await renderPanelWithMountedTool({ includeBinding: false });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    await click(rootLabel);
    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent('未命名 Tool');
    });

    await click(within(dialog).getByRole('button', { name: '新建分组' }));

    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).not.toHaveTextContent(
        '未命名 Tool'
      );
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue('');
    });
  });

  test('moves the single Tool draft when adding under another group', async () => {
    await renderPanelWithMountedTool({ includeBinding: false, includeGroup: true });

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });
    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    const tree = within(dialog).getByRole('tree');
    await expandTreeRootIfCollapsed(tree);

    const rootLabel = within(within(dialog).getByRole('tree')).getByText('/');
    await click(rootLabel);
    await waitFor(() => {
      expect(rootLabel.closest('.ant-tree-node-content-wrapper')).toHaveClass(
        'ant-tree-node-selected'
      );
    });
    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    await waitFor(() => {
      expect(within(dialog).getByRole('tree')).toHaveTextContent('未命名 Tool');
    });

    const groupLabel = within(dialog).getByText('ops');
    await click(groupLabel);
    await waitFor(() => {
      expect(
        within(dialog)
          .getByText('ops')
          .closest('.ant-tree-node-content-wrapper')
      ).toHaveClass('ant-tree-node-selected');
    });
    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    await waitFor(() => {
      expect(
        within(within(dialog).getByRole('tree')).getAllByText('未命名 Tool')
      ).toHaveLength(1);
      expect(within(dialog).getByLabelText('挂载路径')).toHaveValue('/ops');
    });
  });

  test('does not carry a draft group path into the binding mount path', async () => {
    await renderPanelWithMountedTool();

    await click(screen.getByRole('tab', { name: 'MCP 实例' }));
    const instancesPanel = screen.getByRole('tabpanel', { name: 'MCP 实例' });

    await click(
      within(instancesPanel).getByRole('button', { name: '目录编辑' })
    );

    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    await change(within(dialog).getByLabelText('路径'), {
      target: { value: '/ops/customer' }
    });

    expect(within(dialog).getByLabelText('路径')).toHaveValue('/ops/customer');

    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));

    await waitFor(() => {
      expect(within(dialog).getByLabelText('挂载路径')).toHaveValue('/');
    });
  });
});
