import {
  FileAddOutlined,
  FolderAddOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { Button, Dropdown, Form, Space } from 'antd';
import { useEffect, useState } from 'react';

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
