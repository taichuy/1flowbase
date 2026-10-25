import { useMemo, useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  Alert,
  App,
  ConfigProvider,
  AutoComplete,
  Button,
  Dropdown,
  Empty,
  Input,
  Space,
  Spin,
  Tooltip
} from 'antd';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import EditOutlined from '@ant-design/icons/es/icons/EditOutlined';
import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import EllipsisOutlined from '@ant-design/icons/es/icons/EllipsisOutlined';
import SearchOutlined from '@ant-design/icons/es/icons/SearchOutlined';
import DragOutlined from '@ant-design/icons/es/icons/DragOutlined';
import EyeOutlined from '@ant-design/icons/es/icons/EyeOutlined';
import EyeInvisibleOutlined from '@ant-design/icons/es/icons/EyeInvisibleOutlined';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../state/navigation-query-scope';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  type MoveFrontstageNodeInput,
  type FrontstagePageTreeNode
} from '../../../frontstage/api/page-tree';
import { useFrontstagePageTreeMutations } from '../../../frontstage/hooks/use-frontstage-page-tree-mutations';
import { NavigationMoveTree } from '../../../frontstage/components/page-tree-actions/NavigationMoveTree';
import { MovePageModal } from '../../../frontstage/components/page-tree-actions/MovePageModal';
import { useRouteManagementMutation } from '../../api/ui-management/route-management';
import {
  RouteNodeDrawer,
  type RouteNodeEditor
} from './route-management/RouteNodeDrawer';
import './route-management/route-management.css';

export function RouteManagementTab({ canManage }: { canManage: boolean }) {
  const scope = useAuthStore(selectNavigationQueryScope);
  return <RouteManagementContent key={scope} canManage={canManage} />;
}

