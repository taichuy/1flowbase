import {
  Alert,
  App as AntdApp,
  Button,
  Divider,
  Drawer,
  Empty,
  Form,
  Typography
} from 'antd';
import type { FC } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';

import { SectionPageLayout } from '../../../shared/ui/section-page-layout/SectionPageLayout';
import { useAuthStore } from '../../../state/auth-store';
import { useFrontstageDesignModeStore } from '../../../state/frontstage-design-mode-store';
import { saveFrontstageBlockCode } from '../api/block-code';
import type { FrontstagePageContent } from '../api/page-content';
import { AddBlockCatalogPickerDrawer } from '../components/AddBlockCatalogPickerDrawer';
import { BlockCodeEditorDrawer } from '../components/BlockCodeEditorDrawer';
import { BlockConfigurationDrawer } from '../components/BlockConfigurationDrawer';
import { FrontStagePageTreeSidebar } from '../components/FrontStagePageTreeSidebar';
import { FrontstagePageTabs } from '../components/FrontstagePageTabs';
import { JsBlockTrialPanel } from '../components/JsBlockTrialPanel';
import { PageCanvas } from '../components/PageCanvas';
import { useFrontstageBlockCatalog } from '../hooks/use-frontstage-block-catalog';
import { useFrontstageBlockCode } from '../hooks/use-frontstage-block-code';
import { useFrontstagePageCanvasRuntimeSessions } from '../hooks/use-frontstage-page-canvas-runtime-sessions';
import { useFrontstagePageCanvasRuntimeSources } from '../hooks/use-frontstage-page-canvas-runtime-sources';
import { useFrontstagePageContentSave } from '../hooks/use-frontstage-page-content-save';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import {
  appendFrontstageBlock,
  createFrontstageBlockCompositionState,
  moveFrontstageBlock,
  removeFrontstageBlock,
  updateFrontstageBlockLayout,
  type FrontstageBlockCompositionState
} from '../lib/block-composition';
import { createFrontstageBlockConfigurationModel } from '../lib/block-configuration';
import { createFrontstageJsBlockCapabilityHandlers } from '../lib/js-block-capability-handlers';
import {
  createFrontstagePageDocument,
  createFrontstagePageDocumentSaveInput
} from '../lib/page-document';
import { createFrontstagePageRenderPlan } from '../lib/page-canvas/render-plan';
import { createFrontstagePageCanvasRuntimeRunPlanState } from '../lib/page-canvas/runtime-run-plan';
import {
  findNodeById,
  getDeleteConfirmMessage,
  getPageDisplayTitle,
  moveNodeInTree,
  normalizePageTree,
  removeNodeFromTree,
  resolveSelectedPageId
} from '../lib/page-tree';
import type { FrontStageTreeNode } from '../lib/page-tree';
import type { RestrictedBlockLoaderLimits } from '../lib/restricted-block-loader';
import { i18nText } from '../../../shared/i18n/text';
import {
  createCatalogBlockInput,
  findMatchingFrontstageBlockCatalogEntry
} from './frontstage-page/block-catalog-helpers';
import { requireCsrfToken, toDisplayErrorMessage } from './frontstage-page/page-action-helpers';
import { DEFAULT_JS_BLOCK_TRIAL_LIMITS, DESIGN_MODE_PERMISSION } from './frontstage-page/page-constants';
import type { FrontStagePageProps } from './frontstage-page/page-props';
import {
  PageTreeFormModal,
  type PageTreeFormDialog,
  type PageTreeFormValues
} from './frontstage-page/page-tree-form-modal';
import {
  findSiblingContext,
  getNodeAppendRank,
  isNodeDescendantOf,
  moveNodeToTreePosition,
  rankForMoveTarget,
  updatePageTreeNode,
  type CreatePageTreeNodeInput,
  type PageTreeOperationStatus
} from './frontstage-page/page-tree-operations';
import './frontstage-page.css';

