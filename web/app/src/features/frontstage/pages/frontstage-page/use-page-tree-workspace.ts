import { App as AntdApp, Form } from 'antd';
import { useEffect, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  findNodeById,
  getDeleteConfirmMessage,
  moveNodeInTree,
  normalizePageTree,
  removeNodeFromTree,
  resolveSelectedPageId
} from '../../lib/page-tree';
import type { FrontStageTreeNode } from '../../lib/page-tree';
import type { FrontStagePageProps } from './page-props';
import type {
  PageTreeFormDialog,
  PageTreeFormValues
} from './page-tree-form-modal';
import {
  findSiblingContext,
  getNodeAppendRank,
  isNodeDescendantOf,
  moveNodeToTreePosition,
  rankForMoveTarget,
  updatePageTreeNode,
  type CreatePageTreeNodeInput,
  type PageTreeOperationStatus
} from './page-tree-operations';

type PageTreeWorkspaceInput = Pick<
  FrontStagePageProps,
  | 'pageId'
  | 'onNavigatePage'
  | 'initialPageTree'
  | 'isPageTreeMutating'
  | 'onCreateGroupNode'
  | 'onCreatePageNode'
  | 'onRenamePageNode'
  | 'onUpdatePageNodeMetadata'
  | 'onMovePageNode'
  | 'onDeletePageNode'
> & {
  autoSelectFirstPage: boolean;
};

