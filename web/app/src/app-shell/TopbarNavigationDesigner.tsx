import {
  ArrowDownOutlined,
  ArrowUpOutlined,
  DeleteOutlined,
  EditOutlined,
  FileAddOutlined,
  FolderAddOutlined,
  MenuOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { App, Button, Dropdown, Form, Space } from 'antd';
import { useEffect, useState, type ReactNode } from 'react';

import type { FrontstagePageTreeNode } from '../features/frontstage/api/page-tree';
import { useFrontstagePageTreeMutations } from '../features/frontstage/hooks/use-frontstage-page-tree-mutations';
import {
  PageTreeFormModal,
  type PageTreeFormDialog,
  type PageTreeFormValues
} from '../features/frontstage/pages/frontstage-page/page-tree-form-modal';
import '../features/frontstage/components/frontstage-add-action.css';
import '../features/frontstage/pages/frontstage-page.css';

function appendRank(nodes: FrontstagePageTreeNode[]): string {
  return String((nodes.length + 1) * 1000).padStart(6, '0');
}

function randomSlug(): string {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz0123456789';
  const bytes = crypto.getRandomValues(new Uint8Array(7));
  return `p${Array.from(bytes, (byte) => alphabet[byte % alphabet.length]).join('')}`;
}

export function TopbarNavigationItemLabel({
  workspaceId,
  node,
  siblings,
  children
}: {
  workspaceId: string;
  node: FrontstagePageTreeNode;
  siblings: FrontstagePageTreeNode[];
  children: ReactNode;
}) {
  const { modal } = App.useApp();
  const [form] = Form.useForm<PageTreeFormValues>();
  const [dialog, setDialog] = useState<PageTreeFormDialog | null>(null);
  const [iconPickerOpen, setIconPickerOpen] = useState(false);
  const mutations = useFrontstagePageTreeMutations(workspaceId);
  const index = siblings.findIndex((candidate) => candidate.id === node.id);

  useEffect(() => {
    if (dialog?.kind !== 'rename') return;
    form.setFieldsValue({
      title: dialog.initialTitle,
      slug: node.slug ?? '',
      icon: dialog.initialIcon,
      tooltip: dialog.initialTooltip
    });
  }, [dialog, form, node.slug]);

  const openEdit = () => {
    setDialog({
      kind: 'rename',
      nodeId: node.id,
      title: '编辑顶部栏目',
      initialTitle: node.title ?? '',
      initialIcon: node.icon ?? '',
      initialTooltip: node.tooltip ?? '',
      initialSlug: node.slug ?? '',
      showSlug: true
    });
  };

  const submitEdit = async () => {
    if (dialog?.kind !== 'rename') return;
    const values = await form.validateFields();
    await mutations.renameNode(node.id, {
      title: values.title?.trim() ?? '',
      slug: values.slug?.trim() ?? '',
      icon: values.icon ?? null,
      tooltip: values.tooltip ?? null
    });
    setDialog(null);
  };

  const move = (direction: -1 | 1) => {
    const rank =
      direction < 0
        ? index === 0
          ? '000000'
          : String(index * 1000 + 500).padStart(6, '0')
        : String((index + 1) * 1000 + 500).padStart(6, '0');
    void mutations.moveNode(node.id, { parentId: null, rank });
  };

  return (
    <span className="app-shell-dynamic-nav-item">
      {children}
      <span className="app-shell-dynamic-nav-item__actions">
        <Dropdown
          menu={{
            items: [
              {
                key: 'up',
                label: '上移',
                icon: <ArrowUpOutlined />,
                disabled: index <= 0,
                onClick: () => move(-1)
              },
              {
                key: 'down',
                label: '下移',
                icon: <ArrowDownOutlined />,
                disabled: index >= siblings.length - 1,
                onClick: () => move(1)
              }
            ]
          }}
          trigger={['click']}
        >
          <Button
            aria-label={`排序${node.title ?? '顶部栏目'}`}
            className="app-shell-dynamic-nav-item__action"
            icon={<PlusOutlined />}
            size="small"
            type="text"
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
          />
        </Dropdown>
        <Dropdown
          menu={{
            items: [
              {
                key: 'edit',
                label: '编辑',
                icon: <EditOutlined />,
                onClick: openEdit
              },
              { type: 'divider' },
              {
                key: 'delete',
                label: '删除',
                danger: true,
                icon: <DeleteOutlined />,
                onClick: () =>
                  modal.confirm({
                    title: `删除“${node.title?.trim() || '未命名栏目'}”`,
                    content: '删除后无法撤销。',
                    okText: '删除',
                    okButtonProps: { danger: true },
                    cancelText: '取消',
                    onOk: () => mutations.deleteNode(node.id)
                  })
              }
            ]
          }}
          trigger={['click']}
        >
          <Button
            aria-label={`配置${node.title ?? '顶部栏目'}`}
            className="app-shell-dynamic-nav-item__action"
            icon={<MenuOutlined />}
            size="small"
            type="text"
            onClick={(event) => {
              event.preventDefault();
              event.stopPropagation();
            }}
          />
        </Dropdown>
      </span>
      <PageTreeFormModal
        dialog={dialog}
        form={form}
        iconPickerOpen={iconPickerOpen}
        isOperationPending={mutations.isPending}
        onCancel={() => setDialog(null)}
        onIconPickerOpenChange={setIconPickerOpen}
        onRefreshSlug={() => form.setFieldValue('slug', randomSlug())}
        onSubmit={() => {
          void submitEdit();
        }}
      />
    </span>
  );
}

export function TopbarNavigationDesigner({
  workspaceId,
  nodes
}: {
  workspaceId: string;
  nodes: FrontstagePageTreeNode[];
}) {
  const [form] = Form.useForm<PageTreeFormValues>();
  const [dialog, setDialog] = useState<PageTreeFormDialog | null>(null);
  const [iconPickerOpen, setIconPickerOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const mutations = useFrontstagePageTreeMutations(workspaceId);
  const topbarNodes = nodes.filter((node) => node.placement === 'topbar');

  useEffect(() => {
    if (dialog?.kind !== 'create') return;
    form.setFieldsValue({
      title: dialog.initialTitle,
      slug: dialog.initialSlug,
      icon: dialog.initialIcon,
      tooltip: dialog.initialTooltip
    });
  }, [dialog, form]);

  const promptCreate = (nodeKind: 'group' | 'page') => {
    const initialSlug = randomSlug();
    setDialog({
      kind: 'create',
      nodeKind,
      parentId: null,
      rank: appendRank(topbarNodes),
      title: nodeKind === 'group' ? '新增分组' : '新增页面',
      initialTitle: '',
      initialSlug,
      initialIcon: '',
      initialTooltip: '',
      showSlug: true
    });
  };

  const submitCreate = async () => {
    if (dialog?.kind !== 'create') return;
    const values = await form.validateFields();
    setPending(true);
    try {
      const input = {
        title: values.title?.trim() ?? '',
        slug: values.slug?.trim() ?? '',
        icon: values.icon ?? null,
        tooltip: values.tooltip ?? null,
        parentId: null,
        rank: dialog.rank,
        placement: 'topbar' as const
      };
      if (dialog.nodeKind === 'group') {
        await mutations.createGroup(input);
      } else {
        await mutations.createPage(input);
      }
      setDialog(null);
    } finally {
      setPending(false);
    }
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
      <PageTreeFormModal
        dialog={dialog}
        form={form}
        iconPickerOpen={iconPickerOpen}
        isOperationPending={pending || mutations.isPending}
        onCancel={() => setDialog(null)}
        onIconPickerOpenChange={setIconPickerOpen}
        onRefreshSlug={() => form.setFieldValue('slug', randomSlug())}
        onSubmit={() => {
          void submitCreate();
        }}
      />
    </Space>
  );
}
