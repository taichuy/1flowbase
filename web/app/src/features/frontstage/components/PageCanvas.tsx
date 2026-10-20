import { type FrontstagePageCanvasRuntimeContext } from './canvas/RenderPlanSlot';
import {
  type DesignBlockActions,
  RenderPlanSlot
} from './canvas/RenderPlanSlot';

import { useFrontstagePageCanvasWidth } from '../lib/page-canvas/geometry/use-canvas-width';
import { Alert, Button, Empty, Space, Typography } from 'antd';

import type { IsolatedFrontendBlockCapabilityHandlers } from '@1flowbase/page-runtime';
import type { FC, Ref } from 'react';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState
} from 'react';
import {
  ResponsiveGridLayout,
  type Layout,
  type ResizeHandleAxis
} from 'react-grid-layout/react';
import 'react-grid-layout/css/styles.css';
import './page-canvas.css';

import type { FrontstagePageContent } from '../api/page-content';

import { createFrontstagePageDocument } from '../lib/page-document';
import {
  createFrontstageBlockRenderPlanItems,
  createFrontstagePageRenderPlan
} from '../lib/page-canvas/render-plan';
import { i18nText } from '../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';

import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  FRONTSTAGE_GRID_BREAKPOINTS,
  FRONTSTAGE_GRID_COLUMNS,
  FRONTSTAGE_GRID_ROW_HEIGHT,
  FRONTSTAGE_GRID_VERTICAL_MARGIN,
  frontstageGridRowsToPixels,
  normalizeFrontstageAutomaticResponsiveLayouts,
  replaceFrontstageBreakpointLayout,
  type FrontstageGridBreakpoint,
  type FrontstagePersistedGridLayout
} from '../lib/responsive-grid-layout';
import type {
  FrontstageBlockInstance,
  FrontstageBlockPresentation
} from '../lib/page-document';
import {
  createFrontstageInteractionCompactor,
  frontstageLayoutsEqualForCommit
} from '../lib/page-canvas/frontstage-block-interaction';
import type { FrontstageRuntimeDemandPriority } from '../lib/page-canvas/runtime-demand';

import {
  FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES,
  FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
  FrontstageAutoHeightBatch,
  resolveFrontstageAutoHeightScrollDelta
} from '../lib/page-canvas/auto-height-layout';
import type { FrontstageIntrinsicMeasurementContext } from '../lib/page-canvas/auto-height-layout';
import type { FrontstageNativePreparationSource } from '../lib/page-canvas/native-runtime-preparation';
import { applyFrontstageLayoutCompensation } from '../lib/native-modules/surface/scroll-intent-arbiter';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator
} from '../lib/page-canvas/signal-runtime';
import type { FrontstageNativeBlockContextHost } from '../lib/page-canvas/native-block-context-host';

import type { PreparedFrontstageIsolatedContribution } from '../lib/isolated-frontend-block-contribution';
import { createFrontstageAssistantDomRuntime } from '../lib/assistant-frontstage-runtime-dom';
import { registerFrontstageAssistantRuntime } from '../lib/assistant-frontstage-runtime';

type PageCanvasProps = {
  content?: FrontstagePageContent;
  isLoading?: boolean;
  hasError?: boolean;
  isPermissionDenied?: boolean;
  selectedBlockId?: string | null;
  onSelectBlock?: (blockId: string | null) => void;
  onRetry?: () => void;
  runtimePreparations?: FrontstageNativePreparationSource | null;
  isolatedRuntimePreparations?:
    | readonly PreparedFrontstageIsolatedContribution[]
    | null;
  isolatedRuntimePreparationErrorsByBlockId?: Readonly<Record<string, Error>>;
  isolatedCapabilityHandlersByBlockId?: Readonly<
    Record<string, IsolatedFrontendBlockCapabilityHandlers>
  >;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
  nativeContextHost?: FrontstageNativeBlockContextHost;
  renderBlockIds?: readonly string[];
  runtimeBlocks?: readonly FrontstageBlockInstance[];
  runtimeInputsByBlockId?: Readonly<
    Record<string, Readonly<Record<string, unknown>>>
  >;
  sharedSignalCoordinator?: FrontstageSignalRuntimeCoordinator;
  /** When true, blocks show blue outlines + hover toolbar */
  isDesignMode?: boolean;
  /** Actions triggered from the design mode hover toolbar */
  designActions?: DesignBlockActions;
  /** When true, all hover toolbar buttons are disabled (e.g. during save) */
  toolbarDisabled?: boolean;
  showTitle?: boolean;
  onResponsiveLayoutSave?: (
    layouts: FrontstagePersistedGridLayout,
    presentationPatch?: {
      blockId: string;
      presentation: FrontstageBlockPresentation;
    }
  ) => void;
  onRuntimeDemandChange?: (
    blockId: string,
    priority: FrontstageRuntimeDemandPriority
  ) => void;
  onRuntimeInteraction?: () => void;
  onRuntimeRetry?: (blockId: string) => void;
  onRuntimeRefresh?: (blockId: string) => void;
};

