import { DeleteOutlined, EditOutlined, PlusOutlined } from '@ant-design/icons';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  App,
  Button,
  ConfigProvider,
  Empty,
  Form,
  Input,
  List,
  Modal,
  Select,
  Space,
  Spin,
  Tooltip,
  Tree,
  Typography
} from 'antd';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../../shared/ui/PermissionDeniedState';
import { useWindowWorkspaceOverlayZIndex } from '../../../../shared/ui/window-workspace/WindowWorkspaceWindow';
import {
  fetchFrontstageBlockAncestors,
  fetchFrontstageBlockChildren,
  fetchFrontstageBlockDeleteImpact,
  fetchFrontstageBlockNode,
  fetchFrontstageBlockRoots,
  frontstageBlockTreeQueryKeys,
  searchFrontstageBlocks,
  type FrontstageBlockNodeSummary,
  type FrontstageBlockPresentation,
  type FrontstageBlockSearchResult
} from '../../api/block-tree';
import { useFrontstageBlockTreeMutations } from '../../hooks/use-frontstage-block-tree-mutations';
import { isForbiddenResponseError } from '../../lib/api-errors';
import type { FrontstageBlockDeletedEvent } from '../jsx-studio/block-tabs/types';
import {
  toBlockTreeMoveInput,
  type BlockSchemaTreeDropInfo,
  type BlockSchemaTreeNode
} from './tree-drop';

import './block-schema-tree.css';

const INITIAL_BLOCK_SOURCE =
  'export default function Block() { return null; }\n';

const BLOCK_SCHEMA_TREE_THEME = {
  components: {
    Tree: { indentSize: 12 }
  }
};

interface BlockFormValues {
  title: string;
  description?: string;
  presentation: FrontstageBlockPresentation;
}

type FormTarget =
  | { mode: 'create'; parent: FrontstageBlockNodeSummary }
  | { mode: 'edit'; node: FrontstageBlockNodeSummary };

export interface BlockSchemaTreePanelProps {
  workspaceId: string;
  pageId: string;
  currentBlockId: string;
  onOpenBlock: (blockId: string) => void;
  onDeletedBlock?: (event: FrontstageBlockDeletedEvent) => void;
}

