import {
  FileAddOutlined,
  FolderAddOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { App, Button, Dropdown, Input, Space } from 'antd';
import { useState } from 'react';

import type { FrontstagePageTreeNode } from '../features/frontstage/api/page-tree';
import { useFrontstagePageTreeMutations } from '../features/frontstage/hooks/use-frontstage-page-tree-mutations';
import '../features/frontstage/components/frontstage-add-action.css';

function appendRank(nodes: FrontstagePageTreeNode[]): string {
  return String((nodes.length + 1) * 1000).padStart(6, '0');
}

function randomSlug(): string {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz0123456789';
  const bytes = crypto.getRandomValues(new Uint8Array(7));
  return `p${Array.from(bytes, (byte) => alphabet[byte % alphabet.length]).join('')}`;
}

function TopbarNodeCreateFields({
  initialSlug,
  onTitleChange,
  onSlugChange
}: {
  initialSlug: string;
  onTitleChange: (value: string) => void;
  onSlugChange: (value: string) => void;
}) {
  const [slug, setSlug] = useState(initialSlug);

  const updateSlug = (value: string) => {
    setSlug(value);
    onSlugChange(value);
  };

  return (
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      <Input
        aria-label="显示名称"
        placeholder="请输入显示名称"
        onChange={(event) => onTitleChange(event.target.value)}
      />
      <Space.Compact style={{ width: '100%' }}>
        <Input
          aria-label="访问路径"
          prefix="/"
          value={slug}
          onChange={(event) => updateSlug(event.target.value)}
        />
        <Button
          aria-label="刷新访问路径"
          onClick={() => updateSlug(randomSlug())}
        >
          刷新
        </Button>
      </Space.Compact>
    </Space>
  );
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

  const promptCreate = (kind: 'group' | 'page') => {
    let title = '';
    let slug = randomSlug();
    const content = (
      <TopbarNodeCreateFields
        initialSlug={slug}
        onTitleChange={(value) => {
          title = value;
        }}
        onSlugChange={(value) => {
          slug = value;
        }}
      />
    );
    modal.confirm({
      title: kind === 'group' ? '新增顶部空间' : '新增顶部页面',
      content,
      okText: '创建',
      cancelText: '取消',
      onOk: async () => {
        const normalizedTitle = title.trim();
        if (!normalizedTitle) {
          void message.warning('显示名称不能为空');
          throw new Error('empty title');
        }
        await run(() =>
          kind === 'group'
            ? mutations.createGroup({
                title: normalizedTitle,
                slug,
                parentId: null,
                rank: appendRank(topbarNodes),
                placement: 'topbar'
              })
            : mutations.createPage({
                title: normalizedTitle,
                slug,
                parentId: null,
                rank: appendRank(topbarNodes),
                placement: 'topbar'
              })
        );
      }
    });
  };

  return (
    <Space className="app-shell-topbar-designer" size={2}>
      <Dropdown
        menu={{
          items: [
            {
              key: 'add-group',
              icon: <FolderAddOutlined />,
              label: '新增菜单',
              onClick: () => promptCreate('group')
            },
            {
              key: 'add-page',
              icon: <FileAddOutlined />,
              label: '新增页面',
              onClick: () => promptCreate('page')
            }
          ]
        }}
        trigger={['click']}
      >
        <Button
          aria-label="添加菜单"
          className="app-shell-topbar-designer__button frontstage-add-action-button frontstage-add-action-button--compact"
          disabled={pending || mutations.isPending}
          icon={<PlusOutlined />}
          size="small"
        >
          添加菜单
        </Button>
      </Dropdown>
    </Space>
  );
}
