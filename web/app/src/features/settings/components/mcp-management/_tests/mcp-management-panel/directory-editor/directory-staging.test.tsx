import type { ConsoleMcpInstanceDirectory } from '@1flowbase/api-client';
import { act, screen, waitFor, within } from '@testing-library/react';
import { describe, expect, test } from 'vitest';
import {
  change,
  click,
  expandTreeRootIfCollapsed,
  grantDirectoryPermissions,
  mcpManagementApi,
  mouseDown,
  renderPanelWithMountedTool,
  selectAntdOption,
  setupDirectoryEditorFixture,
  waitForRequests
} from './fixture';
import { holdNextDirectoryReplace } from '../fixtures/directory-snapshot';

async function openDirectory(
  options: Parameters<typeof renderPanelWithMountedTool>[0] = {
    includeGroup: true
  }
) {
  await renderPanelWithMountedTool(options);
  await click(screen.getByRole('tab', { name: 'MCP 实例' }));
  await click(screen.getAllByRole('button', { name: '目录编辑' })[0]!);
  const dialog = screen.getByRole('dialog', { name: '目录编辑' });
  await waitFor(() =>
    expect(
      mcpManagementApi.fetchSettingsMcpInstanceDirectory
    ).toHaveBeenCalledWith('ops_mcp')
  );
  const snapshot: ConsoleMcpInstanceDirectory =
    await mcpManagementApi.fetchSettingsMcpInstanceDirectory.mock.results[0]!
      .value;
  await expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
  return { dialog, snapshot };
}

async function selectGroup(dialog: HTMLElement, name = 'ops') {
  await click(within(within(dialog).getByRole('tree')).getByText(name));
}

async function editGroup(dialog: HTMLElement, name: string) {
  await click(within(dialog).getByRole('button', { name: '编辑节点' }));
  await change(within(dialog).getByLabelText('显示名称'), {
    target: { value: name }
  });
}

async function saveNode(dialog: HTMLElement) {
  await click(within(dialog).getByRole('button', { name: '保存节点' }));
  await waitFor(() =>
    expect(
      within(dialog).queryByRole('button', { name: '保存节点' })
    ).not.toBeInTheDocument()
  );
}

async function waitForDirectorySaveReady(dialog: HTMLElement) {
  await waitForRequests();
  return waitFor(() => {
    // Ant Design keeps an inner loading state after the HTTP/mutation callbacks finish.
    const save = within(dialog).getByRole('button', { name: '保存目录' });
    expect(save).toBeEnabled();
    expect(save).not.toHaveClass('ant-btn-loading');
    expect(
      within(dialog).getByRole('button', { name: '关闭目录编辑' })
    ).toBeEnabled();
    return save;
  });
}

async function expandMountAncestors(
  dialog: HTMLElement,
  groupPath: string,
  snapshot: ConsoleMcpInstanceDirectory
) {
  const ancestorPaths = ['/'];
  let path = '';
  for (const segment of groupPath.split('/').filter(Boolean)) {
    path += `/${segment}`;
    ancestorPaths.push(path);
  }
  for (const ancestorPath of ancestorPaths) {
    const title =
      ancestorPath === '/'
        ? '/'
        : snapshot.groups.find((group) => group.path === ancestorPath)
            ?.display_name || ancestorPath.split('/').at(-1)!;
    const currentItem = () => {
      const tree = within(dialog).getByRole('tree');
      const item = within(tree).getByTitle(title).closest('[role="treeitem"]');
      expect(item).toBeInstanceOf(HTMLElement);
      expect(item).toBeVisible();
      return item as HTMLElement;
    };
    const item = await waitFor(currentItem);
    if (item.getAttribute('aria-expanded') === 'false') {
      const switcher = Array.from(currentItem().children).find((child) =>
        child.classList.contains('ant-tree-switcher')
      );
      expect(switcher).toBeInstanceOf(HTMLElement);
      await click(switcher!);
    }
    await waitFor(() =>
      expect(currentItem()).toHaveAttribute('aria-expanded', 'true')
    );
  }
}

function expectNoImmediateWrites() {
  for (const method of [
    mcpManagementApi.upsertSettingsMcpGroup,
    mcpManagementApi.moveSettingsMcpGroup,
    mcpManagementApi.deleteSettingsMcpGroup,
    mcpManagementApi.createSettingsMcpToolBinding,
    mcpManagementApi.updateSettingsMcpToolBinding,
    mcpManagementApi.deleteSettingsMcpToolBinding
  ])
    expect(method).not.toHaveBeenCalled();
}