export function BlockSchemaTreePanel({
  workspaceId,
  pageId,
  currentBlockId,
  onOpenBlock,
  onDeletedBlock
}: BlockSchemaTreePanelProps) {
  const { message, modal } = App.useApp();
  const formOverlayZIndex = useWindowWorkspaceOverlayZIndex();
  const [form] = Form.useForm<BlockFormValues>();
  const queryClient = useQueryClient();
  const mutations = useFrontstageBlockTreeMutations(workspaceId, pageId);
  const [expandedKeys, setExpandedKeys] = useState<string[]>([]);
  const [childrenByParent, setChildrenByParent] = useState<
    Map<string, FrontstageBlockNodeSummary[]>
  >(new Map());
  const [searchInput, setSearchInput] = useState('');
  const [searchQuery, setSearchQuery] = useState('');
  const [formTarget, setFormTarget] = useState<FormTarget | null>(null);
  const [childrenError, setChildrenError] = useState<unknown>(null);

  const rootsQuery = useQuery({
    queryKey: frontstageBlockTreeQueryKeys.roots(workspaceId, pageId),
    queryFn: () => fetchFrontstageBlockRoots(workspaceId, pageId)
  });
  const currentBlockQuery = useQuery({
    queryKey: frontstageBlockTreeQueryKeys.block(
      workspaceId,
      pageId,
      currentBlockId
    ),
    queryFn: () =>
      fetchFrontstageBlockNode(workspaceId, pageId, currentBlockId)
  });
  const ancestorsQuery = useQuery({
    queryKey: frontstageBlockTreeQueryKeys.ancestors(
      workspaceId,
      pageId,
      currentBlockId
    ),
    queryFn: () =>
      fetchFrontstageBlockAncestors(workspaceId, pageId, currentBlockId)
  });
  const searchResultsQuery = useQuery({
    queryKey: frontstageBlockTreeQueryKeys.search(workspaceId, pageId, {
      query: searchQuery
    }),
    queryFn: () =>
      searchFrontstageBlocks(workspaceId, pageId, { query: searchQuery }),
    enabled: searchQuery.length > 0
  });

  useEffect(() => {
    const timeout = window.setTimeout(() => {
      setSearchQuery(searchInput.trim());
    }, 250);
    return () => window.clearTimeout(timeout);
  }, [searchInput]);

  const loadChildren = useCallback(
    async (parent: FrontstageBlockNodeSummary) => {
      setChildrenError(null);
      try {
        const children = await queryClient.fetchQuery({
          queryKey: frontstageBlockTreeQueryKeys.children(
            workspaceId,
            pageId,
            parent.block_id
          ),
          queryFn: () =>
            fetchFrontstageBlockChildren(
              workspaceId,
              pageId,
              parent.block_id
            )
        });
        setChildrenByParent((current) => {
          const next = new Map(current);
          next.set(parent.block_id, children);
          return next;
        });
      } catch (error) {
        setChildrenError(error);
        throw error;
      }
    },
    [pageId, queryClient, workspaceId]
  );

  useEffect(() => {
    if (!ancestorsQuery.data || !currentBlockQuery.data) return;
    const path = ancestorsQuery.data;
    setExpandedKeys((current) => {
      const next = new Set(current);
      path.forEach((node) => next.add(node.block_id));
      return next.size === current.length ? current : [...next];
    });
    void Promise.all(path.map((node) => loadChildren(node))).catch(
      () => undefined
    );
  }, [ancestorsQuery.data, currentBlockQuery.data, loadChildren]);

  const treeData = useMemo(
    () =>
      (rootsQuery.data ?? []).map((root) =>
        toTreeNode(root, childrenByParent)
      ),
    [childrenByParent, rootsQuery.data]
  );

  const operationPending =
    mutations.create.isPending ||
    mutations.update.isPending ||
    mutations.move.isPending ||
    mutations.deleteLeaf.isPending ||
    mutations.deleteSubtree.isPending;

  const showOperationError = (error: unknown) => {
    void message.error(
      error instanceof Error
        ? error.message
        : i18nText('frontstage', 'auto.block_tree_operation_failed')
    );
  };

  const refreshOwner = async (parentBlockId: string | null) => {
    if (parentBlockId === null) {
      await rootsQuery.refetch();
      return;
    }
    const parent = findSummary(treeData, parentBlockId);
    if (parent) await loadChildren(parent);
  };

  const openCreateForm = (parent: FrontstageBlockNodeSummary) => {
    setFormTarget({ mode: 'create', parent });
    form.setFieldsValue({ title: '', description: '', presentation: 'page' });
  };

  const openEditForm = (node: FrontstageBlockNodeSummary) => {
    setFormTarget({ mode: 'edit', node });
    form.setFieldsValue({
      title: node.title ?? node.block_id,
      description: node.description ?? '',
      presentation: node.presentation
    });
  };

  const submitForm = async () => {
    if (!formTarget) return;
    const values = await form.validateFields();
    try {
      if (formTarget.mode === 'create') {
        const created = await mutations.create.mutateAsync({
          tab_id: formTarget.parent.tab_id,
          title: values.title,
          description: values.description ?? '',
          presentation: values.presentation,
          parent_block_id: formTarget.parent.block_id,
          before_block_id: null,
          after_block_id: null,
          code: INITIAL_BLOCK_SOURCE,
          runtime_descriptor: null
        });
        await loadChildren(formTarget.parent);
        setExpandedKeys((current) =>
          current.includes(formTarget.parent.block_id)
            ? current
            : [...current, formTarget.parent.block_id]
        );
        void message.success(
          i18nText('frontstage', 'auto.block_tree_created')
        );
        onOpenBlock(created.block_id);
      } else {
        await mutations.update.mutateAsync({
          block_id: formTarget.node.block_id,
          input: {
            title: values.title,
            description: values.description ?? '',
            presentation: values.presentation
          }
        });
        await refreshOwner(formTarget.node.parent_block_id);
        void message.success(
          i18nText('frontstage', 'auto.block_tree_updated')
        );
      }
      setFormTarget(null);
      form.resetFields();
    } catch (error) {
      showOperationError(error);
    }
  };

  const deleteNode = async (node: FrontstageBlockNodeSummary) => {
    try {
      const impact = await fetchFrontstageBlockDeleteImpact(
        workspaceId,
        pageId,
        node.block_id
      );
      modal.confirm({
        title:
          impact.affected_count === 1
            ? i18nText('frontstage', 'auto.block_tree_delete_leaf_title')
            : i18nText('frontstage', 'auto.block_tree_delete_subtree_title'),
        content:
          impact.affected_count === 1
            ? i18nText('frontstage', 'auto.block_tree_delete_leaf_description')
            : i18nText(
                'frontstage',
                'auto.block_tree_delete_subtree_description',
                { value1: impact.affected_count }
              ),
        okButtonProps: { danger: true },
        okText: i18nText('frontstage', 'auto.delete'),
        cancelText: i18nText('frontstage', 'auto.cancel'),
        onOk: async () => {
          try {
            if (impact.affected_count === 1) {
              await mutations.deleteLeaf.mutateAsync({
                block_id: node.block_id,
                parent_block_id: node.parent_block_id
              });
            } else {
              await mutations.deleteSubtree.mutateAsync({
                block_id: node.block_id,
                parent_block_id: node.parent_block_id,
                input: {
                  expected_affected_count: impact.affected_count
                }
              });
            }
            await refreshOwner(node.parent_block_id);
            onDeletedBlock?.({
              block_id: node.block_id,
              subtree: impact.affected_count > 1
            });
            void message.success(
              i18nText('frontstage', 'auto.block_tree_deleted')
            );
          } catch (error) {
            showOperationError(error);
            throw error;
          }
        }
      });
    } catch (error) {
      showOperationError(error);
    }
  };

  const moveNode = async (info: BlockSchemaTreeDropInfo) => {
    const input = toBlockTreeMoveInput(info);
    const previousParentBlockId = info.dragNode.summary.parent_block_id;
    try {
      await mutations.move.mutateAsync({
        block_id: info.dragNode.summary.block_id,
        previous_parent_block_id: previousParentBlockId,
        input
      });
      await Promise.all([
        refreshOwner(previousParentBlockId),
        input.parent_block_id === previousParentBlockId
          ? Promise.resolve()
          : refreshOwner(input.parent_block_id)
      ]);
    } catch (error) {
      showOperationError(error);
    }
  };

  const initialError =
    rootsQuery.error ?? currentBlockQuery.error ?? ancestorsQuery.error;
  const permissionDenied =
    isForbiddenResponseError(initialError) ||
    isForbiddenResponseError(childrenError);

  return (
    <div className="frontstage-block-schema-tree">
      <div className="frontstage-block-schema-tree__search">
        <Input.Search
          allowClear
          aria-label={i18nText('frontstage', 'auto.block_tree_search')}
          placeholder={i18nText('frontstage', 'auto.block_tree_search')}
          value={searchInput}
          onChange={(event) => setSearchInput(event.target.value)}
        />
        {searchQuery ? (
          <SearchResults
            error={searchResultsQuery.error}
            loading={searchResultsQuery.isFetching}
            results={searchResultsQuery.data ?? []}
            onOpenBlock={onOpenBlock}
          />
        ) : null}
      </div>

      <div className="frontstage-block-schema-tree__body">
        {permissionDenied ? <PermissionDeniedState /> : null}
        {!permissionDenied && initialError ? (
          <Alert
            showIcon
            type="error"
            title={i18nText('frontstage', 'auto.block_tree_load_failed')}
            action={
              <Button
                size="small"
                onClick={() => {
                  void Promise.all([
                    rootsQuery.refetch(),
                    currentBlockQuery.refetch(),
                    ancestorsQuery.refetch()
                  ]);
                }}
              >
                {i18nText('frontstage', 'auto.retry')}
              </Button>
            }
          />
        ) : null}
        {!permissionDenied && !initialError && rootsQuery.isPending ? (
          <div className="frontstage-block-schema-tree__state">
            <Spin />
            <Typography.Text type="secondary">
              {i18nText('frontstage', 'auto.block_tree_loading')}
            </Typography.Text>
          </div>
        ) : null}
        {!permissionDenied && !initialError && !rootsQuery.isPending ? (
          <>
            {childrenError ? (
              <Alert
                showIcon
                type="error"
                title={i18nText(
                  'frontstage',
                  'auto.block_tree_children_load_failed'
                )}
              />
            ) : null}
            {treeData.length > 0 ? (
              <ConfigProvider theme={BLOCK_SCHEMA_TREE_THEME}>
                <Tree<BlockSchemaTreeNode>
                  blockNode
                  draggable={operationPending ? false : { icon: false }}
                  expandedKeys={expandedKeys}
                  selectedKeys={[currentBlockId]}
                  treeData={treeData}
                  loadData={(node) => loadChildren(node.summary)}
                  onDrop={(info) => void moveNode(info)}
                  onExpand={(keys) => setExpandedKeys(keys.map(String))}
                  titleRender={(node) => (
                    <BlockTreeNodeTitle
                      node={node.summary}
                      pending={operationPending}
                      onCreate={openCreateForm}
                      onDelete={(target) => void deleteNode(target)}
                      onEdit={openEditForm}
                      onOpen={onOpenBlock}
                    />
                  )}
                />
              </ConfigProvider>
            ) : (
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={i18nText('frontstage', 'auto.block_tree_empty')}
            />
            )}
          </>
        ) : null}
      </div>

      <Modal
        destroyOnHidden
        open={formTarget !== null}
        zIndex={formOverlayZIndex}
        title={
          formTarget?.mode === 'create'
            ? i18nText('frontstage', 'auto.block_tree_create_child')
            : i18nText('frontstage', 'auto.edit_block')
        }
        confirmLoading={operationPending}
        okText={
          formTarget?.mode === 'create'
            ? i18nText('frontstage', 'auto.create')
            : i18nText('frontstage', 'auto.update')
        }
        cancelText={i18nText('frontstage', 'auto.cancel')}
        onCancel={() => {
          setFormTarget(null);
          form.resetFields();
        }}
        onOk={() => void submitForm()}
      >
        <Form form={form} layout="vertical">
          <Form.Item
            name="title"
            label={i18nText('frontstage', 'auto.title')}
            rules={[
              {
                required: true,
                whitespace: true,
                message: i18nText(
                  'frontstage',
                  'auto.block_tree_title_required'
                )
              }
            ]}
          >
            <Input autoFocus />
          </Form.Item>
          <Form.Item
            name="presentation"
            label={i18nText('frontstage', 'auto.block_tree_presentation')}
            rules={[{ required: true }]}
          >
            <Select
              options={presentationOptions()}
              styles={
                formOverlayZIndex === undefined
                  ? undefined
                  : {
                      popup: {
                        root: { zIndex: formOverlayZIndex + 1 }
                      }
                    }
              }
            />
          </Form.Item>
          <Form.Item
            name="description"
            label={i18nText('frontstage', 'auto.block_tree_description')}
          >
            <Input.TextArea autoSize={{ minRows: 3, maxRows: 8 }} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}

function toTreeNode(
  node: FrontstageBlockNodeSummary,
  childrenByParent: Map<string, FrontstageBlockNodeSummary[]>
): BlockSchemaTreeNode {
  const children = childrenByParent.get(node.block_id);
  return {
    key: node.block_id,
    title: node.title ?? node.block_id,
    summary: node,
    isLeaf: children !== undefined && children.length === 0,
    children: children?.map((child) => toTreeNode(child, childrenByParent))
  };
}

function findSummary(
  nodes: BlockSchemaTreeNode[],
  blockId: string
): FrontstageBlockNodeSummary | null {
  for (const node of nodes) {
    if (node.summary.block_id === blockId) return node.summary;
    const child = findSummary(node.children ?? [], blockId);
    if (child) return child;
  }
  return null;
}

function presentationOptions() {
  return [
    {
      value: 'page' as const,
      label: i18nText('frontstage', 'auto.page')
    },
    {
      value: 'drawer' as const,
      label: i18nText('frontstage', 'auto.block_tree_drawer')
    },
    {
      value: 'modal' as const,
      label: i18nText('frontstage', 'auto.block_tree_modal')
    },
    {
      value: 'inline' as const,
      label: i18nText('frontstage', 'auto.block_tree_inline')
    }
  ];
}

function BlockTreeNodeTitle({
  node,
  pending,
  onCreate,
  onEdit,
  onDelete,
  onOpen
}: {
  node: FrontstageBlockNodeSummary;
  pending: boolean;
  onCreate: (node: FrontstageBlockNodeSummary) => void;
  onEdit: (node: FrontstageBlockNodeSummary) => void;
  onDelete: (node: FrontstageBlockNodeSummary) => void;
  onOpen: (blockId: string) => void;
}) {
  return (
    <span
      className="frontstage-block-schema-tree__node"
      onClick={() => onOpen(node.block_id)}
    >
      <span className="frontstage-block-schema-tree__node-title">
        {node.title ?? node.block_id}
      </span>
      <span
        className="frontstage-block-schema-tree__node-actions"
        onClick={(event) => event.stopPropagation()}
      >
        <Tooltip
          title={i18nText('frontstage', 'auto.block_tree_create_child')}
        >
          <Button
            aria-label={i18nText(
              'frontstage',
              'auto.block_tree_create_child'
            )}
            disabled={pending}
            icon={<PlusOutlined />}
            size="small"
            type="text"
            onClick={() => onCreate(node)}
          />
        </Tooltip>
        <Tooltip title={i18nText('frontstage', 'auto.edit')}>
          <Button
            aria-label={i18nText('frontstage', 'auto.edit')}
            disabled={pending}
            icon={<EditOutlined />}
            size="small"
            type="text"
            onClick={() => onEdit(node)}
          />
        </Tooltip>
        <Tooltip title={i18nText('frontstage', 'auto.delete')}>
          <Button
            danger
            aria-label={i18nText('frontstage', 'auto.delete')}
            disabled={pending}
            icon={<DeleteOutlined />}
            size="small"
            type="text"
            onClick={() => onDelete(node)}
          />
        </Tooltip>
      </span>
    </span>
  );
}

function SearchResults({
  error,
  loading,
  results,
  onOpenBlock
}: {
  error: unknown;
  loading: boolean;
  results: FrontstageBlockSearchResult[];
  onOpenBlock: (blockId: string) => void;
}) {
  if (isForbiddenResponseError(error)) return <PermissionDeniedState />;
  if (error) {
    return (
      <Alert
        showIcon
        type="error"
        title={i18nText('frontstage', 'auto.block_tree_search_failed')}
      />
    );
  }
  if (loading) return <Spin size="small" />;
  if (results.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description={i18nText('frontstage', 'auto.block_tree_search_empty')}
      />
    );
  }

  return (
    <List
      className="frontstage-block-schema-tree__search-results"
      dataSource={results}
      renderItem={(result) => (
        <List.Item>
          <Button
            block
            type="text"
            className="frontstage-block-schema-tree__search-result"
            onClick={() => onOpenBlock(result.node.block_id)}
          >
            <Space direction="vertical" size={0} align="start">
              <Typography.Text>
                {result.node.title ?? result.node.block_id}
              </Typography.Text>
              <Typography.Text type="secondary" ellipsis>
                {[
                  ...result.ancestors.map(
                    (ancestor) => ancestor.title ?? ancestor.block_id
                  ),
                  result.node.title ?? result.node.block_id
                ].join(' / ')}
              </Typography.Text>
            </Space>
          </Button>
        </List.Item>
      )}
    />
  );
}
