import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import DragOutlined from '@ant-design/icons/es/icons/DragOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import FileAddOutlined from '@ant-design/icons/es/icons/FileAddOutlined';
import FolderAddOutlined from '@ant-design/icons/es/icons/FolderAddOutlined';
import MenuOutlined from '@ant-design/icons/es/icons/MenuOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { App, Button, Dropdown, Form, Space } from 'antd';
import { useEffect, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { i18nText } from '../shared/i18n/text';

import type { FrontstagePageTreeNode } from '../features/frontstage/api/page-tree';
import { FrontstageNodeActionButton } from '../features/frontstage/components/FrontstageNodeActionButton';
import { useFrontstagePageTreeMutations } from '../features/frontstage/hooks/use-frontstage-page-tree-mutations';
import {
  PageTreeFormModal,
  type PageTreeFormDialog,
  type PageTreeFormValues
} from '../features/frontstage/pages/frontstage-page/page-tree-form-modal';
import { projectNavigationPosition } from '../features/frontstage/lib/navigation-drag/projection';
import { useTopbarDragStore } from '../features/frontstage/lib/navigation-drag/topbar-drag-store';
import './navigation-drag.css';
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
  const { modal, message } = App.useApp();
  const [form] = Form.useForm<PageTreeFormValues>();
  const [dialog, setDialog] = useState<PageTreeFormDialog | null>(null);
  const [iconPickerOpen, setIconPickerOpen] = useState(false);
  const mutations = useFrontstagePageTreeMutations(workspaceId);
  const [projectionRect, setProjectionRect] = useState<DOMRect | null>(null);
  const drag = useTopbarDragStore((state) => state.drag);
  const activeDrag = drag?.workspaceId === workspaceId ? drag : null;
  const projection =
    !mutations.isPending && activeDrag?.target?.nodeId === node.id
      ? activeDrag.target
      : null;
  const source = siblings.find(
    (candidate) => candidate.id === activeDrag?.nodeId
  );
  const clearDrag = () => useTopbarDragStore.setState({ drag: null });
  useEffect(
    () => () => {
      const current = useTopbarDragStore.getState().drag;
      if (current?.workspaceId === workspaceId && current.nodeId === node.id)
        clearDrag();
    },
    [workspaceId, node.id]
  );
  const index = siblings.findIndex((candidate) => candidate.id === node.id);

  const openEdit = () => {
    setDialog({
      kind: 'rename',
      nodeId: node.id,
      title: '编辑顶部栏目',
      initialTitle: node.title ?? '',
      initialIcon: node.icon ?? '',
      initialTooltip: node.tooltip ?? '',
      initialSlug: node.slug ?? '',
      nodeKind: node.kind,
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

  const dragDataType = `application/x-frontstage-topbar-${workspaceId}`;
  const move = (nodeId: string, direction: -1 | 1) => {
    const rank =
      direction < 0
        ? index === 0
          ? '000000'
          : String(index * 1000 + 500).padStart(6, '0')
        : String((index + 1) * 1000 + 500).padStart(6, '0');
    void mutations.moveNode(nodeId, { parentId: null, rank }).catch(() => {
      void message.error('栏目排序失败，请重试');
    });
  };

  return (
    <span
      className={`app-shell-dynamic-nav-item${activeDrag?.nodeId === node.id ? ' app-shell-dynamic-nav-item--dragging' : ''}`}
      onDragLeave={(event) => {
        if (
          !event.currentTarget.contains(event.relatedTarget as Node | null) &&
          projection &&
          activeDrag
        ) {
          useTopbarDragStore.setState({
            drag: { ...activeDrag, target: null }
          });
        }
      }}
      onDragOver={(event) => {
        if (
          mutations.isPending ||
          !activeDrag ||
          !source ||
          source.id === node.id ||
          !event.dataTransfer.types.includes(dragDataType)
        )
          return;
        event.preventDefault();
        event.stopPropagation();
        event.dataTransfer.dropEffect = 'move';
        const rect = event.currentTarget.getBoundingClientRect();
        setProjectionRect(rect);
        const position = projectNavigationPosition(
          event.clientX,
          rect.left,
          rect.width,
          false
        );
        if (position && position !== 'inside') {
          useTopbarDragStore.setState({
            drag: { ...activeDrag, target: { nodeId: node.id, position } }
          });
        }
      }}
      onDrop={(event) => {
        const nodeId = event.dataTransfer.getData(dragDataType);
        event.preventDefault();
        event.stopPropagation();
        clearDrag();
        if (
          mutations.isPending ||
          !projection ||
          nodeId !== source?.id ||
          nodeId === node.id
        )
          return;
        move(nodeId, projection.position === 'before' ? -1 : 1);
      }}
    >
      {projection && source && projectionRect ? (
        <>
          <span
            className={`app-shell-nav-projection app-shell-nav-projection--${projection.position}`}
            aria-hidden
          />
          {createPortal(
            <span
              className="app-shell-nav-projection__caption"
              role="status"
              style={{
                left:
                  projection.position === 'before'
                    ? projectionRect.left
                    : projectionRect.right,
                top: projectionRect.bottom,
                transform:
                  projection.position === 'after'
                    ? 'translateX(-100%)'
                    : undefined
              }}
            >
              {source.title} → {node.title} ·{' '}
              {projection.position === 'before'
                ? i18nText('appShell', 'drag_projection.before')
                : i18nText('appShell', 'drag_projection.after')}
            </span>,
            document.body
          )}
        </>
      ) : null}
      {children}
      <span className="app-shell-dynamic-nav-item__actions">
        <FrontstageNodeActionButton
          aria-label={`拖拽排序${node.title ?? '顶部栏目'}`}
          icon={<DragOutlined />}
          disabled={mutations.isPending}
          draggable={!mutations.isPending}
          onDragStart={(event) => {
            if (mutations.isPending) {
              event.preventDefault();
              return;
            }
            event.stopPropagation();
            event.dataTransfer.effectAllowed = 'move';
            event.dataTransfer.setData(dragDataType, node.id);
            const label = event.currentTarget.closest(
              '.app-shell-dynamic-nav-item'
            ) as HTMLElement | null;
            if (label) {
              const rect = label.getBoundingClientRect();
              event.dataTransfer.setDragImage(
                label,
                event.clientX - rect.left,
                event.clientY - rect.top
              );
            }
            useTopbarDragStore.setState({
              drag: { workspaceId, nodeId: node.id, target: null }
            });
          }}
          onDragEnd={clearDrag}
          onClick={(event) => {
            event.preventDefault();
            event.stopPropagation();
          }}
        />
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
          <FrontstageNodeActionButton
            aria-label={`配置${node.title ?? '顶部栏目'}`}
            icon={<MenuOutlined />}
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