function expectedGroup(
  snapshot: ConsoleMcpInstanceDirectory,
  displayName: string
) {
  const group = snapshot.groups[0]!;
  return {
    id: group.id,
    path: group.path,
    display_name: displayName,
    description_short: group.description_short,
    enabled: group.enabled,
    sort_order: group.sort_order
  };
}

function expectedBindings(snapshot: ConsoleMcpInstanceDirectory) {
  return snapshot.bindings.map(
    ({ id, tool_id, group_path, display_alias, visible, sort_order }) => ({
      id,
      tool_id,
      group_path,
      display_alias,
      visible,
      sort_order
    })
  );
}

describe('instance directory confirmed tree and node form', () => {
  setupDirectoryEditorFixture();

  test('AC-S1 node cancel restores the last confirmed draft, not the original server value', async () => {
    const { dialog } = await openDirectory();
    await selectGroup(dialog);
    expect(within(dialog).getByRole('switch', { name: '启用' })).toBeDisabled();
    expect(
      within(dialog).queryByRole('combobox', { name: '实例名称' })
    ).not.toBeInTheDocument();
    expect(
      within(dialog).queryByRole('textbox', { name: '显示名称' })
    ).not.toBeInTheDocument();

    await editGroup(dialog, 'Confirmed operations');
    await saveNode(dialog);
    expect(within(dialog).getByRole('tree')).toHaveTextContent(
      'Confirmed operations'
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
    expectNoImmediateWrites();

    await editGroup(dialog, 'Discard this node form');
    await click(within(dialog).getByRole('button', { name: '取消节点编辑' }));
    await click(within(dialog).getByRole('button', { name: '编辑节点' }));
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
      'Confirmed operations'
    );
    expectNoImmediateWrites();
  });

  test('AC-S2/S3 footer save submits exactly the confirmed graph and advances the expected snapshot', async () => {
    const { dialog, snapshot } = await openDirectory();
    await selectGroup(dialog);
    await editGroup(dialog, 'Confirmed operations');
    await saveNode(dialog);
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(1)
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).toHaveBeenLastCalledWith(
      'ops_mcp',
      {
        expected_snapshot: snapshot,
        groups: [expectedGroup(snapshot, 'Confirmed operations')],
        bindings: expectedBindings(snapshot)
      },
      'csrf-123'
    );
    expectNoImmediateWrites();
    const committed =
      await mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock
        .results[0]!.value;
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(2)
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[1]![1]
    ).toEqual({
      expected_snapshot: committed,
      groups: [expectedGroup(snapshot, 'Confirmed operations')],
      bindings: expectedBindings(snapshot)
    });
    expect(dialog).toBeInTheDocument();
  });

  test('AC-S4 a deferred footer response advances the baseline without overwriting nodes confirmed during the request', async () => {
    const { dialog, snapshot } = await openDirectory();
    await selectGroup(dialog);
    await editGroup(dialog, 'Submitted operations');
    await saveNode(dialog);
    const response = holdNextDirectoryReplace(mcpManagementApi);
    try {
      await click(within(dialog).getByRole('button', { name: '保存目录' }));
      await waitFor(() =>
        expect(
          mcpManagementApi.replaceSettingsMcpInstanceDirectory
        ).toHaveBeenCalledTimes(1)
      );
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[0]![1]
      ).toEqual({
        expected_snapshot: snapshot,
        groups: [expectedGroup(snapshot, 'Submitted operations')],
        bindings: expectedBindings(snapshot)
      });
      await editGroup(dialog, 'Confirmed while saving');
      await saveNode(dialog);
      expect(within(dialog).getByRole('tree')).toHaveTextContent(
        'Confirmed while saving'
      );
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(1);
      expectNoImmediateWrites();
      await act(async () => {
        response.release();
        await mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock
          .results[0]!.value;
      });
      await waitForRequests();
      const committed: ConsoleMcpInstanceDirectory =
        await mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock
          .results[0]!.value;
      expect(committed.groups[0]!.display_name).toBe('Submitted operations');
      expect(within(dialog).getByRole('tree')).toHaveTextContent(
        'Confirmed while saving'
      );
      expect(
        within(dialog).queryByRole('textbox', { name: '显示名称' })
      ).not.toBeInTheDocument();
      await click(await waitForDirectorySaveReady(dialog));
      await waitForRequests();
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(2);
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[1]![1]
      ).toEqual({
        expected_snapshot: committed,
        groups: [expectedGroup(snapshot, 'Confirmed while saving')],
        bindings: expectedBindings(snapshot)
      });
      expectNoImmediateWrites();
    } finally {
      response.release();
    }
  });

  test.each(['cancel', 'save'] as const)(
    'AC-S4 footer save keeps an unconfirmed form; later node %s uses the correct value',
    async (nextAction) => {
      const { dialog, snapshot } = await openDirectory();
      await selectGroup(dialog);
      await editGroup(dialog, 'Still typing');
      const saveDirectory = within(dialog).getByRole('button', {
        name: '保存目录'
      });
      expect(saveDirectory).toBeEnabled();
      await click(saveDirectory);
      await waitFor(() =>
        expect(
          mcpManagementApi.replaceSettingsMcpInstanceDirectory
        ).toHaveBeenCalledTimes(1)
      );
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenLastCalledWith(
        'ops_mcp',
        {
          expected_snapshot: snapshot,
          groups: [expectedGroup(snapshot, 'ops')],
          bindings: expectedBindings(snapshot)
        },
        'csrf-123'
      );
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
        'Still typing'
      );
      expect(
        within(dialog).getByRole('button', { name: '保存节点' })
      ).toBeEnabled();
      const committed =
        await mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock
          .results[0]!.value;

      if (nextAction === 'cancel') {
        await click(
          within(dialog).getByRole('button', { name: '取消节点编辑' })
        );
        await click(within(dialog).getByRole('button', { name: '编辑节点' }));
        expect(within(dialog).getByLabelText('显示名称')).toHaveValue('ops');
      } else {
        await saveNode(dialog);
        expect(within(dialog).getByRole('tree')).toHaveTextContent(
          'Still typing'
        );
      }
      await click(saveDirectory);
      await waitFor(() =>
        expect(
          mcpManagementApi.replaceSettingsMcpInstanceDirectory
        ).toHaveBeenCalledTimes(2)
      );
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[1]![1]
      ).toEqual({
        expected_snapshot: committed,
        groups: [
          expectedGroup(
            snapshot,
            nextAction === 'save' ? 'Still typing' : 'ops'
          )
        ],
        bindings: expectedBindings(snapshot)
      });
      expectNoImmediateWrites();
    }
  );

  test.each([
    ['network failure', new Error('Directory unavailable')],
    [
      '409 conflict',
      Object.assign(new Error('Directory changed; reload before retrying'), {
        status: 409
      })
    ]
  ] as const)(
    'AC-S3/S4 %s preserves both confirmed draft and current form for a retry',
    async (_label, error) => {
      const { dialog, snapshot } = await openDirectory();
      await selectGroup(dialog);
      await editGroup(dialog, 'Confirmed operations');
      await saveNode(dialog);
      await editGroup(dialog, 'Unconfirmed operations');
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mockRejectedValueOnce(
        error
      );
      await click(within(dialog).getByRole('button', { name: '保存目录' }));
      expect(await screen.findByText(error.message)).toBeInTheDocument();
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
        'Unconfirmed operations'
      );
      const failedBody =
        mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[0]![1];
      expect(failedBody).toEqual({
        expected_snapshot: snapshot,
        groups: [expectedGroup(snapshot, 'Confirmed operations')],
        bindings: expectedBindings(snapshot)
      });
      await click(within(dialog).getByRole('button', { name: '取消节点编辑' }));
      await click(within(dialog).getByRole('button', { name: '编辑节点' }));
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
        'Confirmed operations'
      );
      await click(within(dialog).getByRole('button', { name: '保存目录' }));
      await waitFor(() =>
        expect(
          mcpManagementApi.replaceSettingsMcpInstanceDirectory
        ).toHaveBeenCalledTimes(2)
      );
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[1]![1]
      ).toEqual(failedBody);
      expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
        'Confirmed operations'
      );
      expectNoImmediateWrites();
    }
  );

  test('AC-S3 footer cancel confirms and discards the whole draft without any writes', async () => {
    const { dialog } = await openDirectory();
    await selectGroup(dialog);
    await editGroup(dialog, 'Confirmed but not persisted');
    await saveNode(dialog);
    await editGroup(dialog, 'Unconfirmed text');
    await click(within(dialog).getByRole('button', { name: '关闭目录编辑' }));
    await click(screen.getByRole('button', { name: '继续编辑' }));
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue(
      'Unconfirmed text'
    );
    await click(within(dialog).getByRole('button', { name: '关闭目录编辑' }));
    await click(screen.getByRole('button', { name: '放弃更改' }));
    await waitFor(() => expect(dialog).not.toBeInTheDocument());
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
    expectNoImmediateWrites();
    await click(screen.getByRole('button', { name: '目录编辑' }));
    const reopened = screen.getByRole('dialog', { name: '目录编辑' });
    await expandTreeRootIfCollapsed(within(reopened).getByRole('tree'));
    await selectGroup(reopened);
    await click(within(reopened).getByRole('button', { name: '编辑节点' }));
    expect(within(reopened).getByLabelText('显示名称')).toHaveValue('ops');
  });

  test('AC-S2 creates a group in the draft and retains the returned server identity for the next commit', async () => {
    const { dialog, snapshot } = await openDirectory({
      includeBinding: false,
      includeGroup: true
    });
    await selectGroup(dialog);
    await click(within(dialog).getByRole('button', { name: '新建分组' }));
    expect(
      within(dialog).getByRole('button', { name: '保存节点' })
    ).toBeEnabled();
    await change(within(dialog).getByLabelText('路径'), {
      target: { value: 'customer_ops' }
    });
    await change(within(dialog).getByLabelText('显示名称'), {
      target: { value: '客户运营' }
    });
    await saveNode(dialog);
    const tree = within(dialog).getByRole('tree');
    expect(tree).toHaveTextContent('客户运营');
    expect(
      within(tree)
        .getByText('客户运营')
        .closest('.ant-tree-node-content-wrapper')
    ).toHaveClass('ant-tree-node-selected');
    expectNoImmediateWrites();
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(1)
    );
    const staged =
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[0]![1];
    expect(staged).toEqual({
      expected_snapshot: snapshot,
      groups: [
        expectedGroup(snapshot, 'ops'),
        {
          path: '/ops/customer_ops',
          display_name: '客户运营',
          description_short: null,
          enabled: true,
          sort_order: 0
        }
      ],
      bindings: []
    });
    const committed: ConsoleMcpInstanceDirectory =
      await mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock
        .results[0]!.value;
    const savedGroup = committed.groups.find(
      (group) => group.path === '/ops/customer_ops'
    )!;
    await click(within(dialog).getByRole('button', { name: '编辑节点' }));
    await change(within(dialog).getByLabelText('显示名称'), {
      target: { value: 'Updated customers' }
    });
    await saveNode(dialog);
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(2)
    );
    const secondBody =
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[1]![1];
    expect(secondBody.expected_snapshot).toEqual(committed);
    expect(secondBody.groups).toEqual([
      expectedGroup(snapshot, 'ops'),
      {
        id: savedGroup.id,
        path: '/ops/customer_ops',
        display_name: 'Updated customers',
        description_short: null,
        enabled: true,
        sort_order: 0
      }
    ]);
    expectNoImmediateWrites();
  });

  test('AC-S2 new mount starts editing, validates required Tool, and enters the confirmed graph without a client ID', async () => {
    const { dialog, snapshot } = await openDirectory({
      includeBinding: false,
      includeGroup: true
    });
    await selectGroup(dialog);
    await click(within(dialog).getByRole('button', { name: '挂载 Tool' }));
    expect(
      within(dialog).getByRole('button', { name: '保存节点' })
    ).toBeEnabled();
    expect(within(dialog).queryByLabelText('显示别名')).not.toBeInTheDocument();
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(1)
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[0]![1]
    ).toEqual({
      expected_snapshot: snapshot,
      groups: [expectedGroup(snapshot, 'ops')],
      bindings: []
    });
    expect(
      within(dialog).getByRole('combobox', { name: '选择 Tool' })
    ).toBeInTheDocument();
    expect(within(dialog).getByLabelText('选择 Tool')).not.toHaveAttribute(
      'aria-invalid',
      'true'
    );
    await click(within(dialog).getByRole('button', { name: '保存节点' }));
    await waitFor(() =>
      expect(within(dialog).getByLabelText('选择 Tool')).toHaveAttribute(
        'aria-invalid',
        'true'
      )
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).toHaveBeenCalledTimes(1);
    await mouseDown(
      within(dialog).getByRole('combobox', { name: '选择 Tool' })
    );
    await selectAntdOption('Search customer');
    await saveNode(dialog);
    expect(
      within(dialog).queryByRole('combobox', { name: '编辑 Tool 挂载' })
    ).not.toBeInTheDocument();
    expectNoImmediateWrites();
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(2)
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[1]![1]
    ).toEqual({
      expected_snapshot: snapshot,
      groups: [expectedGroup(snapshot, 'ops')],
      bindings: [
        {
          tool_id: 'search_customer',
          group_path: '/ops',
          display_alias: null,
          visible: true,
          sort_order: 0
        }
      ]
    });
  });

  test('AC-S2 deleting a mount changes only the draft until footer save', async () => {
    const groupPath = '/ops/customer';
    const { dialog, snapshot } = await openDirectory({
      includeGroup: true,
      bindingGroupPath: groupPath
    });
    await expandMountAncestors(dialog, groupPath, snapshot);
    const currentBindingItem = () => {
      const tree = within(dialog).getByRole('tree');
      return within(tree)
        .getByText('search_customer')
        .closest('[role="treeitem"]') as HTMLElement;
    };
    await waitFor(() => expect(currentBindingItem()).toBeVisible());
    await click(within(currentBindingItem()).getByText('search_customer'));
    await waitFor(() =>
      expect(
        within(dialog).getByRole('group', { name: '挂载路径' })
      ).toHaveTextContent(`${groupPath}/search_customer`)
    );
    // Selecting a binding changes editor mode and can remount the tree; use its current row.
    await waitFor(() => expect(currentBindingItem()).toBeVisible());
    await click(
      within(currentBindingItem()).getByRole('button', { name: 'Delete' })
    );
    await click(screen.getByRole('button', { name: /^确\s*定$/ }));
    await waitFor(() =>
      expect(within(dialog).getByRole('tree')).not.toHaveTextContent(
        'search_customer'
      )
    );
    expectNoImmediateWrites();
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
    await click(within(dialog).getByRole('button', { name: '保存目录' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.replaceSettingsMcpInstanceDirectory
      ).toHaveBeenCalledTimes(1)
    );
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory.mock.calls[0]![1]
    ).toEqual({
      expected_snapshot: snapshot,
      groups: [expectedGroup(snapshot, 'ops')],
      bindings: []
    });
  });

  test('AC-S5 legacy item grants do not imply the directory view operation', async () => {
    await renderPanelWithMountedTool({ includeGroup: true });
    grantDirectoryPermissions([
      'mcp.instances.view',
      'mcp.groups.upsert',
      'mcp.tool_bindings.update'
    ]);
    const directoryButton = screen.getByRole('button', { name: '目录编辑' });
    expect(directoryButton).toBeDisabled();
    await click(directoryButton);
    expect(
      screen.queryByRole('dialog', { name: '目录编辑' })
    ).not.toBeInTheDocument();
    expect(
      mcpManagementApi.fetchSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
    expectNoImmediateWrites();
  });

  test('AC-S5 directory view grant permits inspection but prevents all draft writes', async () => {
    await renderPanelWithMountedTool({ includeGroup: true });
    grantDirectoryPermissions(['mcp.directory.view']);
    await click(screen.getByRole('button', { name: '目录编辑' }));
    const dialog = screen.getByRole('dialog', { name: '目录编辑' });
    await expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    await selectGroup(dialog);
    expect(
      within(dialog).getByRole('button', { name: '编辑节点' })
    ).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: '新建分组' })
    ).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: '挂载 Tool' })
    ).toBeDisabled();
    expect(
      within(dialog).getByRole('button', { name: '保存目录' })
    ).toBeDisabled();
    expect(
      within(dialog)
        .queryByRole('tree')
        ?.querySelector('.ant-tree-treenode-draggable')
    ).toBeNull();
    expectNoImmediateWrites();
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
  });

  test('AC-S5 instance switch discards only after confirmation and loads the new instance snapshot', async () => {
    const { dialog } = await openDirectory({
      includeGroup: true,
      secondInstance: true
    });
    await selectGroup(dialog);
    await editGroup(dialog, 'Ops draft');
    await saveNode(dialog);
    await click(within(dialog).getByRole('button', { name: '编辑节点' }));
    await mouseDown(within(dialog).getByRole('combobox', { name: '实例名称' }));
    await selectAntdOption('Other MCP');
    await click(screen.getByRole('button', { name: '继续编辑' }));
    expect(within(dialog).getByLabelText('显示名称')).toHaveValue('Ops draft');
    await mouseDown(within(dialog).getByRole('combobox', { name: '实例名称' }));
    await selectAntdOption('Other MCP');
    await click(screen.getByRole('button', { name: '放弃更改' }));
    await waitFor(() =>
      expect(
        mcpManagementApi.fetchSettingsMcpInstanceDirectory
      ).toHaveBeenLastCalledWith('other_mcp')
    );
    await expandTreeRootIfCollapsed(within(dialog).getByRole('tree'));
    expect(within(dialog).getByRole('tree')).toHaveTextContent(
      'Other directory'
    );
    expect(within(dialog).getByRole('tree')).not.toHaveTextContent('Ops draft');
    expectNoImmediateWrites();
    expect(
      mcpManagementApi.replaceSettingsMcpInstanceDirectory
    ).not.toHaveBeenCalled();
  });
});
