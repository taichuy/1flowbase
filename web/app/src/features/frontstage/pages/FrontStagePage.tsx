import {
  Alert,
  App as AntdApp,
  Button,
  Divider,
  Drawer,
  Empty,
  Modal,
  Typography
} from 'antd';
import type { CSSProperties, FC, ReactNode } from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createNativeBlockContextCapabilities } from '@1flowbase/page-runtime';

import { SectionPageLayout } from '../../../shared/ui/section-page-layout/SectionPageLayout';
import { useAuthStore } from '../../../state/auth-store';
import { useFrontstageDesignModeStore } from '../../../state/frontstage-design-mode-store';
import type { FrontstagePageContent } from '../api/page-content';
import { FrontStagePageTreeSidebar } from '../components/FrontStagePageTreeSidebar';
import { FrontstagePageTabs } from '../components/FrontstagePageTabs';
import {
  JsxStudioRunPanel,
  type JsxStudioRunBlockContextInput
} from '../components/jsx-studio/JsxStudioRunPanel';
import {
  PageCanvas,
  type FrontstagePageCanvasRuntimeContext
} from '../components/PageCanvas';
import { FrontstageJsxStudioDrawer } from '../components/jsx-studio/FrontstageJsxStudioDrawer';
import { useFrontstageBlockCatalog } from '../hooks/use-frontstage-block-catalog';
import { useFrontstagePageCanvasNativePreparations } from '../hooks/use-frontstage-page-canvas-native-preparations';
import { useFrontstagePageContentSave } from '../hooks/use-frontstage-page-content-save';
import {
  appendFrontstageBlock,
  createFrontstageBlockCompositionState,
  removeFrontstageBlock,
  updateFrontstageBlock,
  updateFrontstageBlockLayout,
  updateFrontstageBlockPresentation,
  updateFrontstagePageLayoutMode,
  type FrontstageBlockCompositionState
} from '../lib/block-composition';
import { resolveFrontstageNativeDependencyLock } from '../lib/block-catalog';
import type { ChildContainerNode } from '../lib/child-container-tree';
import {
  getRootPageBlockIds,
  resolveChildContainerCloseTarget,
  resolveChildContainerEvent,
  resolveChildContainerRuntime
} from '../lib/child-container-runtime';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';
import { createFrontstageJsBlockCapabilityHandlers } from '../lib/js-block-capability-handlers';
import { createFrontstageUnavailableBlockContext } from '../lib/native-trusted-block-react-adapter';
import {
  createFrontstagePageDocument,
  createFrontstagePageDocumentSaveInput,
  type FrontstageBlockInstance,
  type FrontstageBlockPresentation,
  type FrontstagePageLayoutMode
} from '../lib/page-document';
import { createFrontstagePageRenderPlan } from '../lib/page-canvas/render-plan';
import { createFrontstagePageCanvasBlockCodeReadPlan } from '../lib/page-canvas/runtime-source';
import type {
  FrontstageRuntimeDemandByBlockId,
  FrontstageRuntimeDemandPriority
} from '../lib/page-canvas/runtime-demand';
import type { FrontstageNativeBlockContextHost } from '../lib/page-canvas/native-block-context-host';
import { recordFrontstageRuntimeObservation } from '../lib/page-canvas/runtime-observation';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator
} from '../lib/page-canvas/signal-runtime';
import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  normalizeFrontstageAutomaticResponsiveLayouts,
  type FrontstagePersistedGridLayout
} from '../lib/responsive-grid-layout';
import { getPageDisplayTitle } from '../lib/page-tree';
import { i18nText } from '../../../shared/i18n/text';
import {
  createCatalogBlockInput,
  findMatchingFrontstageBlockCatalogEntry
} from './frontstage-page/block-catalog-helpers';
import { toDisplayErrorMessage } from './frontstage-page/page-action-helpers';
import { DESIGN_MODE_PERMISSION } from './frontstage-page/page-constants';
import type { FrontStagePageProps } from './frontstage-page/page-props';
import { PageTreeFormModal } from './frontstage-page/page-tree-form-modal';
import { PageWorkspaceActionMenu } from './frontstage-page/PageWorkspaceActionMenu';
import { usePageTreeWorkspace } from './frontstage-page/use-page-tree-workspace';
import './frontstage-page.css';

