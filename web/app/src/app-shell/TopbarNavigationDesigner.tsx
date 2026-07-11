import {
  DeleteOutlined,
  DownOutlined,
  EditOutlined,
  FileAddOutlined,
  FolderAddOutlined,
  MoreOutlined,
  PlusOutlined,
  UpOutlined
} from '@ant-design/icons';
import { App, Button, Dropdown, Input, Space } from 'antd';
import type { MenuProps } from 'antd';
import { useState } from 'react';

import type { FrontstagePageTreeNode } from '../features/frontstage/api/page-tree';
import { useFrontstagePageTreeMutations } from '../features/frontstage/hooks/use-frontstage-page-tree-mutations';

function appendRank(nodes: FrontstagePageTreeNode[]): string {
  return String((nodes.length + 1) * 1000).padStart(6, '0');
}

function moveRank(index: number, direction: -1 | 1): string {
  return direction < 0
    ? index === 1
      ? '000000'
      : String((index - 1) * 1000 + 500).padStart(6, '0')
    : String((index + 2) * 1000 + 500).padStart(6, '0');
}

export function TopbarNavigationDesigner({
  workspaceId,
  nodes
}: {
  workspaceId: string;
  nodes: FrontstagePageTreeNode[];
}) {
  const { modal, message } = App.useApp();
  const [pending, setPending] = useState(false);
  const mutations = useFrontstagePageTreeMutations(workspaceId);
  const topbarNodes = nodes.filter((node) => node.placement === 'topbar');

  const run = async (operation: () => Promise<unknown>) => {
    setPending(true);
    try {
      await operation();
    } catch {
      void message.error('顶部导航操作失败');
    } finally {
      setPending(false);
    }
  };

  const promptTitle = (
    title: string,
    initialValue: string,
    onConfirm: (value: string) => Promise<unknown>
  ) => {
    let value = initialValue;
    modal.confirm({
      title,
      content: (
        <Input
          autoFocus
          defaultValue={initialValue}
          placeholder="请输入名称"
          onChange={(event) => {
            value = event.target.value;
          }}
        />
      ),
      okText: '确认',
      cancelText: '取消',
      onOk: async () => {
        const normalized = value.trim();
        if (!normalized) {
          void message.warning('名称不能为空');
          throw new Error('empty title');
        }
        await run(() => onConfirm(normalized));
      }
    });
  };

  const createNode = (kind: 'group' | 'page', parentId: string | null) => {
    const siblings = parentId
      ? (topbarNodes.find((node) => node.id === parentId)?.children ?? [])
      : topbarNodes;
    promptTitle(
      kind === 'group' ? '新增顶部菜单' : '新增顶部页面',
      '',
      (title) =>
        kind === 'group'
          ? mutations.createGroup({
              title,
              parentId: null,
              rank: appendRank(siblings),
              placement: 'topbar'
            })
          : mutations.createPage({
              title,
              parentId,
              rank: appendRank(siblings),
              placement: 'topbar'
            })
    );
  };

  const renameNode = (node: FrontstagePageTreeNode) => {
    promptTitle('重命名顶部导航', node.title ?? '', (title) =>
      mutations.renameNode(node.id, {
        title,
        icon: node.icon,
        tooltip: node.tooltip
      })
    );
  };

  const deleteNode = (node: FrontstagePageTreeNode) => {
    modal.confirm({
      title: `删除“${node.title?.trim() || '未命名导航'}”`,
      content:
        node.kind === 'group' && node.children.length > 0
          ? '该菜单及其子页面将一并删除，且无法撤销。'
          : '删除后无法撤销。',
      okText: '删除',
      okButtonProps: { danger: true },
      cancelText: '取消',
      onOk: () => run(() => mutations.deleteNode(node.id))
    });
  };

  const nodeActions = (
    node: FrontstagePageTreeNode,
    siblings: FrontstagePageTreeNode[],
    parentId: string | null
  ): MenuProps['items'] => {
    const index = siblings.findIndex((sibling) => sibling.id === node.id);

    return [
      ...(node.kind === 'group'
        ? [
            {
              key: `${node.id}-add-page`,
              icon: <FileAddOutlined />,
              label: '新增子页面',
              onClick: () => createNode('page', node.id)
            }
          ]
        : []),
      {
        key: `${node.id}-rename`,
        icon: <EditOutlined />,
        label: '重命名',
        onClick: () => renameNode(node)
      },
      {
        key: `${node.id}-up`,
        icon: <UpOutlined />,
        label: '上移',
        disabled: index <= 0,
        onClick: () =>
          void run(() =>
            mutations.moveNode(node.id, {
              parentId,
              rank: moveRank(index, -1)
            })
          )
      },
      {
        key: `${node.id}-down`,
        icon: <DownOutlined />,
        label: '下移',
        disabled: index === siblings.length - 1,
        onClick: () =>
          void run(() =>
            mutations.moveNode(node.id, {
              parentId,
              rank: moveRank(index, 1)
            })
          )
      },
      { type: 'divider' },
      {
        key: `${node.id}-delete`,
        danger: true,
        icon: <DeleteOutlined />,
        label: '删除',
        onClick: () => deleteNode(node)
      }
    ];
  };

  const manageItems: MenuProps['items'] = topbarNodes.map((node) => ({
    key: node.id,
    label: node.title?.trim() || '未命名导航',
    children: [
      ...(node.kind === 'group'
        ? node.children.map((child) => ({
            key: child.id,
            label: child.title?.trim() || '未命名页面',
            children: nodeActions(child, node.children, node.id)
          }))
        : []),
      ...(node.kind === 'group' && node.children.length > 0
        ? [{ type: 'divider' as const }]
        : []),
      ...(nodeActions(node, topbarNodes, null) ?? [])
    ]
  }));

  return (
    <Space className="app-shell-topbar-designer" size={2}>
      <Dropdown
        menu={{
          items: [
            {
              key: 'add-group',
              icon: <FolderAddOutlined />,
              label: '新增菜单',
              onClick: () => createNode('group', null)
            },
            {
              key: 'add-page',
              icon: <FileAddOutlined />,
              label: '新增页面',
              onClick: () => createNode('page', null)
            }
          ]
        }}
        trigger={['click']}
      >
        <Button
          aria-label="新增顶部导航"
          className="app-shell-topbar-designer__button"
          disabled={pending || mutations.isPending}
          icon={<PlusOutlined />}
          type="text"
        />
      </Dropdown>
      {topbarNodes.length > 0 ? (
        <Dropdown menu={{ items: manageItems }} trigger={['click']}>
          <Button
            aria-label="管理顶部导航"
            className="app-shell-topbar-designer__button"
            disabled={pending || mutations.isPending}
            icon={<MoreOutlined />}
            type="text"
          />
        </Dropdown>
      ) : null}
    </Space>
  );
}