export function usePageTreeWorkspace({
  pageId,
  autoSelectFirstPage,
  onNavigatePage,
  initialPageTree,
  isPageTreeMutating,
  onCreateGroupNode,
  onCreatePageNode,
  onRenamePageNode,
  onUpdatePageNodeMetadata,
  onMovePageNode,
  onDeletePageNode
}: PageTreeWorkspaceInput) {
  const { modal } = AntdApp.useApp();
  const [pageTreeForm] = Form.useForm<PageTreeFormValues>();
  const [operationStatus, setOperationStatus] =
    useState<PageTreeOperationStatus>('idle');
  const [pageTreeFormDialog, setPageTreeFormDialog] =
    useState<PageTreeFormDialog | null>(null);
  const [isPageTreeIconPickerOpen, setIsPageTreeIconPickerOpen] =
    useState(false);
  const [pageTree, setPageTree] = useState<FrontStageTreeNode[]>(() =>
    normalizePageTree(initialPageTree ?? [])
  );
  const [selectedPageId, setSelectedPageId] = useState<string | null>(() =>
    !pageId && !autoSelectFirstPage
      ? null
      : resolveSelectedPageId({
          pageId,
          pageTree: normalizePageTree(initialPageTree ?? [])
        }).selectedPageId
  );
  const selectedPageNode = selectedPageId
    ? findNodeById(pageTree, selectedPageId)
    : null;

  useEffect(() => {
    const resolution =
      !pageId && !autoSelectFirstPage
        ? {
            selectedPageId: null,
            navigationTarget: undefined,
            shouldNavigate: false
          }
        : resolveSelectedPageId({
            currentSelectedPageId: selectedPageId,
            pageId,
            pageTree
          });

    if (selectedPageId !== resolution.selectedPageId) {
      setSelectedPageId(resolution.selectedPageId);
    }

    if (resolution.shouldNavigate) {
      onNavigatePage?.(resolution.navigationTarget);
    }
  }, [autoSelectFirstPage, onNavigatePage, pageId, pageTree, selectedPageId]);

  useEffect(() => {
    if (!initialPageTree) {
      return;
    }

    setPageTree(normalizePageTree(initialPageTree));
    setOperationStatus('idle');
  }, [initialPageTree]);

  useEffect(() => {
    if (!pageTreeFormDialog) {
      setIsPageTreeIconPickerOpen(false);
      return;
    }

    if (pageTreeFormDialog.kind === 'tooltip') {
      pageTreeForm.setFieldsValue({
        tooltip: pageTreeFormDialog.initialTooltip
      });
      return;
    }

    pageTreeForm.setFieldsValue({
      title: pageTreeFormDialog.initialTitle,
      icon: pageTreeFormDialog.initialIcon,
      tooltip: pageTreeFormDialog.initialTooltip,
      slug: pageTreeFormDialog.initialSlug
    });
  }, [pageTreeForm, pageTreeFormDialog]);

  const runPageTreeOperation = async (
    operation: () => Promise<unknown>
  ): Promise<boolean> => {
    setOperationStatus('pending');

    try {
      await operation();
      setOperationStatus('idle');
      return true;
    } catch {
      setOperationStatus('error');
      return false;
    }
  };

  const openCreateNodeDialog = (
    nodeKind: 'group' | 'page',
    parentId: string | null,
    rank: string
  ) => {
    setPageTreeFormDialog({
      kind: 'create',
      nodeKind,
      parentId,
      rank,
      initialTitle: '',
      initialIcon: '',
      initialTooltip: '',
      title:
        nodeKind === 'page'
          ? i18nText('frontstage', 'auto.add_page')
          : i18nText('frontstage', 'auto.add_group')
    });
  };

  const createPageTreeNode = async (
    nodeKind: 'group' | 'page',
    input: CreatePageTreeNodeInput
  ) => {
    if (nodeKind === 'page') {
      const createdNode = await onCreatePageNode?.(input);
      if (createdNode?.kind === 'page') {
        setSelectedPageId(createdNode.id);
        onNavigatePage?.(createdNode.id);
      }
      return;
    }

    await onCreateGroupNode?.(input);
  };

  const handleAddGroup = () => {
    openCreateNodeDialog('group', null, getNodeAppendRank(pageTree, null));
  };

  const handleAddPage = () => {
    openCreateNodeDialog('page', null, getNodeAppendRank(pageTree, null));
  };

  const handleAddPageInGroup = (groupId: string) => {
    openCreateNodeDialog('page', groupId, getNodeAppendRank(pageTree, groupId));
  };

  const handleAddNodeAtPosition = (
    kind: 'page' | 'group',
    targetNodeId: string,
    position: 'before' | 'after'
  ) => {
    const siblingContext = findSiblingContext(pageTree, targetNodeId);
    if (!siblingContext) {
      return;
    }

    const { parentId, siblings, index } = siblingContext;
    let rank = '';
    if (position === 'before') {
      rank = rankForMoveTarget(index, -1);
    } else if (index === siblings.length - 1) {
      rank = getNodeAppendRank(pageTree, parentId);
    } else {
      rank = rankForMoveTarget(index, 1);
    }

    openCreateNodeDialog(kind, parentId, rank);
  };

  const handleDeleteNode = (nodeId: string) => {
    const node = findNodeById(pageTree, nodeId);
    if (!node) {
      return;
    }

    modal.confirm({
      title: i18nText('frontstage', 'auto.delete_node'),
      content: getDeleteConfirmMessage(node),
      okText: i18nText('frontstage', 'auto.delete'),
      okButtonProps: { danger: true },
      cancelText: i18nText('frontstage', 'auto.cancel'),
      onOk: async () => {
        await runPageTreeOperation(async () => {
          await onDeletePageNode?.(nodeId);
          const next = removeNodeFromTree(pageTree, nodeId);
          const nextResolution = resolveSelectedPageId({
            currentSelectedPageId: selectedPageId,
            pageTree: next
          });

          setSelectedPageId(nextResolution.selectedPageId);
          if (nextResolution.shouldNavigate) {
            onNavigatePage?.(nextResolution.navigationTarget);
          }
        });
      }
    });
  };

  const handleSubmitPageTreeForm = async () => {
    if (!pageTreeFormDialog) {
      return;
    }

    const values = await pageTreeForm.validateFields();
    const dialog = pageTreeFormDialog;

    if (dialog.kind === 'create') {
      const input = {
        title: values.title ?? '',
        icon: values.icon ?? null,
        tooltip: values.tooltip ?? null,
        parentId: dialog.parentId,
        rank: dialog.rank
      };
      const created = await runPageTreeOperation(async () => {
        await createPageTreeNode(dialog.nodeKind, input);
      });
      if (created) {
        setPageTreeFormDialog(null);
      }
      return;
    }

    if (dialog.kind === 'rename') {
      const title = values.title ?? '';
      const icon = values.icon ?? null;
      const tooltip = values.tooltip ?? null;
      const renamed = await runPageTreeOperation(async () => {
        await onRenamePageNode?.(dialog.nodeId, { title, icon, tooltip });
        setPageTree((currentTree) =>
          updatePageTreeNode(currentTree, dialog.nodeId, {
            title,
            icon,
            tooltip
          })
        );
      });
      if (renamed) {
        setPageTreeFormDialog(null);
      }
      return;
    }

    const updated = await runPageTreeOperation(async () => {
      await onUpdatePageNodeMetadata?.(dialog.nodeId, {
        tooltip: values.tooltip ?? ''
      });
    });
    if (updated) {
      setPageTreeFormDialog(null);
    }
  };

  const handleRenameNode = (node: FrontStageTreeNode) => {
    setPageTreeFormDialog({
      kind: 'rename',
      nodeId: node.id,
      initialTitle: node.title ?? '',
      initialIcon: node.icon ?? '',
      initialTooltip: node.tooltip ?? '',
      nodeKind: node.kind,
      title:
        node.kind === 'page'
          ? i18nText('frontstage', 'design.configure_page')
          : i18nText('frontstage', 'auto.edit_node')
    });
  };

  const handlePageTabsEnabledChange = (enabled: boolean) => {
    if (!selectedPageNode || selectedPageNode.kind !== 'page') {
      return;
    }

    const contentPresentation = enabled ? 'tabs' : 'single';
    void runPageTreeOperation(async () => {
      await onRenamePageNode?.(selectedPageNode.id, {
        title: selectedPageNode.title ?? '',
        icon: selectedPageNode.icon ?? '',
        tooltip: selectedPageNode.tooltip ?? '',
        contentPresentation
      });
      setPageTree((currentTree) =>
        updatePageTreeNode(currentTree, selectedPageNode.id, {
          content_presentation: contentPresentation
        })
      );
    });
  };

  const handleEditNodeTooltip = (
    nodeId: string,
    currentTooltip: string | null
  ) => {
    setPageTreeFormDialog({
      kind: 'tooltip',
      nodeId,
      initialTooltip: currentTooltip ?? '',
      title: i18nText('frontstage', 'auto.edit_description')
    });
  };

  const handleUpdateNodeMetadata = (
    nodeId: string,
    input: { tooltip?: string | null; isHidden?: boolean }
  ) => {
    void runPageTreeOperation(async () => {
      await onUpdatePageNodeMetadata?.(nodeId, input);
      setPageTree((currentTree) =>
        updatePageTreeNode(currentTree, nodeId, {
          ...(Object.prototype.hasOwnProperty.call(input, 'tooltip')
            ? { tooltip: input.tooltip ?? null }
            : {}),
          ...(Object.prototype.hasOwnProperty.call(input, 'isHidden')
            ? { is_hidden: input.isHidden }
            : {})
        })
      );
    });
  };

  const handleMoveNode = (nodeId: string, direction: -1 | 1) => {
    const siblingContext = findSiblingContext(pageTree, nodeId);
    if (!siblingContext) {
      return;
    }

    const targetIndex = siblingContext.index + direction;
    if (targetIndex < 0 || targetIndex >= siblingContext.siblings.length) {
      return;
    }

    setPageTree((currentTree) =>
      moveNodeInTree(currentTree, nodeId, direction)
    );
    void runPageTreeOperation(async () => {
      await onMovePageNode?.(nodeId, {
        parentId: siblingContext.parentId,
        rank: rankForMoveTarget(targetIndex, direction)
      });
    });
  };

  const handleMoveNodeToPosition = (
    nodeId: string,
    targetNodeId: string,
    position: 'before' | 'inside' | 'after'
  ) => {
    if (
      nodeId === targetNodeId ||
      isNodeDescendantOf(pageTree, nodeId, targetNodeId)
    ) {
      return;
    }

    const draggedNode = findNodeById(pageTree, nodeId);
    const targetNode = findNodeById(pageTree, targetNodeId);
    const targetSiblingContext = findSiblingContext(pageTree, targetNodeId);
    if (
      !draggedNode ||
      !targetNode ||
      !targetSiblingContext ||
      (draggedNode.kind === 'group' && targetSiblingContext.parentId)
    ) {
      return;
    }

    if (
      position === 'inside' &&
      (draggedNode.kind !== 'page' || targetNode.kind !== 'group')
    ) {
      return;
    }

    const { parentId, siblings, index } = targetSiblingContext;
    const nextParentId = position === 'inside' ? targetNodeId : parentId;
    const rank =
      position === 'inside'
        ? getNodeAppendRank(pageTree, targetNodeId)
        : position === 'before'
          ? rankForMoveTarget(index, -1)
          : index === siblings.length - 1
            ? getNodeAppendRank(pageTree, parentId)
            : rankForMoveTarget(index, 1);

    setPageTree((currentTree) =>
      moveNodeToTreePosition(currentTree, nodeId, targetNodeId, position)
    );
    void runPageTreeOperation(async () => {
      await onMovePageNode?.(nodeId, { parentId: nextParentId, rank });
    });
  };

  const handleMovePageToGroup = (
    nodeId: string,
    currentParentId: string | null,
    nextParentId: string | null
  ) => {
    if (currentParentId === nextParentId) {
      return;
    }

    void runPageTreeOperation(async () => {
      await onMovePageNode?.(nodeId, {
        parentId: nextParentId,
        rank: getNodeAppendRank(pageTree, nextParentId)
      });
    });
  };

  const handleSelectPage = (nodeId: string) => {
    if (selectedPageId === nodeId) {
      return;
    }

    setSelectedPageId(nodeId);
    onNavigatePage?.(nodeId);
  };

  return {
    pageTree,
    selectedPageId,
    selectedPageNode,
    operationStatus,
    pageTreeForm,
    pageTreeFormDialog,
    isPageTreeIconPickerOpen,
    setPageTreeFormDialog,
    setIsPageTreeIconPickerOpen,
    isOperationPending:
      operationStatus === 'pending' || Boolean(isPageTreeMutating),
    handleAddGroup,
    handleAddPage,
    handleAddPageInGroup,
    handleAddNodeAtPosition,
    handleDeleteNode,
    handleSubmitPageTreeForm,
    handleRenameNode,
    handlePageTabsEnabledChange,
    handleEditNodeTooltip,
    handleUpdateNodeMetadata,
    handleMoveNode,
    handleMoveNodeToPosition,
    handleMovePageToGroup,
    handleSelectPage
  };
}