type FrontStagePageContainerProps = FrontStagePageProps & {
  containerId?: string;
  onContainerIdChange?: (
    containerId: string | null,
    mode: 'push' | 'replace'
  ) => void;
};
const EMPTY_CHILD_CONTAINERS: readonly ChildContainerNode[] = [];

export const FrontStagePage: FC<FrontStagePageContainerProps> = ({
  workspaceId,
  pageId,
  tabId,
  showSidebar = true,
  autoSelectFirstPage = true,
  onNavigatePage,
  onNavigateTab,
  containerId,
  onContainerIdChange,
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
  const {
    pageTree,
    selectedPageId,
    selectedPageNode,
    operationStatus,
    pageTreeForm,
    pageTreeFormDialog,
    isPageTreeIconPickerOpen,
    setPageTreeFormDialog,
    setIsPageTreeIconPickerOpen,
    isOperationPending,
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
  } = usePageTreeWorkspace({
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
  });
  const [selectedBlockId, setSelectedBlockId] = useState<string | null>(null);
  const [isJsxStudioOpen, setIsJsxStudioOpen] = useState(false);
  const [savedPageContent, setSavedPageContent] =
    useState<FrontstagePageContent | null>(null);
  const [isBlockSavePending, setIsBlockSavePending] = useState(false);
  const blockCreationPendingRef = useRef(false);
  const [blockSaveError, setBlockSaveError] = useState<string | null>(null);
  const [childContainerRuntimeError, setChildContainerRuntimeError] = useState<
    string | null
  >(null);
  const blockCatalog = useFrontstageBlockCatalog({ workspaceId });
  const pageContentSave = useFrontstagePageContentSave({
    workspaceId,
    pageId: selectedPageId,
    tabId
  });
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
  const childContainers =
    displayedPageDocument?.childContainers ?? EMPTY_CHILD_CONTAINERS;
  const childContainerRuntime = useMemo(
    () => resolveChildContainerRuntime(childContainers, containerId),
    [childContainers, containerId]
  );
  const rootPageBlockIds = useMemo(
    () =>
      getRootPageBlockIds(
        (displayedPageDocument?.blocks ?? []).map((block) => block.id),
        childContainers
      ),
    [childContainers, displayedPageDocument?.blocks]
  );
  const pageSignalSession = useMemo(
    () => createFrontstagePageSignalSession(),
    [activePageContent?.tab.id]
  );
  const pageSignalCoordinator = useMemo(
    () =>
      displayedPageDocument && activePageContent
        ? new FrontstageSignalRuntimeCoordinator(
            displayedPageDocument.blocks,
            activePageContent.tab.id,
            pageSignalSession
          )
        : undefined,
    [
      activePageContent?.tab.id,
      displayedPageDocument?.rootUid,
      pageSignalSession
    ]
  );
  useEffect(() => {
    pageSignalCoordinator?.updateBlocks(displayedPageDocument?.blocks ?? []);
  }, [displayedPageDocument?.blocks, pageSignalCoordinator]);
  useEffect(
    () => () => pageSignalCoordinator?.dispose(),
    [pageSignalCoordinator]
  );
  useEffect(() => {
    setChildContainerRuntimeError(
      childContainerRuntime.diagnostics[0]?.message ?? null
    );
  }, [childContainerRuntime.diagnostics]);
  const openChildContainer = useCallback(
    (
      sourceBlockId: string,
      name: string,
      payload?: Record<string, unknown>
    ) => {
      const resolution = resolveChildContainerEvent(childContainers, {
        sourceBlockId,
        sourceTargetContainerIds:
          displayedPageDocument?.blocks.find(({ id }) => id === sourceBlockId)
            ?.childContainerTargetIds ?? [],
        name,
        payload
      });
      if (resolution.diagnostic) {
        setChildContainerRuntimeError(resolution.diagnostic.message);
        return;
      }
      if (resolution.containerId) {
        setChildContainerRuntimeError(null);
        onContainerIdChange?.(resolution.containerId, 'push');
      }
    },
    [childContainers, displayedPageDocument?.blocks, onContainerIdChange]
  );
  const confirmRuntimeWrite = useCallback(
    ({ method, path }: { method: string; path: string }) =>
      new Promise<boolean>((resolve) => {
        modal.confirm({
          title: i18nText('frontstage', 'auto.confirm_runtime_write'),
          content: i18nText(
            'frontstage',
            'auto.confirm_runtime_write_description',
            { value1: method.toUpperCase(), value2: path }
          ),
          okText: i18nText('frontstage', 'auto.confirm'),
          cancelText: i18nText('frontstage', 'auto.cancel'),
          onOk: () => resolve(true),
          onCancel: () => resolve(false)
        });
      }),
    [modal]
  );
  const jsBlockCapabilityHandlers = useMemo(
    () =>
      selectedPageId && tabId
        ? createFrontstageJsBlockCapabilityHandlers({
            workspaceId,
            pageId: selectedPageId,
            tabId,
            csrfToken,
            confirmRuntimeWrite,
            resolveBlockId: (requestId) => {
              const blockId = requestId.split(':')[1];
              return (
                displayedPageDocument?.blocks.find(
                  (candidate) => candidate.id === blockId
                )?.id ?? null
              );
            }
          })
        : undefined,
    [
      confirmRuntimeWrite,
      csrfToken,
      displayedPageDocument,
      selectedPageId,
      tabId,
      workspaceId
    ]
  );
  const nativeContextHost = useMemo<
    FrontstageNativeBlockContextHost | undefined
  >(
    () =>
      jsBlockCapabilityHandlers && selectedPageId && tabId
        ? {
            interface: jsBlockCapabilityHandlers.interface,
            emitEvent: (event) =>
              openChildContainer(
                event.requestId.split(':')[1] ?? '',
                event.name,
                event.payload
              ),
            observeApiCall: (observation) =>
              recordFrontstageRuntimeObservation({
                actorId: actor?.id ?? 'anonymous',
                workspaceId,
                pageId: selectedPageId,
                tabId,
                blockId: observation.requestId.split(':')[1] ?? '',
                runtimeKind: 'native',
                stage: 'api_wait',
                instanceEpoch: observation.instanceEpoch,
                callId: observation.callId,
                apiCallStatus: observation.status,
                method: observation.method,
                path: observation.path,
                durationMs: observation.durationMs,
                error: observation.error
              })
          }
        : undefined,
    [
      actor?.id,
      jsBlockCapabilityHandlers,
      openChildContainer,
      selectedPageId,
      tabId,
      workspaceId
    ]
  );
  const activePageRenderPlan = useMemo(
    () =>
      displayedPageDocument
        ? createFrontstagePageRenderPlan(displayedPageDocument)
        : null,
    [displayedPageDocument]
  );
  const [runtimeDemandsByBlockId, setRuntimeDemandsByBlockId] =
    useState<FrontstageRuntimeDemandByBlockId>({});
  useEffect(() => {
    setRuntimeDemandsByBlockId({});
  }, [activePageContent?.page.id]);
  const handleRuntimeDemandChange = useCallback(
    (blockId: string, priority: FrontstageRuntimeDemandPriority) => {
      setRuntimeDemandsByBlockId((current) =>
        current[blockId] === priority
          ? current
          : { ...current, [blockId]: priority }
      );
    },
    []
  );
  const pageCanvasCodeReadPlan = useMemo(
    () =>
      activePageRenderPlan
        ? createFrontstagePageCanvasBlockCodeReadPlan({
            workspaceId,
            renderPlan: activePageRenderPlan
          })
        : null,
    [activePageRenderPlan, workspaceId]
  );
  const nativeDependencyLocksByBlockId = useMemo(
    () =>
      Object.fromEntries(
        (displayedPageDocument?.blocks ?? []).map((block) => {
          const catalogEntry = findMatchingFrontstageBlockCatalogEntry(
            block,
            blockCatalog.items
          );
          return [
            block.id,
            resolveFrontstageNativeDependencyLock({
              catalogEntry,
              workspaceId
            }).dependencyLock
          ];
        })
      ),
    [blockCatalog.items, displayedPageDocument?.blocks, workspaceId]
  );
  const pageCanvasNativePreparations =
    useFrontstagePageCanvasNativePreparations({
      actorId: actor?.id,
      actorWorkspaceId: actor?.current_workspace_id,
      readPlan: pageCanvasCodeReadPlan,
      dependencyLocksByBlockId: nativeDependencyLocksByBlockId,
      demandsByBlockId: runtimeDemandsByBlockId
    });
  const nativeBlockRuntimeContext = useMemo<FrontstagePageCanvasRuntimeContext>(
    () => ({
      currentUser: actor
        ? {
            id: actor.id,
            displayName:
              me?.nickname?.trim() || me?.name?.trim() || actor.account
          }
        : null,
      workspace: { id: workspaceId },
      application: null,
      theme: { mode: 'light', tokens: {} },
      ui: { locale: me?.preferred_locale ?? undefined }
    }),
    [actor, me?.name, me?.nickname, me?.preferred_locale, workspaceId]
  );
  const createTrialBlockContext = useMemo(() => {
    if (!jsBlockCapabilityHandlers || !selectedPageId || !tabId) {
      return undefined;
    }
    return (input: JsxStudioRunBlockContextInput) => {
      const unavailable = createFrontstageUnavailableBlockContext(input.plan);
      const capabilities = createNativeBlockContextCapabilities({
        requestId: input.requestId,
        instanceEpoch: input.instanceEpoch,
        isCurrentInstance: input.isCurrentInstance,
        interfaceHandler: jsBlockCapabilityHandlers.interface,
        outputs: unavailable.outputs,
        observeApiCall: input.observeApiCall
      });
      return {
        ...unavailable,
        ...nativeBlockRuntimeContext,
        page: {
          id: selectedPageId,
          route: activePageContent?.tab.routeSegment ?? selectedPageId,
          ...(activePageContent?.page.title
            ? { title: activePageContent.page.title }
            : {})
        },
        ...capabilities,
        props: { ...input.plan.props }
      };
    };
  }, [
    activePageContent?.page.title,
    activePageContent?.tab.routeSegment,
    jsBlockCapabilityHandlers,
    nativeBlockRuntimeContext,
    selectedPageId,
    tabId
  ]);
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
    ? i18nText('frontstage', 'auto.saving')
    : pageTreeMutationError
      ? toDisplayErrorMessage(pageTreeMutationError)
      : i18nText('frontstage', 'auto.operation_failed');

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
  const matchingJsBlockCatalogEntry = useMemo(
    () =>
      findMatchingFrontstageBlockCatalogEntry(
        selectedBlock,
        blockCatalog.items
      ),
    [blockCatalog.items, selectedBlock]
  );
  const nativeDependencyLockResolution = useMemo(
    () =>
      resolveFrontstageNativeDependencyLock({
        catalogEntry: matchingJsBlockCatalogEntry,
        workspaceId
      }),
    [matchingJsBlockCatalogEntry, workspaceId]
  );
  useEffect(() => {
    setSavedPageContent(null);
    setSelectedBlockId(null);
    setIsJsxStudioOpen(false);
    setBlockSaveError(null);
  }, [selectedPageId]);

  useEffect(() => {
    setSavedPageContent(null);
    setSelectedBlockId((currentBlockId) => {
      if (!currentBlockId || !pageContent) {
        setIsJsxStudioOpen(false);
        return null;
      }

      const document = createFrontstagePageDocument(pageContent);
      const hasCurrentBlock = document.blocks.some(
        (block) => block.id === currentBlockId
      );
      if (!hasCurrentBlock) {
        setIsJsxStudioOpen(false);
      }

      return hasCurrentBlock ? currentBlockId : null;
    });
  }, [pageContent]);

  useEffect(() => {
    if (!canShowSelectedBlockActions) {
      setIsJsxStudioOpen(false);
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
      setIsJsxStudioOpen(false);
    }
  }, [canEnterDesignMode, isDesignMode]);

  const selectedPageDisplayTitle = getPageDisplayTitle(
    pageTree,
    selectedPageId
  );
  const selectedPageLabel = selectedPageDisplayTitle
    ? selectedPageDisplayTitle
    : selectedPageId
      ? i18nText('frontstage', 'auto.page_with_id', { value1: selectedPageId })
      : null;
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
        return true;
      } catch (error) {
        setBlockSaveError(toDisplayErrorMessage(error));
        return false;
      } finally {
        setIsBlockSavePending(false);
      }
    },
    [pageContentSave]
  );

  const saveStudioBlock = useCallback(
    async (nextBlock: FrontstageBlockInstance) => {
      if (!blockCompositionState || !activePageContent) {
        return false;
      }

      const nextCompositionState = updateFrontstageBlock(
        blockCompositionState,
        nextBlock
      );
      return saveBlockComposition(activePageContent, nextCompositionState);
    },
    [activePageContent, blockCompositionState, saveBlockComposition]
  );
  const saveStudioChildContainers = useCallback(
    async (nextChildContainers: ChildContainerNode[]) => {
      if (!blockCompositionState || !activePageContent) {
        return false;
      }
      return saveBlockComposition(activePageContent, {
        ...blockCompositionState,
        document: {
          ...blockCompositionState.document,
          childContainers: nextChildContainers.map((container) => ({
            ...container,
            blockIds: [...container.blockIds]
          }))
        }
      });
    },
    [activePageContent, blockCompositionState, saveBlockComposition]
  );

  const designActions = useMemo(() => {
    if (!canEnterDesignMode || !isDesignMode) {
      return undefined;
    }

    return {
      onEditCode: (blockId: string) => {
        setSelectedBlockId(blockId);
        setIsJsxStudioOpen(true);
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
    blockCompositionState,
    activePageContent,
    saveBlockComposition,
    setSelectedBlockId,
    setIsJsxStudioOpen
  ]);

  if (initialPageTree === undefined && isPageTreeLoading) {
    return (
      <SectionPageLayout
        pageTitle={i18nText('frontstage', 'auto.frontstage')}
        navItems={[]}
        activeKey=""
        contentWidth="wide"
        heightMode="viewport"
        sidebarContent={
          <Typography.Text type="secondary" style={{ paddingInline: 16 }}>
            {i18nText('frontstage', 'auto.page_tree_loading')}
          </Typography.Text>
        }
      >
        <section className="frontstage-page-workspace">
          <header className="frontstage-page-workspace__header">
            <Typography.Title
              className="frontstage-page-workspace__title"
              level={4}
            >
              {i18nText('frontstage', 'auto.page_tree_loading_ellipsis')}
            </Typography.Title>
          </header>
          <Divider style={{ margin: 0 }} />
          <div className="frontstage-page-workspace__body">
            <Empty
              description={
                <Typography.Text>
                  {i18nText('frontstage', 'auto.page_tree_loading_wait')}
                </Typography.Text>
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
        pageTitle={i18nText('frontstage', 'auto.frontstage')}
        navItems={[]}
        activeKey=""
        contentWidth="wide"
        heightMode="viewport"
        sidebarContent={
          <Typography.Text type="secondary" style={{ paddingInline: 16 }}>
            {i18nText('frontstage', 'auto.page_tree_unavailable')}
          </Typography.Text>
        }
      >
        <section className="frontstage-page-workspace">
          <header className="frontstage-page-workspace__header">
            <Typography.Title
              className="frontstage-page-workspace__title"
              level={4}
            >
              {i18nText('frontstage', 'auto.page_tree_load_failed')}
            </Typography.Title>
          </header>
          <Divider style={{ margin: 0 }} />
          <div className="frontstage-page-workspace__body">
            <Empty
              image={Empty.PRESENTED_IMAGE_SIMPLE}
              description={
                <Typography.Text>
                  {i18nText('frontstage', 'auto.page_tree_load_failed_retry')}
                </Typography.Text>
              }
            >
              <Button type="primary" onClick={onRetryLoadPageTree}>
                {i18nText('frontstage', 'auto.retry')}
              </Button>
            </Empty>
          </div>
        </section>
      </SectionPageLayout>
    );
  }

  const renderPageTreeErrorBanner = hasPageTreeLoadError ? (
    <Alert
      style={{ marginBottom: 12 }}
      title={i18nText('frontstage', 'auto.page_tree_load_failed')}
      description={i18nText('frontstage', 'auto.page_tree_load_failed_recover')}
      type="error"
      showIcon
      action={
        onRetryLoadPageTree ? (
          <Button size="small" onClick={() => onRetryLoadPageTree()}>
            {i18nText('frontstage', 'auto.retry')}
          </Button>
        ) : null
      }
    />
  ) : null;

  const handlePageLayoutModeChange = (layoutMode: FrontstagePageLayoutMode) => {
    if (
      !blockCompositionState ||
      !activePageContent ||
      blockCompositionState.document.layoutMode === layoutMode
    ) {
      return;
    }

    let next = updateFrontstagePageLayoutMode(
      blockCompositionState,
      layoutMode
    );
    if (layoutMode === 'auto') {
      const responsiveLayouts = createFrontstageResponsiveLayouts(
        next.document.blocks.map((block) => ({
          blockId: block.id,
          layout: block.layout,
          presentation: block.presentation
        }))
      );
      const normalizedLayouts = createFrontstagePersistedGridLayout(
        normalizeFrontstageAutomaticResponsiveLayouts(responsiveLayouts)
      );
      for (const [blockId, blockLayouts] of Object.entries(normalizedLayouts)) {
        next = updateFrontstageBlockLayout(next, blockId, blockLayouts);
      }
    }

    void saveBlockComposition(activePageContent, next);
  };

  const handleAddBlock = async () => {
    if (
      !canAddBlock ||
      blockCatalog.loading ||
      blockCreationPendingRef.current
    ) {
      return;
    }
    setBlockSaveError(null);
    pageContentSave.clearError();
    const sourceContent = activePageContent;
    if (!sourceContent || !blockCompositionState) {
      return;
    }

    if (blockCatalog.error) {
      setBlockSaveError(toDisplayErrorMessage(blockCatalog.error));
      return;
    }

    if (blockCatalog.items.length === 0) {
      setBlockSaveError(
        i18nText('frontstage', 'auto.no_available_block_catalog_entries')
      );
      return;
    }

    if (blockCatalog.items.length !== 1) {
      setBlockSaveError(
        i18nText('frontstage', 'auto.block_catalog_load_failed')
      );
      return;
    }

    const [entry] = blockCatalog.items;
    if (!entry) return;

    const codeTemplate = entry.codeCapabilities?.template;
    if (!codeTemplate) {
      setBlockSaveError(
        i18nText('frontstage', 'auto.catalog_entry_missing_code_template')
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

    blockCreationPendingRef.current = true;
    setIsBlockSavePending(true);
    try {
      const input = createFrontstagePageDocumentSaveInput(
        sourceContent,
        nextCompositionState.document
      );
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
      const nextContent = await pageContentSave.createBlock({
        payload: input.payload,
        code_ref: codeRef,
        code: codeTemplate.source
      });

      setSavedPageContent(nextContent);
      setSelectedBlockId(nextCompositionState.selectedBlockId);
    } catch (error) {
      setBlockSaveError(toDisplayErrorMessage(error));
    } finally {
      blockCreationPendingRef.current = false;
      setIsBlockSavePending(false);
    }
  };

  const canEditPageTree = canEnterDesignMode && isDesignMode;
  const handleCanvasResponsiveLayoutSave = (
    layouts: FrontstagePersistedGridLayout,
    presentationPatch?: {
      blockId: string;
      presentation: FrontstageBlockPresentation;
    }
  ) => {
    if (!blockCompositionState || !activePageContent) return;
    let next = blockCompositionState;
    for (const [blockId, blockLayouts] of Object.entries(layouts)) {
      next = updateFrontstageBlockLayout(next, blockId, blockLayouts);
    }
    if (presentationPatch) {
      next = updateFrontstageBlockPresentation(
        next,
        presentationPatch.blockId,
        presentationPatch.presentation
      );
    }
    void saveBlockComposition(activePageContent, next);
  };
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
  const renderChildContainerLayers = (index = 0): ReactNode => {
    const container = childContainerRuntime.renderPath[index];
    if (!container) return null;
    const closeContainer = () =>
      onContainerIdChange?.(
        resolveChildContainerCloseTarget(childContainers, container.id),
        'replace'
      );
    const content = (
      <>
        <PageCanvas
          content={activePageContent}
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
          runtimePreparations={pageCanvasNativePreparations.preparations}
          runtimeContext={nativeBlockRuntimeContext}
          nativeContextHost={nativeContextHost}
          renderBlockIds={container.blockIds}
          sharedSignalCoordinator={pageSignalCoordinator}
          onRuntimeDemandChange={handleRuntimeDemandChange}
          onRuntimeRetry={pageCanvasNativePreparations.retryBlock}
          isDesignMode={canEnterDesignMode && isDesignMode}
          designActions={designActions}
          toolbarDisabled={isPageContentSavePending}
          onResponsiveLayoutSave={
            canEnterDesignMode && isDesignMode
              ? handleCanvasResponsiveLayoutSave
              : undefined
          }
          showTitle={false}
        />
        {renderChildContainerLayers(index + 1)}
      </>
    );

    if (container.presentation === 'drawer') {
      return (
        <Drawer
          open
          title={container.title}
          width="min(720px, 92vw)"
          onClose={closeContainer}
        >
          {content}
        </Drawer>
      );
    }
    if (container.presentation === 'modal') {
      return (
        <Modal
          open
          destroyOnHidden
          footer={null}
          title={container.title}
          width={720}
          onCancel={closeContainer}
        >
          {content}
        </Modal>
      );
    }
    return (
      <section
        aria-label={container.title}
        style={{ border: '1px solid #f0f0f0', borderRadius: 8, padding: 12 }}
      >
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 8,
            marginBottom: 8
          }}
        >
          <Typography.Text strong>{container.title}</Typography.Text>
          <Button size="small" onClick={closeContainer}>
            {i18nText('frontstage', 'auto.close')}
          </Button>
        </div>
        {content}
      </section>
    );
  };
  const frontstageTabContent = (
    <>
      {canEnterDesignMode && isDesignMode && isPageContentSavePending ? (
        <Typography.Text
          type="secondary"
          style={{ marginBottom: 12, display: 'block' }}
        >
          {i18nText('frontstage', 'auto.block_saving')}
        </Typography.Text>
      ) : null}
      {canEnterDesignMode && isDesignMode && pageContentSaveError ? (
        <Alert
          style={{ marginBottom: 12 }}
          title={i18nText('frontstage', 'auto.block_save_failed')}
          description={pageContentSaveError}
          type="error"
          showIcon
        />
      ) : null}
      {childContainerRuntimeError ? (
        <Alert
          showIcon
          type="error"
          title={childContainerRuntimeError}
          style={{ marginBottom: 12 }}
        />
      ) : null}
      <PageCanvas
        content={
          selectedPageNode && hasLoadedSelectedPageContent
            ? displayedPageContent
            : undefined
        }
        isLoading={Boolean(selectedPageNode && isPageContentLoading)}
        hasError={Boolean(selectedPageNode && hasPageContentLoadError)}
        isPermissionDenied={Boolean(
          selectedPageNode && isPageContentPermissionDenied
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
        runtimePreparations={pageCanvasNativePreparations.preparations}
        runtimeContext={nativeBlockRuntimeContext}
        nativeContextHost={nativeContextHost}
        renderBlockIds={rootPageBlockIds}
        sharedSignalCoordinator={pageSignalCoordinator}
        onRuntimeDemandChange={handleRuntimeDemandChange}
        onRuntimeRetry={pageCanvasNativePreparations.retryBlock}
        isDesignMode={canEnterDesignMode && isDesignMode}
        designActions={designActions}
        toolbarDisabled={isPageContentSavePending}
        onResponsiveLayoutSave={
          canEnterDesignMode && isDesignMode
            ? handleCanvasResponsiveLayoutSave
            : undefined
        }
        showTitle={false}
      />
      {renderChildContainerLayers()}
      {canEnterDesignMode && isDesignMode && selectedPageNode ? (
        <Button
          size="middle"
          aria-label={i18nText('frontstage', 'auto.create_block')}
          onClick={() => void handleAddBlock()}
          disabled={!canAddBlock || blockCatalog.loading}
          loading={isBlockSavePending || blockCatalog.loading}
          style={{
            margin: '8px 16px 16px',
            borderStyle: 'dashed',
            borderColor: FRONTSTAGE_DESIGN_BLUE.dashed,
            color: FRONTSTAGE_DESIGN_BLUE.primary
          }}
        >
          {i18nText('frontstage', 'auto.add_block_button')}
        </Button>
      ) : null}
    </>
  );

  return (
    <SectionPageLayout
      navItems={[]}
      activeKey=""
      contentWidth="wide"
      heightMode="viewport"
      sidebarContent={showSidebar ? frontstageSidebar : undefined}
    >
      <>
        <section
          className={[
            'frontstage-page-workspace',
            canEditPageTree && selectedPageNode
              ? 'frontstage-page-workspace--design-selected'
              : null
          ]
            .filter(Boolean)
            .join(' ')}
          data-testid="frontstage-page-workspace"
          data-design-selected={
            canEditPageTree && selectedPageNode ? 'true' : 'false'
          }
          style={
            canEditPageTree && selectedPageNode
              ? ({
                  '--frontstage-design-page-border':
                    FRONTSTAGE_DESIGN_BLUE.borderSelected,
                  '--frontstage-design-page-halo': FRONTSTAGE_DESIGN_BLUE.halo
                } as CSSProperties)
              : undefined
          }
        >
          {selectedPageNode ? (
            <>
              <header className="frontstage-page-workspace__header">
                <Typography.Title
                  className="frontstage-page-workspace__title"
                  level={4}
                >
                  {selectedPageLabel}
                </Typography.Title>
                {canEditPageTree ? (
                  <div className="frontstage-page-workspace__page-action">
                    <PageWorkspaceActionMenu
                      disabled={isOperationPending || isPageContentSavePending}
                      tabsEnabled={
                        selectedPageNode.content_presentation === 'tabs'
                      }
                      layoutMode={
                        blockCompositionState?.document.layoutMode ?? 'auto'
                      }
                      onEdit={() => handleRenameNode(selectedPageNode)}
                      onTabsEnabledChange={handlePageTabsEnabledChange}
                      onLayoutModeChange={handlePageLayoutModeChange}
                    />
                  </div>
                ) : null}
              </header>
              <Divider style={{ margin: 0 }} />
            </>
          ) : null}
          <div className="frontstage-page-workspace__body">
            {renderPageTreeErrorBanner}
            {canEnterDesignMode &&
            isDesignMode &&
            (isOperationPending || hasOperationError) ? (
              <Typography.Text
                type={hasOperationError ? 'danger' : 'secondary'}
                style={{ marginBottom: 12, display: 'block' }}
              >
                {operationStatusText}
              </Typography.Text>
            ) : null}
            {selectedPageId && tabId && onNavigateTab && activePageContent ? (
              <FrontstagePageTabs
                workspaceId={workspaceId}
                pageId={selectedPageId}
                tabId={tabId}
                presentation={activePageContent.page.contentPresentation}
                isDesignMode={canEnterDesignMode && isDesignMode}
                onNavigateTab={onNavigateTab}
              >
                {frontstageTabContent}
              </FrontstagePageTabs>
            ) : (
              frontstageTabContent
            )}
          </div>
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
        {selectedBlock && selectedPageId ? (
          <FrontstageJsxStudioDrawer
            open={isJsxStudioOpen && canShowSelectedBlockActions}
            initialSection="code"
            workspaceId={workspaceId}
            pageId={selectedPageId}
            tabId={tabId}
            block={selectedBlock}
            pageBlocks={displayedPageDocument?.blocks}
            childContainers={childContainers}
            catalogEntry={matchingJsBlockCatalogEntry}
            onClose={() => setIsJsxStudioOpen(false)}
            onSaveBlock={saveStudioBlock}
            onSaveChildContainers={saveStudioChildContainers}
            runPanel={({ code, runRevision }) =>
              runRevision === null ? undefined : (
                <JsxStudioRunPanel
                  block={selectedBlock}
                  code={code}
                  createBlockContext={createTrialBlockContext}
                  onPrepareDraftRun={jsBlockCapabilityHandlers?.prepareDraftRun}
                  onRevokeDraftRun={jsBlockCapabilityHandlers?.revokeDraftRun}
                  nativeDependencyLock={
                    nativeDependencyLockResolution.dependencyLock
                  }
                  nativeDependencyLockError={
                    nativeDependencyLockResolution.error
                  }
                  revision={`run:${runRevision}`}
                />
              )
            }
          />
        ) : null}
      </>
    </SectionPageLayout>
  );
};