export const FrontStagePage: FC<FrontStagePageProps> = ({
  workspaceId,
  pageId,
  tabId,
  onNavigatePage,
  onNavigateTab,
  initialPageTree,
  isPageTreeLoading,
  hasPageTreeLoadError,
  onRetryLoadPageTree,
  pageContent,
  isPageContentLoading,
  hasPageContentLoadError,
  isPageContentPermissionDenied,
  onRetryLoadPageContent,
  isPageTreeMutating,
  pageTreeMutationError,
  onCreateGroupNode,
  onCreatePageNode,
  onRenamePageNode,
  onUpdatePageNodeMetadata,
  onMovePageNode,
  onDeletePageNode
}) => {
  const [pageTreeForm] = Form.useForm<PageTreeFormValues>();
  const { modal } = AntdApp.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken);
  const sessionStatus = useAuthStore((state) => state.sessionStatus);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const isDesignMode = useFrontstageDesignModeStore(
    (state) => state.isDesignMode
  );
  const setDesignMode = useFrontstageDesignModeStore(
    (state) => state.setDesignMode
  );
  const [operationStatus, setOperationStatus] =
    useState<PageTreeOperationStatus>('idle');
  const [pageTreeFormDialog, setPageTreeFormDialog] =
    useState<PageTreeFormDialog | null>(null);
  const [isPageTreeIconPickerOpen, setIsPageTreeIconPickerOpen] =
    useState(false);
  const [selectedBlockId, setSelectedBlockId] = useState<string | null>(null);
  const [isBlockCodeEditorOpen, setIsBlockCodeEditorOpen] = useState(false);
  const [isBlockConfigurationOpen, setIsBlockConfigurationOpen] =
    useState(false);
  const [isJsBlockTrialPanelOpen, setIsJsBlockTrialPanelOpen] = useState(false);
  const [jsBlockTrialContextSnapshot, setJsBlockTrialContextSnapshot] =
    useState<Record<string, unknown>>({});
  const [jsBlockTrialLimits, setJsBlockTrialLimits] =
    useState<RestrictedBlockLoaderLimits>(DEFAULT_JS_BLOCK_TRIAL_LIMITS);
  const [savedPageContent, setSavedPageContent] =
    useState<FrontstagePageContent | null>(null);
  const [isBlockSavePending, setIsBlockSavePending] = useState(false);
  const [blockSaveError, setBlockSaveError] = useState<string | null>(null);
  const [isAddBlockPickerOpen, setIsAddBlockPickerOpen] = useState(false);
  const [pageTree, setPageTree] = useState<FrontStageTreeNode[]>(() =>
    normalizePageTree(initialPageTree ?? [])
  );
  const [selectedPageId, setSelectedPageId] = useState<string | null>(
    () =>
      resolveSelectedPageId({
        pageId,
        pageTree: normalizePageTree(initialPageTree ?? [])
      }).selectedPageId
  );
  const blockCatalog = useFrontstageBlockCatalog({ workspaceId });
  const pageContentSave = useFrontstagePageContentSave({
    workspaceId,
    pageId: selectedPageId,
    tabId
  });
  const jsBlockCapabilityHandlers = useMemo(
    () =>
      selectedPageId && tabId
        ? createFrontstageJsBlockCapabilityHandlers({
            workspaceId,
            pageId: selectedPageId,
            tabId,
            csrfToken
          })
        : undefined,
    [csrfToken, selectedPageId, tabId, workspaceId]
  );
  const displayedPageContent = savedPageContent ?? pageContent;
  const hasLoadedSelectedPageContent = Boolean(
    selectedPageId && displayedPageContent?.page.id === selectedPageId
  );
  const activePageContent = hasLoadedSelectedPageContent
    ? displayedPageContent
    : undefined;
  const displayedPageDocument = useMemo(
    () =>
      activePageContent
        ? createFrontstagePageDocument(activePageContent)
        : null,
    [activePageContent]
  );
  const activePageRenderPlan = useMemo(
    () =>
      displayedPageDocument
        ? createFrontstagePageRenderPlan(displayedPageDocument)
        : null,
    [displayedPageDocument]
  );
  const pageCanvasRuntimeSources = useFrontstagePageCanvasRuntimeSources({
    workspaceId,
    renderPlan: activePageRenderPlan
  });
  const pageCanvasRuntimeRunPlanState = useMemo(() => {
    const sourceState = pageCanvasRuntimeSources.sourceState;
    if (!sourceState) {
      return null;
    }

    return createFrontstagePageCanvasRuntimeRunPlanState({
      sourceState,
      catalogEntries: blockCatalog.items,
      contextSnapshot: (source) => ({
        workspaceId,
        pageId: activePageContent?.page.id ?? sourceState.pageId,
        pageTitle: activePageContent?.page.title ?? null,
        blockId: source.blockId,
        codeRef: source.codeRef,
        props: source.block.props
      }),
      limits: jsBlockTrialLimits
    });
  }, [
    activePageContent?.page.id,
    activePageContent?.page.title,
    blockCatalog.items,
    jsBlockTrialLimits,
    pageCanvasRuntimeSources.sourceState,
    workspaceId
  ]);
  const pageCanvasRuntimeSessions = useFrontstagePageCanvasRuntimeSessions({
    runtimeRunPlanState: pageCanvasRuntimeRunPlanState,
    handlers: jsBlockCapabilityHandlers
  });
  const blockCompositionState = useMemo(
    () =>
      displayedPageDocument
        ? createFrontstageBlockCompositionState(
            displayedPageDocument,
            selectedBlockId
          )
        : null,
    [displayedPageDocument, selectedBlockId]
  );
  const isOperationPending =
    operationStatus === 'pending' || Boolean(isPageTreeMutating);
  const hasOperationError =
    operationStatus === 'error' || Boolean(pageTreeMutationError);
  const isPageContentSavePending =
    isBlockSavePending || pageContentSave.saving || pageContentSave.isPending;
  const pageContentSaveError =
    blockSaveError ??
    (pageContentSave.error
      ? toDisplayErrorMessage(pageContentSave.error)
      : null);
  const canAddBlock =
    Boolean(activePageContent) &&
    !isPageContentLoading &&
    !hasPageContentLoadError &&
    !isPageContentSavePending;
  const operationStatusText = isOperationPending
    ? i18nText("frontstage", "auto.saving")
    : hasOperationError
      ? i18nText("frontstage", "auto.operation_failed")
      : i18nText("frontstage", "auto.page_tree_synced");

  const canEnterDesignMode = useMemo(() => {
    return (
      actor?.effective_display_role === 'root' ||
      Boolean(me?.permissions.includes(DESIGN_MODE_PERMISSION))
    );
  }, [actor, me]);
  const hasResolvedDesignModePermission = sessionStatus !== 'unknown';
  const selectedBlockIndex =
    blockCompositionState?.selectedBlockId === selectedBlockId
      ? blockCompositionState.document.blocks.findIndex(
          (block) => block.id === selectedBlockId
        )
      : -1;
  const selectedBlock =
    selectedBlockIndex >= 0
      ? blockCompositionState?.document.blocks[selectedBlockIndex]
      : null;
  const canShowSelectedBlockActions = Boolean(
    canEnterDesignMode &&
    isDesignMode &&
    activePageContent &&
    blockCompositionState &&
    selectedBlock
  );
  const canRunSelectedBlockAction =
    canShowSelectedBlockActions &&
    !isPageContentLoading &&
    !hasPageContentLoadError &&
    !isPageContentSavePending;
  const selectedBlockCode = useFrontstageBlockCode({
    workspaceId: canShowSelectedBlockActions ? workspaceId : null,
    pageId: canShowSelectedBlockActions ? selectedPageId : null,
    codeRef: canShowSelectedBlockActions ? selectedBlock?.codeRef : null
  });
  const matchingJsBlockCatalogEntry = useMemo(
    () =>
      findMatchingFrontstageBlockCatalogEntry(
        selectedBlock,
        blockCatalog.items
      ),
    [blockCatalog.items, selectedBlock]
  );
  const defaultJsBlockTrialContextSnapshot = useMemo(
    () => ({
      workspaceId,
      pageId: activePageContent?.page.id ?? selectedPageId,
      pageTitle: activePageContent?.page.title ?? null,
      blockId: selectedBlock?.id ?? null,
      blockCodeRef: selectedBlock?.codeRef ?? null,
      props: selectedBlock?.props ?? {}
    }),
    [activePageContent, selectedBlock, selectedPageId, workspaceId]
  );
  const selectedBlockConfigurationModel = useMemo(
    () =>
      selectedBlock
        ? createFrontstageBlockConfigurationModel({
            block: selectedBlock,
            catalogEntry: matchingJsBlockCatalogEntry,
            limits: jsBlockTrialLimits
          })
        : null,
    [jsBlockTrialLimits, matchingJsBlockCatalogEntry, selectedBlock]
  );
  useEffect(() => {
    const resolution = resolveSelectedPageId({
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
  }, [onNavigatePage, pageId, pageTree, selectedPageId]);

  useEffect(() => {
    if (!initialPageTree) {
      return;
    }

    setPageTree(normalizePageTree(initialPageTree));
    setOperationStatus('idle');
  }, [initialPageTree]);

  useEffect(() => {
    setSavedPageContent(null);
    setSelectedBlockId(null);
    setIsBlockCodeEditorOpen(false);
    setIsBlockConfigurationOpen(false);
    setIsJsBlockTrialPanelOpen(false);
    setIsAddBlockPickerOpen(false);
    setBlockSaveError(null);
  }, [selectedPageId]);

  useEffect(() => {
    setSavedPageContent(null);
    setSelectedBlockId((currentBlockId) => {
      if (!currentBlockId || !pageContent) {
        setIsBlockCodeEditorOpen(false);
        setIsBlockConfigurationOpen(false);
        setIsJsBlockTrialPanelOpen(false);
        setIsAddBlockPickerOpen(false);
        return null;
      }

      const document = createFrontstagePageDocument(pageContent);
      const hasCurrentBlock = document.blocks.some(
        (block) => block.id === currentBlockId
      );
      if (!hasCurrentBlock) {
        setIsBlockCodeEditorOpen(false);
        setIsBlockConfigurationOpen(false);
        setIsJsBlockTrialPanelOpen(false);
      }

      return hasCurrentBlock ? currentBlockId : null;
    });
  }, [pageContent]);

  useEffect(() => {
    if (!canShowSelectedBlockActions) {
      setIsBlockCodeEditorOpen(false);
      setIsBlockConfigurationOpen(false);
      setIsJsBlockTrialPanelOpen(false);
    }
  }, [canShowSelectedBlockActions]);

  useEffect(() => {
    if (
      hasResolvedDesignModePermission &&
      !canEnterDesignMode &&
      isDesignMode
    ) {
      setDesignMode(false);
    }
  }, [
    canEnterDesignMode,
    hasResolvedDesignModePermission,
    isDesignMode,
    setDesignMode
  ]);

  useEffect(() => {
    if (!canEnterDesignMode || !isDesignMode) {
      setSelectedBlockId(null);
      setIsBlockCodeEditorOpen(false);
      setIsBlockConfigurationOpen(false);
      setIsJsBlockTrialPanelOpen(false);
      setIsAddBlockPickerOpen(false);
    }
  }, [canEnterDesignMode, isDesignMode]);

  useEffect(() => {
    setJsBlockTrialContextSnapshot(defaultJsBlockTrialContextSnapshot);
    setJsBlockTrialLimits(DEFAULT_JS_BLOCK_TRIAL_LIMITS);
  }, [defaultJsBlockTrialContextSnapshot]);

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
      tooltip: pageTreeFormDialog.initialTooltip
    });
  }, [pageTreeForm, pageTreeFormDialog]);

  const selectedPageDisplayTitle = getPageDisplayTitle(
    pageTree,
    selectedPageId
  );
  const selectedPageLabel = selectedPageDisplayTitle
    ? selectedPageDisplayTitle
    : selectedPageId
      ? i18nText("frontstage", "auto.page_with_id", { value1: selectedPageId })
      : null;
  const pageLabel = selectedPageLabel
    ? selectedPageLabel
    : i18nText("frontstage", "auto.default_home_page_notice");

  const saveBlockComposition = useCallback(
    async (
      sourceContent: FrontstagePageContent,
      compositionState: FrontstageBlockCompositionState
    ) => {
      setIsBlockSavePending(true);
      setBlockSaveError(null);
      pageContentSave.clearError();

      try {
        const input = createFrontstagePageDocumentSaveInput(
          sourceContent,
          compositionState.document
        );
        const nextContent = await pageContentSave.save(input);

        setSavedPageContent(nextContent);
        setSelectedBlockId(compositionState.selectedBlockId);
      } catch (error) {
        setBlockSaveError(toDisplayErrorMessage(error));
      } finally {
        setIsBlockSavePending(false);
      }
    },
    [pageContentSave]
  );

  const designActions = useMemo(() => {
    if (!canEnterDesignMode || !isDesignMode) {
      return undefined;
    }

    const renderItems = activePageRenderPlan?.items ?? [];

    return {
      onMoveUp: (blockId: string) => {
        const idx = renderItems.findIndex((item) => item.blockId === blockId);
        if (idx <= 0 || !blockCompositionState || !activePageContent) return;
        const next = moveFrontstageBlock(
          blockCompositionState,
          blockId,
          idx - 1
        );
        void saveBlockComposition(activePageContent, next);
      },
      onMoveDown: (blockId: string) => {
        const idx = renderItems.findIndex((item) => item.blockId === blockId);
        if (
          idx < 0 ||
          idx >= renderItems.length - 1 ||
          !blockCompositionState ||
          !activePageContent
        )
          return;
        const next = moveFrontstageBlock(
          blockCompositionState,
          blockId,
          idx + 1
        );
        void saveBlockComposition(activePageContent, next);
      },
      onConfigure: (blockId: string) => {
        setSelectedBlockId(blockId);
        setIsBlockConfigurationOpen(true);
      },
      onEditCode: (blockId: string) => {
        setSelectedBlockId(blockId);
        setIsBlockCodeEditorOpen(true);
      },
      onDelete: (blockId: string) => {
        if (!blockCompositionState || !activePageContent) return;
        const next = removeFrontstageBlock(blockCompositionState, blockId);
        void saveBlockComposition(activePageContent, next);
      }
    };
  }, [
    canEnterDesignMode,
    isDesignMode,
    activePageRenderPlan?.items,
    blockCompositionState,
    activePageContent,
    saveBlockComposition,
    setSelectedBlockId,
    setIsBlockConfigurationOpen,
    setIsBlockCodeEditorOpen
  ]);

  if (initialPageTree === undefined && isPageTreeLoading) {
    return (
      <SectionPageLayout
        pageTitle={i18nText("frontstage", "auto.frontstage")}
        navItems={[]}
        activeKey=""
        contentWidth="wide"
        heightMode="viewport"
        sidebarContent={
          <Typography.Text type="secondary" style={{ paddingInline: 16 }}>
            {i18nText("frontstage", "auto.page_tree_loading")}</Typography.Text>
        }
      >
        <section className="frontstage-page-workspace">
          <header className="frontstage-page-workspace__header">
            <Typography.Title
              className="frontstage-page-workspace__title"
              level={3}
            >
              {i18nText("frontstage", "auto.page_tree_loading_ellipsis")}</Typography.Title>
          </header>
          <Divider style={{ margin: 0 }} />
          <div className="frontstage-page-workspace__body">
            <Empty
              description={
                <Typography.Text>{i18nText("frontstage", "auto.page_tree_loading_wait")}</Typography.Text>
              }
            />
          </div>
        </section>
      </SectionPageLayout>
    );
  }

  if (initialPageTree === undefined && hasPageTreeLoadError) {
    return (
      <SectionPageLayout
        pageTitle={i18nText("frontstage", "auto.frontstage")}
        navItems={[]}
        activeKey=""
        contentWidth="wide"
        heightMode="viewport"
        sidebarContent={
          <Typography.Text type="secondary" style={{ paddingInline: 16 }}>
            {i18nText("frontstage", "auto.page_tree_unavailable")}</Typography.Text>
        }
      >
        <section className="frontstage-page-workspace">
          <header className="frontstage-page-workspace__header">
            <Typography.Title
              className="frontstage-page-workspace__title"
              level={3}
            >
              {i18nText("frontstage", "auto.page_tree_load_failed")}</Typography.Title>
          </header>
          <Divider style={{ margin: 0 }} />
          <div className="frontstage-page-workspace__body">
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                <Typography.Text>
                  {i18nText("frontstage", "auto.page_tree_load_failed_retry")}</Typography.Text>
              }
            >
              <Button type="primary" onClick={onRetryLoadPageTree}>
                {i18nText("frontstage", "auto.retry")}</Button>
            </Empty>
          </div>
        </section>
      </SectionPageLayout>
    );
  }

  const renderPageTreeErrorBanner = hasPageTreeLoadError ? (
    <Alert
      style={{ marginBottom: 12 }}
      message={i18nText("frontstage", "auto.page_tree_load_failed")}
      description={i18nText("frontstage", "auto.page_tree_load_failed_recover")}
      type="error"
      showIcon
      action={
        onRetryLoadPageTree ? (
          <Button size="small" onClick={() => onRetryLoadPageTree()}>
            {i18nText("frontstage", "auto.retry")}</Button>
        ) : null
      }
    />
  ) : null;

  const runPageTreeOperation = async <T,>(
    operation: () => Promise<T | void>
  ): Promise<T | null> => {
    setOperationStatus('pending');

    try {
      const result = await operation();
      setOperationStatus('idle');
      return result ?? null;
    } catch {
      setOperationStatus('error');
      return null;
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
      title: nodeKind === 'page' ? i18nText("frontstage", "auto.add_page") : i18nText("frontstage", "auto.add_group")
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
    } else {
      if (index === siblings.length - 1) {
        rank = getNodeAppendRank(pageTree, parentId);
      } else {
        rank = rankForMoveTarget(index, 1);
      }
    }

    openCreateNodeDialog(kind, parentId, rank);
  };

  const handleDeleteNode = (nodeId: string) => {
    const node = findNodeById(pageTree, nodeId);
    if (!node) {
      return;
    }

    modal.confirm({
      title: i18nText("frontstage", "auto.delete_node"),
      content: getDeleteConfirmMessage(node),
      okText: i18nText("frontstage", "auto.delete"),
      okButtonProps: { danger: true },
      cancelText: i18nText("frontstage", "auto.cancel"),
      onOk: async () => {
        await runPageTreeOperation(async () => {
          await onDeletePageNode?.(nodeId);
          const next = removeNodeFromTree(pageTree, nodeId);
          const nextResolution = resolveSelectedPageId({
            currentSelectedPageId: selectedPageId,
            pageTree: next
          });
          const nextSelectedPageId = nextResolution.selectedPageId;

          setSelectedPageId(nextSelectedPageId);
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
      const title = values.title ?? '';
      const input = {
        title,
        icon: values.icon ?? null,
        tooltip: values.tooltip ?? null,
        parentId: dialog.parentId,
        rank: dialog.rank
      };

      await runPageTreeOperation(async () => {
        await createPageTreeNode(dialog.nodeKind, input);
      });

      setPageTreeFormDialog(null);
      return;
    }

    if (dialog.kind === 'rename') {
      const title = values.title ?? '';
      const icon = values.icon ?? null;
      const tooltip = values.tooltip ?? null;

      await runPageTreeOperation(async () => {
        await onRenamePageNode?.(dialog.nodeId, {
          title,
          icon,
          tooltip
        });
        setPageTree((currentTree) =>
          updatePageTreeNode(currentTree, dialog.nodeId, {
            title,
            icon,
            tooltip
          })
        );
      });
      setPageTreeFormDialog(null);
      return;
    }

    await runPageTreeOperation(async () => {
      await onUpdatePageNodeMetadata?.(dialog.nodeId, {
        tooltip: values.tooltip ?? ''
      });
    });
    setPageTreeFormDialog(null);
  };

  const handleRenameNode = (node: FrontStageTreeNode) => {
    setPageTreeFormDialog({
      kind: 'rename',
      nodeId: node.id,
      initialTitle: node.title ?? '',
      initialIcon: node.icon ?? '',
      initialTooltip: node.tooltip ?? '',
      title: i18nText("frontstage", "auto.edit_node")
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
      title: i18nText("frontstage", "auto.edit_description")
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
      await onMovePageNode?.(nodeId, {
        parentId: nextParentId,
        rank
      });
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

  const handleAddBlock = () => {
    if (!canAddBlock) {
      return;
    }

    setBlockSaveError(null);
    pageContentSave.clearError();
    setIsAddBlockPickerOpen(true);
  };

  const handleSelectBlockCatalogEntry = async (
    entry: NormalizedFrontstageBlockCatalogEntry
  ) => {
    const sourceContent = activePageContent;
    if (!canAddBlock || !sourceContent || !blockCompositionState) {
      return;
    }

    const codeTemplate = entry.codeCapabilities?.template;
    if (!codeTemplate) {
      setBlockSaveError(
        i18nText("frontstage", "auto.catalog_entry_missing_code_template")
      );
      return;
    }

    const nextBlockInput = createCatalogBlockInput(
      entry,
      blockCompositionState.document.blocks.length
    );
    const nextCompositionState = appendFrontstageBlock(
      blockCompositionState,
      nextBlockInput
    );

    setIsBlockSavePending(true);
    setBlockSaveError(null);
    pageContentSave.clearError();

    try {
      const input = createFrontstagePageDocumentSaveInput(
        sourceContent,
        nextCompositionState.document
      );
      const nextContent = await pageContentSave.save(input);
      const createdBlock =
        nextCompositionState.document.blocks.find(
          (block) => block.id === nextCompositionState.selectedBlockId
        ) ??
        nextCompositionState.document.blocks[
          nextCompositionState.document.blocks.length - 1
        ];
      if (!createdBlock) {
        throw new Error('created block is missing');
      }

      const codeRef = createdBlock.codeRef;

      await saveFrontstageBlockCode(
        workspaceId,
        selectedPageId ?? sourceContent.page.id,
        {
          codeRef,
          code: codeTemplate.source
        },
        requireCsrfToken(csrfToken)
      );

      setSavedPageContent(nextContent);
      setSelectedBlockId(nextCompositionState.selectedBlockId);
      setIsAddBlockPickerOpen(false);
    } catch (error) {
      setBlockSaveError(toDisplayErrorMessage(error));
    } finally {
      setIsBlockSavePending(false);
    }
  };

  const handleOpenJsBlockTrialPanel = () => {
    if (!canRunSelectedBlockAction) {
      return;
    }

    setJsBlockTrialContextSnapshot(defaultJsBlockTrialContextSnapshot);
    setJsBlockTrialLimits(DEFAULT_JS_BLOCK_TRIAL_LIMITS);
    setIsJsBlockTrialPanelOpen(true);
  };

  const canEditPageTree = canEnterDesignMode && isDesignMode;
  const frontstageSidebar = (
    <FrontStagePageTreeSidebar
      pageTree={pageTree}
      selectedPageId={selectedPageId}
      canEdit={canEditPageTree}
      isOperationPending={isOperationPending}
      onAddGroup={handleAddGroup}
      onAddPage={handleAddPage}
      onAddPageInGroup={handleAddPageInGroup}
      onRenameNode={handleRenameNode}
      onUpdateNodeMetadata={handleUpdateNodeMetadata}
      onEditNodeTooltip={handleEditNodeTooltip}
      onMoveNode={handleMoveNode}
      onAddNodeAtPosition={handleAddNodeAtPosition}
      onMoveNodeToPosition={handleMoveNodeToPosition}
      onMovePageToGroup={handleMovePageToGroup}
      onDeleteNode={handleDeleteNode}
      onSelectPage={handleSelectPage}
    />
  );

  return (
    <SectionPageLayout
      navItems={[]}
      activeKey=""
      contentWidth="wide"
      heightMode="viewport"
      sidebarContent={frontstageSidebar}
    >
      <>
        <section className="frontstage-page-workspace">
          <header className="frontstage-page-workspace__header">
            <Typography.Title
              className="frontstage-page-workspace__title"
              level={3}
            >
              {pageLabel}
            </Typography.Title>
          </header>
          <Divider style={{ margin: 0 }} />
          <div className="frontstage-page-workspace__body">
            {renderPageTreeErrorBanner}

            {selectedPageId && tabId && onNavigateTab ? (
              <FrontstagePageTabs
                workspaceId={workspaceId}
                pageId={selectedPageId}
                tabId={tabId}
                isDesignMode={canEnterDesignMode && isDesignMode}
                onNavigateTab={onNavigateTab}
              />
            ) : null}

            {canEnterDesignMode && isDesignMode && isPageContentSavePending ? (
              <Typography.Text
                type="secondary"
                style={{ marginBottom: 12, display: 'block' }}
              >
                {i18nText("frontstage", "auto.block_saving")}</Typography.Text>
            ) : null}
            {canEnterDesignMode && isDesignMode && pageContentSaveError ? (
              <Alert
                style={{ marginBottom: 12 }}
                message={i18nText("frontstage", "auto.block_save_failed")}
                description={pageContentSaveError}
                type="error"
                showIcon
              />
            ) : null}
            {canEnterDesignMode && isDesignMode ? (
              <Typography.Text
                type={hasOperationError ? 'danger' : 'secondary'}
                style={{ marginBottom: 12, display: 'block' }}
              >
                {operationStatusText}
              </Typography.Text>
            ) : null}
            <PageCanvas
              content={
                selectedPageLabel && hasLoadedSelectedPageContent
                  ? displayedPageContent
                  : undefined
              }
              isLoading={Boolean(selectedPageLabel && isPageContentLoading)}
              hasError={Boolean(selectedPageLabel && hasPageContentLoadError)}
              isPermissionDenied={Boolean(
                selectedPageLabel && isPageContentPermissionDenied
              )}
              selectedBlockId={
                canEnterDesignMode && isDesignMode ? selectedBlockId : null
              }
              onSelectBlock={
                canEnterDesignMode && isDesignMode
                  ? (blockId) => {
                      setSelectedBlockId((currentBlockId) =>
                        currentBlockId === blockId ? null : blockId
                      );
                    }
                  : undefined
              }
              onRetry={onRetryLoadPageContent}
              runtimeSourceState={pageCanvasRuntimeSources.sourceState}
              runtimeRunPlanState={pageCanvasRuntimeRunPlanState}
              runtimeSessionEntries={pageCanvasRuntimeSessions.entries}
              isDesignMode={canEnterDesignMode && isDesignMode}
              designActions={designActions}
              toolbarDisabled={isPageContentSavePending}
              onResponsiveLayoutSave={
                canEnterDesignMode && isDesignMode
                  ? (layouts) => {
                      if (!blockCompositionState || !activePageContent) return;
                      let next = blockCompositionState;
                      for (const [blockId, blockLayouts] of Object.entries(layouts)) {
                        next = updateFrontstageBlockLayout(next, blockId, blockLayouts);
                      }
                      void saveBlockComposition(activePageContent, next);
                    }
                  : undefined
              }
              showTitle={false}
            />
            {canEnterDesignMode && isDesignMode ? (
              <Button
                size="middle"
                aria-label={i18nText("frontstage", "auto.create_block")}
                onClick={handleAddBlock}
                disabled={!canAddBlock}
                style={{
                  marginTop: 20,
                  borderStyle: 'dashed',
                  borderColor: '#20d48a',
                  color: '#00a86b'
                }}
              >
                {i18nText("frontstage", "auto.add_block_button")}</Button>
            ) : null}
          </div>
          <Drawer
            title={i18nText("frontstage", "auto.js_block_trial_run")}
            open={isJsBlockTrialPanelOpen}
            onClose={() => setIsJsBlockTrialPanelOpen(false)}
            width={600}
            destroyOnClose
          >
            {selectedBlock && (
              <JsBlockTrialPanel
                block={selectedBlock}
                catalogEntry={matchingJsBlockCatalogEntry}
                code={selectedBlockCode.draft}
                contextSnapshot={jsBlockTrialContextSnapshot}
                handlers={jsBlockCapabilityHandlers}
                limits={jsBlockTrialLimits}
                onCodeChange={selectedBlockCode.setDraft}
                onContextSnapshotChange={setJsBlockTrialContextSnapshot}
                onLimitsChange={setJsBlockTrialLimits}
              />
            )}
          </Drawer>
        </section>
        <PageTreeFormModal
          dialog={pageTreeFormDialog}
          form={pageTreeForm}
          iconPickerOpen={isPageTreeIconPickerOpen}
          isOperationPending={isOperationPending}
          onCancel={() => setPageTreeFormDialog(null)}
          onIconPickerOpenChange={setIsPageTreeIconPickerOpen}
          onSubmit={() => {
            void handleSubmitPageTreeForm();
          }}
        />
        {canEnterDesignMode && isDesignMode ? (
          <AddBlockCatalogPickerDrawer
            open={isAddBlockPickerOpen}
            items={blockCatalog.items}
            loading={blockCatalog.loading}
            error={blockCatalog.error}
            saving={isPageContentSavePending}
            onSelect={(entry) => {
              void handleSelectBlockCatalogEntry(entry);
            }}
            onClose={() => setIsAddBlockPickerOpen(false)}
          />
        ) : null}
        <BlockCodeEditorDrawer
          open={isBlockCodeEditorOpen && canShowSelectedBlockActions}
          onClose={() => setIsBlockCodeEditorOpen(false)}
          onOpenTrialPanel={handleOpenJsBlockTrialPanel}
          workspaceId={workspaceId}
          pageId={selectedPageId}
          block={canShowSelectedBlockActions ? selectedBlock : null}
        />
        <BlockConfigurationDrawer
          open={isBlockConfigurationOpen && canShowSelectedBlockActions}
          onClose={() => setIsBlockConfigurationOpen(false)}
          model={
            canShowSelectedBlockActions ? selectedBlockConfigurationModel : null
          }
        />
      </>
    </SectionPageLayout>
  );
};