function renderFrontstageResizeHandle(
  axis: ResizeHandleAxis,
  ref: Ref<HTMLElement>
) {
  return (
    <span
      ref={ref as Ref<HTMLSpanElement>}
      className={`react-resizable-handle react-resizable-handle-${axis} frontstage-grid-resize-handle frontstage-grid-resize-handle--${axis}`}
      data-testid={`frontstage-grid-resize-handle-${axis}`}
      aria-hidden="true"
    />
  );
}

function formatPageTitle(content: FrontstagePageContent): string {
  return (
    content.page.title?.trim() || i18nText('frontstage', 'auto.unnamed_page')
  );
}

// ─── PageCanvas ─────────────────────────────────────────────────────

type FrontstageScrollAnchor = {
  scrollOwner: HTMLElement;
  blockId: string;
  layoutDeltaPx: number;
  scrollTop: number;
};

function captureFrontstageScrollAnchor(
  canvasNode: HTMLElement | null
): FrontstageScrollAnchor | null {
  const scrollOwner = canvasNode?.closest<HTMLElement>(
    '[data-flowbase-frontstage-scroll-owner]'
  );
  if (!canvasNode || !scrollOwner) return null;
  const ownerRect = scrollOwner.getBoundingClientRect();
  const visibleBlocks = Array.from(
    canvasNode.querySelectorAll<HTMLElement>(
      '[data-flowbase-frontstage-block-id]'
    )
  )
    .map((blockElement) => {
      const element =
        blockElement.closest<HTMLElement>('.react-grid-item') ?? blockElement;
      return {
        blockId: blockElement.dataset.flowbaseFrontstageBlockId ?? '',
        rect: element.getBoundingClientRect()
      };
    })
    .filter(
      ({ rect }) => rect.bottom > ownerRect.top && rect.top < ownerRect.bottom
    )
    .sort((left, right) => left.rect.top - right.rect.top);
  const anchor = visibleBlocks[0];
  return anchor
    ? {
        scrollOwner,
        blockId: anchor.blockId,
        layoutDeltaPx: 0,
        scrollTop: scrollOwner.scrollTop
      }
    : null;
}