function RouteManagementContent({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const { modal, message } = App.useApp();
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const scope = useAuthStore(selectNavigationQueryScope);
  const canDesign = useAuthStore(
    (state) =>
      state.actor?.effective_display_role === 'root' ||
      Boolean(state.me?.permissions.includes('frontstage.page.design'))
  );
  const editable = canManage && canDesign;
  const tree = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId ?? '', scope),
    queryFn: () => fetchFrontstagePageTree(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    retry: false
  });
  const mutations = useFrontstagePageTreeMutations(workspaceId ?? '');
  const action = useRouteManagementMutation(workspaceId ?? '');
  const moving = useRef(false);
  const [moveError, setMoveError] = useState(false);
  const [editor, setEditor] = useState<RouteNodeEditor | null>(null);
  const [movingId, setMovingId] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [locatedId, setLocatedId] = useState<string>();
  const nodes = useMemo(() => {
    const result: Array<{ node: FrontstagePageTreeNode; path: string }> = [];
    const visit = (items: FrontstagePageTreeNode[], parent = '') => {
      for (const node of items) {
        const path = [parent, node.title || t('routes_unnamed_node')]
          .filter(Boolean)
          .join(' / ');
        result.push({ node, path });
        visit(node.children, path);
      }
    };
    visit(tree.data ?? []);
    return result;
  }, [tree.data, t]);
  const nodesById = useMemo(
    () => new Map(nodes.map(({ node }) => [node.id, node])),
    [nodes]
  );
  const busy = mutations.isPending || action.isPending;
  const move = async (id: string, input: MoveFrontstageNodeInput) => {
    if (!editable || !workspaceId || moving.current || action.isPending)
      return false;
    moving.current = true;
    setMoveError(false);
    try {
      await mutations.moveNode(id, input);
      return true;
    } catch {
      setMoveError(true);
      return false;
    } finally {
      moving.current = false;
    }
  };
  const createMenu = (parent_id: string | null) => ({
    items: [
      {
        key: 'page',
        label: t('routes_add_page'),
        onClick: () =>
          setEditor({ kind: 'create', nodeKind: 'page', parent_id })
      },
      {
        key: 'group',
        label: t('routes_add_group'),
        onClick: () =>
          setEditor({ kind: 'create', nodeKind: 'group', parent_id })
      }
    ]
  });
  const remove = (node: FrontstagePageTreeNode) =>
    modal.confirm({
      title: t('routes_delete_confirm', { name: node.title }),
      content:
        node.kind === 'group'
          ? t('routes_delete_group_description')
          : t('routes_delete_page_description'),
      okText: t('delete'),
      cancelText: t('cancel'),
      okButtonProps: { danger: true },
      onOk: async () => {
        if (!editable || busy) return;
        try {
          await action.mutateAsync({ kind: 'delete', id: node.id });
        } catch (error) {
          void message.error(t('routes_action_failed'));
          throw error;
        }
      }
    });
  const movingNode = nodes.find(({ node }) => node.id === movingId)?.node;
  return (
    <ConfigProvider
      theme={{
        components: {
          Input: { borderRadiusLG: 8, controlHeightLG: 44 },
          Button: {
            colorPrimary: 'var(--color-primary)',
            colorPrimaryHover: 'var(--color-primary-hover)',
            colorPrimaryActive: 'var(--color-primary)',
            primaryColor: '#ffffff',
            borderRadiusLG: 8,
            controlHeightLG: 44
          }
        }
      }}
    >
      <div className="route-management">
        <div className="route-management__notice" role="note">
          {editable ? t('routes_hint') : t('routes_readonly')}
        </div>
        <section className="route-management__surface">
          <div className="route-management__toolbar">
            <AutoComplete
              className="route-management__search"
              value={search}
              onChange={(value) => {
                setSearch(value);
                if (!value) setLocatedId(undefined);
              }}
              options={
                search.trim()
                  ? nodes
                      .filter(({ path, node }) =>
                        `${path} ${node.slug ?? ''}`
                          .toLocaleLowerCase()
                          .includes(search.trim().toLocaleLowerCase())
                      )
                      .map(({ node, path }) => ({
                        value: node.id,
                        label: path
                      }))
                  : []
              }
              onSelect={(id) => {
                setLocatedId(id);
                setSearch(
                  nodes.find(({ node }) => node.id === id)?.node.title ?? ''
                );
              }}
              notFoundContent={search.trim() ? t('routes_no_match') : null}
            >
              <Input
                size="large"
                allowClear
                prefix={<SearchOutlined />}
                placeholder={t('routes_search')}
                aria-label={t('routes_search')}
              />
            </AutoComplete>
            <Space>
              {busy ? (
                <span role="status" className="route-management__status">
                  {t('routes_saving')}
                </span>
              ) : null}
              {editable ? (
                <Dropdown
                  menu={createMenu(null)}
                  trigger={['click']}
                  disabled={busy}
                >
                  <Button size="large" type="primary" icon={<PlusOutlined />}>
                    {t('routes_add_menu')}
                  </Button>
                </Dropdown>
              ) : null}
            </Space>
          </div>
          {moveError ? (
            <Alert type="error" showIcon title={t('routes_move_failed')} />
          ) : null}
          {action.isError ? (
            <Alert type="error" showIcon title={t('routes_action_failed')} />
          ) : null}
          {!workspaceId ? (
            <Alert type="warning" title={t('routes_workspace_missing')} />
          ) : tree.isError ? (
            <Alert
              type="error"
              showIcon
              title={t('routes_load_failed')}
              action={
                <Button onClick={() => void tree.refetch()}>
                  {t('routes_retry')}
                </Button>
              }
            />
          ) : tree.isPending ? (
            <div className="route-management__loading">
              <Spin />
            </div>
          ) : !tree.data.length ? (
            <Empty description={t('routes_empty')} />
          ) : (
            <NavigationMoveTree
              key={scope}
              management
              pageTree={tree.data}
              disabled={!editable || busy}
              height={560}
              locateNodeId={locatedId}
              onSelectMove={(id, input) => void move(id, input)}
              renderActions={(item) => {
                const node = item ? nodesById.get(item.id) : null;
                const visibilityLabel = node?.is_hidden
                  ? t('routes_status_hidden')
                  : t('routes_status_visible');
                return (
                  <Space size={2}>
                    {node ? (
                      <Tooltip title={visibilityLabel}>
                        <span
                          className="route-management__visibility"
                          role="img"
                          aria-label={visibilityLabel}
                          tabIndex={0}
                        >
                          {node.is_hidden ? (
                            <EyeInvisibleOutlined aria-hidden="true" />
                          ) : (
                            <EyeOutlined aria-hidden="true" />
                          )}
                        </span>
                      </Tooltip>
                    ) : null}
                    {editable && (!node || node.kind === 'group') ? (
                      <Dropdown
                        menu={createMenu(node?.id ?? null)}
                        trigger={['click']}
                        disabled={busy}
                      >
                        <Tooltip title={t('routes_add_child')}>
                          <Button
                            type="text"
                            icon={<PlusOutlined />}
                            aria-label={`${t('routes_add_child')} ${node?.title ?? t('routes_root')}`}
                          />
                        </Tooltip>
                      </Dropdown>
                    ) : null}
                    {node ? (
                      <>
                        <span
                          className={
                            editable
                              ? 'route-management__inline-action'
                              : undefined
                          }
                        >
                          <Tooltip title={editable ? t('edit') : t('view')}>
                            <Button
                              type="text"
                              icon={<EditOutlined />}
                              disabled={busy}
                              aria-label={`${editable ? t('edit') : t('view')} ${node.title ?? ''}`}
                              onClick={() => setEditor({ kind: 'edit', node })}
                            />
                          </Tooltip>
                        </span>
                        {editable ? (
                          <>
                            <span className="route-management__inline-action">
                              <Tooltip title={t('delete')}>
                                <Button
                                  className="route-management__delete"
                                  type="text"
                                  icon={<DeleteOutlined />}
                                  disabled={busy}
                                  aria-label={`${t('delete')} ${node.title ?? ''}`}
                                  onClick={() => remove(node)}
                                />
                              </Tooltip>
                            </span>
                            <Dropdown
                              disabled={busy}
                              menu={{
                                items: [
                                  {
                                    key: 'edit',
                                    label: t('edit'),
                                    icon: <EditOutlined />,
                                    onClick: () =>
                                      setEditor({ kind: 'edit', node })
                                  },
                                  {
                                    key: 'move',
                                    label: t('routes_move'),
                                    icon: <DragOutlined />,
                                    onClick: () => setMovingId(node.id)
                                  },
                                  {
                                    key: 'delete',
                                    label: t('delete'),
                                    danger: true,
                                    icon: <DeleteOutlined />,
                                    onClick: () => remove(node)
                                  }
                                ]
                              }}
                              trigger={['click']}
                            >
                              <Button
                                type="text"
                                icon={<EllipsisOutlined />}
                                aria-label={`${t('routes_more')} ${node.title ?? ''}`}
                              />
                            </Dropdown>
                          </>
                        ) : null}
                      </>
                    ) : null}
                  </Space>
                );
              }}
            />
          )}
        </section>
        {editor && workspaceId ? (
          <RouteNodeDrawer
            key={`${scope}:${editor.kind === 'edit' ? editor.node.id : `${editor.nodeKind}:${editor.parent_id}`}`}
            editor={editor}
            workspaceId={workspaceId}
            editable={editable}
            onClose={() => setEditor(null)}
          />
        ) : null}
        {movingNode ? (
          <MovePageModal
            node={movingNode}
            pageTree={tree.data ?? []}
            isOperationPending={busy}
            onMove={move}
            onCancel={() => setMovingId(null)}
          />
        ) : null}
      </div>
    </ConfigProvider>
  );
}