export const PageCanvas: FC<PageCanvasProps> = ({
  content,
  isLoading,
  hasError,
  isPermissionDenied,
  selectedBlockId = null,
  onSelectBlock,
  onRetry,
  runtimePreparations,
  isolatedRuntimePreparations,
  isolatedRuntimePreparationErrorsByBlockId,
  isolatedCapabilityHandlersByBlockId,
  runtimeContext,
  nativeContextHost,
  renderBlockIds,
  runtimeBlocks,
  runtimeInputsByBlockId,
  sharedSignalCoordinator,
  isDesignMode = false,
  designActions,
  toolbarDisabled = false,
  showTitle = true,
  onResponsiveLayoutSave,
  onRuntimeDemandChange,
  onRuntimeInteraction,
  onRuntimeRetry,
  onRuntimeRefresh
}) => {
  useEffect(() => {
    if (!onRuntimeRetry) return;
    return registerFrontstageAssistantRuntime(
      createFrontstageAssistantDomRuntime({ recompile: onRuntimeRetry })
    );
  }, [onRuntimeRetry]);
  const pageDocument = useMemo(
    () => (content ? createFrontstagePageDocument(content) : null),
    [content]
  );
  const document = useMemo(() => {
    if (!pageDocument || renderBlockIds === undefined) return pageDocument;
    const visibleBlockIds = new Set(renderBlockIds);
    const blocks = pageDocument.blocks.filter((block) =>
      visibleBlockIds.has(block.id)
    );
    return { ...pageDocument, blocks, isEmpty: blocks.length === 0 };
  }, [pageDocument, renderBlockIds]);
  const canonicalRenderItems = useMemo(() => {
    if (!runtimeBlocks) return null;
    const visibleBlockIds =
      renderBlockIds === undefined ? null : new Set(renderBlockIds);
    return createFrontstageBlockRenderPlanItems(
      visibleBlockIds
        ? runtimeBlocks.filter((block) => visibleBlockIds.has(block.id))
        : runtimeBlocks
    );
  }, [renderBlockIds, runtimeBlocks]);
  const renderPlan = useMemo(
    () => (document ? createFrontstagePageRenderPlan(document) : null),
    [document]
  );
  const renderItems = useMemo(
    () => canonicalRenderItems ?? renderPlan?.items ?? [],
    [canonicalRenderItems, renderPlan]
  );
  const isRenderEmpty = renderItems.length === 0;
  const signalSession = useMemo(
    () => createFrontstagePageSignalSession(),
    // The page id is the lifecycle key even though session construction is argument-free.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [content?.page.id]
  );
  const localSignalCoordinator = useMemo(
    () =>
      !sharedSignalCoordinator && pageDocument && content
        ? new FrontstageSignalRuntimeCoordinator(
            runtimeBlocks ?? pageDocument.blocks,
            content.tab.id,
            signalSession
          )
        : null,
    // Runtime block arrays reconcile below; depending on their identity would dispose live epochs.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      content?.tab.id,
      pageDocument?.page.id,
      sharedSignalCoordinator,
      signalSession
    ]
  );
  const signalCoordinator = sharedSignalCoordinator ?? localSignalCoordinator;
  useEffect(() => {
    if (!sharedSignalCoordinator) {
      localSignalCoordinator?.updateBlocks(
        runtimeBlocks ?? pageDocument?.blocks ?? []
      );
    }
  }, [
    localSignalCoordinator,
    pageDocument?.blocks,
    runtimeBlocks,
    sharedSignalCoordinator
  ]);
  useEffect(
    () => () => localSignalCoordinator?.dispose(),
    [localSignalCoordinator]
  );
  const {
    width: gridWidth,
    containerNode: gridContainerNode,
    containerRef
  } = useFrontstagePageCanvasWidth();
  const interactionCompactor = useMemo(
    () => createFrontstageInteractionCompactor(document?.layoutMode ?? 'auto'),
    [document?.layoutMode]
  );
  const [autoRows, setAutoRows] = useState<Record<string, number>>({});
  const autoRowsRef = useRef(autoRows);
  const [autoAllocatedHeightEpochs, setAutoAllocatedHeightEpochs] = useState<
    Record<string, number>
  >({});
  const autoAllocatedHeightEpochsRef = useRef(autoAllocatedHeightEpochs);
  const autoHeightScopeRef = useRef(content?.page.id);
  const autoHeightBatchRef = useRef<FrontstageAutoHeightBatch | null>(null);
  if (autoHeightBatchRef.current === null) {
    autoHeightBatchRef.current = new FrontstageAutoHeightBatch({
      settleMs: FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
      settleFrames: FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES
    });
  }
  const autoHeightFrameRef = useRef<number | null>(null);
  const pendingScrollAnchorRef = useRef<FrontstageScrollAnchor | null>(null);
  const layouts = useMemo(() => {
    const responsiveLayouts = createFrontstageResponsiveLayouts(
      renderItems,
      autoRows
    );
    return document?.layoutMode === 'free'
      ? responsiveLayouts
      : normalizeFrontstageAutomaticResponsiveLayouts(responsiveLayouts);
  }, [autoRows, document?.layoutMode, renderItems]);
  useEffect(() => {
    autoRowsRef.current = autoRows;
  }, [autoRows]);
  useEffect(() => {
    autoAllocatedHeightEpochsRef.current = autoAllocatedHeightEpochs;
  }, [autoAllocatedHeightEpochs]);
  useEffect(() => {
    if (autoHeightScopeRef.current === content?.page.id) return;
    autoHeightScopeRef.current = content?.page.id;
    autoRowsRef.current = {};
    autoAllocatedHeightEpochsRef.current = {};
    autoHeightBatchRef.current = new FrontstageAutoHeightBatch({
      settleMs: FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
      settleFrames: FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES
    });
    setAutoRows({});
    setAutoAllocatedHeightEpochs({});
  }, [content?.page.id]);
  useEffect(
    () => () => {
      if (autoHeightFrameRef.current !== null) {
        cancelAnimationFrame(autoHeightFrameRef.current);
      }
    },
    []
  );
  useLayoutEffect(() => {
    const anchor = pendingScrollAnchorRef.current;
    pendingScrollAnchorRef.current = null;
    if (!anchor) return;
    applyFrontstageLayoutCompensation(
      anchor.scrollOwner,
      anchor.scrollTop,
      anchor.layoutDeltaPx
    );
  }, [autoRows]);
  const latestLayouts = useRef(layouts);
  const dragCommittedLayout = useRef<Layout | null>(null);
  const activeBreakpoint = useRef<FrontstageGridBreakpoint>('lg');
  useEffect(() => {
    latestLayouts.current = layouts;
  }, [layouts]);
  useEffect(
    () => () => {
      interactionCompactor.end();
      dragCommittedLayout.current = null;
    },
    [interactionCompactor]
  );

  const saveCurrentResponsiveLayout = (
    currentLayout: Layout,
    presentationPatch?: {
      blockId: string;
      presentation: FrontstageBlockPresentation;
    }
  ) => {
    const nextLayouts = replaceFrontstageBreakpointLayout(
      latestLayouts.current,
      activeBreakpoint.current,
      currentLayout
    );
    latestLayouts.current = nextLayouts;
    onResponsiveLayoutSave?.(
      createFrontstagePersistedGridLayout(nextLayouts),
      presentationPatch
    );
  };

  const createCurrentRowRequirements = () =>
    Object.fromEntries(
      (
        createFrontstageResponsiveLayouts(renderItems, autoRowsRef.current)[
          activeBreakpoint.current
        ] ?? []
      ).map((item) => [item.i, item.h])
    );

  const updateAutoHeight = useCallback(
    (
      blockId: string,
      height: number,
      context: FrontstageIntrinsicMeasurementContext
    ) => {
      autoHeightBatchRef.current!.measure(
        blockId,
        height,
        context,
        performance.now()
      );
      if (autoHeightFrameRef.current !== null) return;
      const flush = (nowMs: number) => {
        autoHeightFrameRef.current = null;
        const currentRows = autoRowsRef.current;
        const nextRows = autoHeightBatchRef.current!.commit(currentRows, nowMs);
        const committedMeasurements =
          autoHeightBatchRef.current!.takeCommittedMeasurements();
        if (committedMeasurements.length > 0) {
          const currentEpochs = autoAllocatedHeightEpochsRef.current;
          let nextEpochs: Record<string, number> | null = null;
          for (const measurement of committedMeasurements) {
            const nextEpoch =
              measurement.source === 'runtime-explicit'
                ? measurement.epoch
                : undefined;
            if (currentEpochs[measurement.blockId] === nextEpoch) {
              continue;
            }
            nextEpochs ??= { ...currentEpochs };
            if (nextEpoch === undefined) {
              delete nextEpochs[measurement.blockId];
            } else {
              nextEpochs[measurement.blockId] = nextEpoch;
            }
          }
          if (nextEpochs) {
            autoAllocatedHeightEpochsRef.current = nextEpochs;
            setAutoAllocatedHeightEpochs(nextEpochs);
          }
        }
        if (nextRows !== currentRows) {
          const anchor = captureFrontstageScrollAnchor(gridContainerNode);
          if (anchor) {
            const currentResponsiveLayouts = createFrontstageResponsiveLayouts(
              renderItems,
              currentRows
            );
            const nextResponsiveLayouts = createFrontstageResponsiveLayouts(
              renderItems,
              nextRows
            );
            const currentLayouts =
              document?.layoutMode === 'free'
                ? currentResponsiveLayouts
                : normalizeFrontstageAutomaticResponsiveLayouts(
                    currentResponsiveLayouts
                  );
            const nextLayouts =
              document?.layoutMode === 'free'
                ? nextResponsiveLayouts
                : normalizeFrontstageAutomaticResponsiveLayouts(
                    nextResponsiveLayouts
                  );
            const breakpoint = activeBreakpoint.current;
            const columns = FRONTSTAGE_GRID_COLUMNS[breakpoint];
            anchor.layoutDeltaPx = resolveFrontstageAutoHeightScrollDelta({
              anchorBlockId: anchor.blockId,
              columns,
              compact: document?.layoutMode !== 'free',
              currentLayout: currentLayouts[breakpoint] ?? [],
              nextLayout: nextLayouts[breakpoint] ?? [],
              rowHeight: FRONTSTAGE_GRID_ROW_HEIGHT,
              rowMargin: FRONTSTAGE_GRID_VERTICAL_MARGIN
            });
          }
          pendingScrollAnchorRef.current = anchor;
          autoRowsRef.current = nextRows;
          setAutoRows(nextRows);
        }
        if (autoHeightBatchRef.current!.hasPendingMeasurements()) {
          autoHeightFrameRef.current = requestAnimationFrame(flush);
        }
      };
      autoHeightFrameRef.current = requestAnimationFrame(flush);
    },
    [document?.layoutMode, gridContainerNode, renderItems]
  );

  if (isLoading) {
    return <LoadingState compact />;
  }

  if (hasError) {
    if (isPermissionDenied) {
      return <PermissionDeniedState />;
    }

    return (
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'auto.page_content_load_failed')}
        description={i18nText('frontstage', 'auto.network_retry')}
        action={
          onRetry ? (
            <Button size="small" onClick={onRetry}>
              {i18nText('frontstage', 'auto.retry')}
            </Button>
          ) : null
        }
      />
    );
  }

  if (!content || !document || !renderPlan) {
    return <Empty description={false} />;
  }

  return (
    <Space orientation="vertical" size={12} style={{ width: '100%' }}>
      {showTitle ? (
        <Typography.Title level={4} style={{ margin: 0 }}>
          {formatPageTitle(content)}
        </Typography.Title>
      ) : null}

      <div
        ref={containerRef}
        className={`frontstage-page-canvas-grid${isDesignMode ? '' : ' frontstage-page-canvas-grid--view'}`}
        data-testid="page-canvas-render-slots"
        onPointerOverCapture={onRuntimeInteraction}
        onPointerDownCapture={onRuntimeInteraction}
        onFocusCapture={onRuntimeInteraction}
        onKeyDownCapture={onRuntimeInteraction}
        onPointerMoveCapture={(event) => {
          if (!isDesignMode || toolbarDisabled) return;
          const bounds = event.currentTarget.getBoundingClientRect();
          if (bounds.width <= 0) return;
          const columns = FRONTSTAGE_GRID_COLUMNS[activeBreakpoint.current];
          interactionCompactor.updateDragPointer({
            column: Math.max(
              0,
              Math.min(
                columns,
                ((event.clientX - bounds.left) / bounds.width) * columns
              )
            ),
            row: Math.max(
              0,
              (event.clientY - bounds.top) / FRONTSTAGE_GRID_ROW_HEIGHT
            )
          });
        }}
      >
        {gridWidth <= 0 ? null : isRenderEmpty && isDesignMode ? (
          <div data-testid="page-canvas-design-empty-state" />
        ) : isRenderEmpty ? (
          <div
            style={{
              background: '#fafafa',
              border: '1px solid #f0f0f0',
              borderRadius: 8,
              padding: 32,
              textAlign: 'center'
            }}
          >
            <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} description={false} />
          </div>
        ) : (
          <ResponsiveGridLayout
            width={gridWidth}
            breakpoints={FRONTSTAGE_GRID_BREAKPOINTS}
            cols={FRONTSTAGE_GRID_COLUMNS}
            layouts={layouts}
            rowHeight={FRONTSTAGE_GRID_ROW_HEIGHT}
            margin={[16, FRONTSTAGE_GRID_VERTICAL_MARGIN]}
            compactor={interactionCompactor}
            dragConfig={{
              enabled: isDesignMode && !toolbarDisabled,
              bounded: false,
              handle: '.frontstage-block-drag-handle',
              cancel:
                'button:not(.frontstage-block-drag-handle), input, textarea, select, a',
              threshold: 3
            }}
            resizeConfig={{
              enabled: isDesignMode && !toolbarDisabled,
              handles: ['e', 'w', 's', 'se', 'sw'],
              handleComponent: renderFrontstageResizeHandle
            }}
            onBreakpointChange={(breakpoint) => {
              activeBreakpoint.current = breakpoint as FrontstageGridBreakpoint;
            }}
            onLayoutChange={(_layout: Layout, nextLayouts) => {
              latestLayouts.current = nextLayouts;
            }}
            onDragStart={(currentLayout, _oldItem, draggedItem) => {
              if (draggedItem) {
                dragCommittedLayout.current = currentLayout.map((item) => ({
                  ...item
                }));
                interactionCompactor.begin(
                  currentLayout,
                  draggedItem.i,
                  'drag',
                  createCurrentRowRequirements()
                );
              }
            }}
            onDragStop={(currentLayout) => {
              const committedLayout = dragCommittedLayout.current;
              interactionCompactor.end();
              dragCommittedLayout.current = null;
              if (
                !committedLayout ||
                !frontstageLayoutsEqualForCommit(committedLayout, currentLayout)
              ) {
                saveCurrentResponsiveLayout(currentLayout);
              }
            }}
            onResizeStart={(currentLayout, _oldItem, resizedItem) => {
              if (resizedItem) {
                interactionCompactor.begin(
                  currentLayout,
                  resizedItem.i,
                  'resize',
                  createCurrentRowRequirements()
                );
              }
            }}
            onResizeStop={(currentLayout, _oldItem, resizedItem) => {
              interactionCompactor.end();
              if (!resizedItem) {
                saveCurrentResponsiveLayout(currentLayout);
                return;
              }
              const item = renderItems.find(
                (candidate) => candidate.blockId === resizedItem.i
              );
              saveCurrentResponsiveLayout(
                currentLayout,
                item?.presentation.heightMode === 'fixed'
                  ? {
                      blockId: resizedItem.i,
                      presentation: {
                        heightMode: 'fixed',
                        height: frontstageGridRowsToPixels(resizedItem.h)
                      }
                    }
                  : undefined
              );
            }}
          >
            {renderItems.map((item) => (
              <div key={item.blockId}>
                <RenderPlanSlot
                  item={item}
                  runtimePreparations={runtimePreparations}
                  isolatedPreparation={
                    isolatedRuntimePreparations?.find(
                      (preparation) =>
                        preparation.blockInstanceId === item.blockId
                    ) ?? null
                  }
                  isolatedPreparationError={
                    isolatedRuntimePreparationErrorsByBlockId?.[item.blockId]
                  }
                  isolatedCapabilityHandlers={
                    isolatedCapabilityHandlersByBlockId?.[item.blockId]
                  }
                  signalCoordinator={signalCoordinator}
                  runtimeInputValues={runtimeInputsByBlockId?.[item.blockId]}
                  runtimeContext={runtimeContext}
                  nativeContextHost={nativeContextHost}
                  pageContent={content}
                  isSelected={item.blockId === selectedBlockId}
                  onSelectBlock={onSelectBlock}
                  isDesignMode={isDesignMode}
                  designActions={designActions}
                  toolbarDisabled={toolbarDisabled}
                  onAutoHeightChange={updateAutoHeight}
                  intrinsicHeight={
                    autoRows[item.blockId] === undefined
                      ? undefined
                      : frontstageGridRowsToPixels(autoRows[item.blockId])
                  }
                  allocatedHeightEpoch={autoAllocatedHeightEpochs[item.blockId]}
                  onRuntimeDemandChange={onRuntimeDemandChange}
                  onRuntimeRetry={onRuntimeRetry}
                  onRuntimeRefresh={onRuntimeRefresh}
                />
              </div>
            ))}
          </ResponsiveGridLayout>
        )}
      </div>
    </Space>
  );
};

export type { FrontstagePageCanvasRuntimeContext } from './canvas/RenderPlanSlot';
