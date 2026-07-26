import { Alert, Button, Empty, Space, Typography } from 'antd';
import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import type { BlockContext } from '@1flowbase/page-protocol';
import {
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { CSSProperties, FC, Ref } from 'react';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ResponsiveGridLayout,
  useContainerWidth,
  type Layout,
  type ResizeHandleAxis
} from 'react-grid-layout/react';
import 'react-grid-layout/css/styles.css';
import './page-canvas.css';

import type { FrontstagePageContent } from '../api/page-content';
import { BlockHoverToolbar } from './BlockHoverToolbar';
import { RestrictedBlockRuntimePreview } from './RestrictedBlockRuntimePreview';
import type { FrontstagePageCanvasRuntimeSessionEntry } from '../hooks/use-frontstage-page-canvas-runtime-sessions';
import { createFrontstagePageDocument } from '../lib/page-document';
import {
  createFrontstagePageRenderPlan,
  type FrontstageBlockRenderPlanItem
} from '../lib/page-canvas/render-plan';
import type { FrontstagePageCanvasRuntimeSourceState } from '../lib/page-canvas/runtime-source';
import type { FrontstagePageCanvasRuntimeRunPlanState } from '../lib/page-canvas/runtime-run-plan';
import { i18nText } from '../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';
import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  FRONTSTAGE_GRID_BREAKPOINTS,
  FRONTSTAGE_GRID_COLUMNS,
  FRONTSTAGE_GRID_ROW_HEIGHT,
  FRONTSTAGE_GRID_ROW_GAP,
  FRONTSTAGE_GRID_VERTICAL_MARGIN,
  frontstageGridRowsToPixels,
  normalizeFrontstageAutomaticResponsiveLayouts,
  replaceFrontstageBreakpointLayout,
  type FrontstageGridBreakpoint,
  type FrontstagePersistedGridLayout
} from '../lib/responsive-grid-layout';
import type { FrontstageBlockPresentation } from '../lib/page-document';
import {
  createFrontstageInteractionCompactor,
  frontstageLayoutsEqualForCommit
} from '../lib/page-canvas/frontstage-block-interaction';
import type { FrontstageRuntimeDemandPriority } from '../lib/page-canvas/runtime-demand';
import type { FrontstageNativePreparationSnapshot } from '../lib/page-canvas/native-runtime-preparation';
import { useFrontstageNativeBlockInstance } from '../hooks/use-frontstage-native-block-instance';
import { createFrontstageUnavailableBlockContext } from '../lib/native-trusted-block-react-adapter';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator
} from '../lib/page-canvas/signal-runtime';

export type FrontstagePageCanvasRuntimeContext = Pick<
  BlockContext,
  'currentUser' | 'workspace' | 'application' | 'theme' | 'ui'
>;

type DesignBlockActions = {
  onEditCode: (blockId: string) => void;
  onDelete: (blockId: string) => void;
};

type PageCanvasProps = {
  content?: FrontstagePageContent;
  isLoading?: boolean;
  hasError?: boolean;
  isPermissionDenied?: boolean;
  selectedBlockId?: string | null;
  onSelectBlock?: (blockId: string | null) => void;
  onRetry?: () => void;
  runtimeSourceState?: FrontstagePageCanvasRuntimeSourceState | null;
  runtimeRunPlanState?: FrontstagePageCanvasRuntimeRunPlanState | null;
  runtimeSessionEntries?:
    | readonly FrontstagePageCanvasRuntimeSessionEntry[]
    | null;
  runtimePreparations?: readonly FrontstageNativePreparationSnapshot[] | null;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
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
  onRuntimeRetry?: (blockId: string) => void;
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

function findRuntimeSessionEntryForSlot({
  item,
  slotIndex,
  runtimeSessionEntries
}: {
  item: { blockId: string; codeRef: string; sourceIndex: number };
  slotIndex: number;
  runtimeSessionEntries?:
    | readonly FrontstagePageCanvasRuntimeSessionEntry[]
    | null;
}): FrontstagePageCanvasRuntimeSessionEntry | null {
  if (!runtimeSessionEntries || runtimeSessionEntries.length === 0) {
    return null;
  }

  return (
    runtimeSessionEntries.find(
      (entry) =>
        entry.slotIndex === slotIndex &&
        entry.blockId === item.blockId &&
        entry.codeRef === item.codeRef
    ) ??
    runtimeSessionEntries.find((entry) => entry.slotIndex === slotIndex) ??
    null
  );
}

// ─── RenderPlanSlot ─────────────────────────────────────────────────

type RenderPlanSlotProps = {
  item: FrontstageBlockRenderPlanItem;
  runtimePreparation?: FrontstageNativePreparationSnapshot | null;
  signalCoordinator?: FrontstageSignalRuntimeCoordinator | null;
  signalRevision: number;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
  pageContent?: FrontstagePageContent;
  onSignalRevision(revision: number): void;
  runtimeSessionEntry?: FrontstagePageCanvasRuntimeSessionEntry | null;
  isSelected: boolean;
  onSelectBlock?: (blockId: string | null) => void;
  isDesignMode?: boolean;
  designActions?: DesignBlockActions;
  toolbarDisabled?: boolean;
  onAutoHeightChange?: (blockId: string, height: number) => void;
  onRuntimeDemandChange?: (
    blockId: string,
    priority: FrontstageRuntimeDemandPriority
  ) => void;
  onRuntimeRetry?: (blockId: string) => void;
};

const blockFrameBaseStyle: CSSProperties = {
  width: '100%',
  height: '100%',
  minWidth: 0,
  boxSizing: 'border-box',
  borderRadius: 8,
  background: '#fff',
  overflow: 'hidden',
  boxShadow: '0 14px 40px rgba(37, 99, 235, 0.05)'
};

const blockLabelStyle: CSSProperties = {
  position: 'absolute',
  top: 12,
  left: 14,
  zIndex: 2,
  borderRadius: 6,
  background: FRONTSTAGE_DESIGN_BLUE.labelBg,
  color: FRONTSTAGE_DESIGN_BLUE.labelText,
  fontSize: 12,
  lineHeight: '20px',
  padding: '0 8px'
};

function resolveRendererVersionError(
  item: FrontstageBlockRenderPlanItem
): { message: string; description: string } | null {
  const reason = item.fallbackReasons.find(
    (fallbackReason) =>
      fallbackReason.code === 'missing_renderer_version' ||
      fallbackReason.code === 'unsupported_renderer_version'
  );

  if (!reason) {
    return null;
  }

  if (reason.code === 'missing_renderer_version') {
    return {
      message: i18nText('frontstage', 'auto.block_renderer_version_missing'),
      description: i18nText(
        'frontstage',
        'auto.block_renderer_version_missing_description'
      )
    };
  }

  return {
    message: i18nText('frontstage', 'auto.block_renderer_version_unsupported'),
    description: i18nText(
      'frontstage',
      'auto.block_renderer_version_unsupported_description',
      { value1: item.rendererVersion ?? '' }
    )
  };
}

function NativeRuntimeSlotSurface({
  item,
  preparation,
  signalCoordinator,
  signalRevision,
  runtimeContext,
  pageContent,
  onSignalRevision,
  contentViewportStyle,
  onRetry
}: {
  item: FrontstageBlockRenderPlanItem;
  preparation: FrontstageNativePreparationSnapshot;
  contentViewportStyle: CSSProperties;
  onRetry?: () => void;
  signalCoordinator?: FrontstageSignalRuntimeCoordinator | null;
  signalRevision: number;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
  pageContent?: FrontstagePageContent;
  onSignalRevision(revision: number): void;
}) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const readyPreparation = preparation.status === 'ready' ? preparation : null;
  const plan = useMemo<NativeTrustedBlockPreparePlan>(() => {
    const preparedSource = readyPreparation
      ? `/* prepared:${readyPreparation.prepared.identityInput.sourceSha256} */`
      : '';
    return {
      runtime: NATIVE_TRUSTED_BLOCK_RUNTIME,
      blockId: item.blockId,
      entry: item.runtime.entry ?? 'default',
      source: preparedSource,
      normalizedSource: preparedSource,
      props: { ...item.props },
      requiredPermissions: [NATIVE_TRUSTED_BLOCK_PERMISSION]
    };
  }, [
    item.blockId,
    item.props,
    item.runtime.entry,
    readyPreparation?.prepared.identityInput.sourceSha256
  ]);
  const instanceEpochOwner = useMemo(
    () =>
      signalCoordinator
        ? {
            begin: () => signalCoordinator.beginInstance(item.blockId),
            end: (instanceEpoch: string) =>
              signalCoordinator.endInstance(item.blockId, instanceEpoch)
          }
        : undefined,
    [item.blockId, signalCoordinator]
  );
  const createRuntimeInput = useCallback(
    (instanceEpoch: string) => {
      const unavailable = createFrontstageUnavailableBlockContext(plan);
      const outputs = signalCoordinator
        ? signalCoordinator.outputsFor(
            item.blockId,
            instanceEpoch,
            onSignalRevision
          )
        : unavailable.outputs;
      return {
        plan,
        context: {
          ...unavailable,
          ...(runtimeContext ?? {}),
          page: {
            id: pageContent?.page.id ?? unavailable.page.id,
            route:
              pageContent?.tab.routeSegment ??
              pageContent?.page.id ??
              unavailable.page.route,
            ...(pageContent?.page.title
              ? { title: pageContent.page.title }
              : {})
          },
          inputs: signalCoordinator?.inputsFor(item.blockId) ?? {},
          outputs,
          props: { ...plan.props }
        }
      };
    },
    [
      item.blockId,
      onSignalRevision,
      pageContent,
      plan,
      runtimeContext,
      signalCoordinator,
      signalRevision
    ]
  );
  const instanceState = useFrontstageNativeBlockInstance({
    root,
    mountIntent: readyPreparation?.mountIntent ?? null,
    prepared: readyPreparation?.prepared ?? null,
    createRuntimeInput,
    instanceEpochOwner
  });

  if (preparation.status === 'failed' || instanceState.status === 'failed') {
    return (
      <div style={contentViewportStyle}>
        <Alert
          type="error"
          showIcon
          message={i18nText('frontstage', 'auto.runtime_preview_unavailable')}
          action={
            onRetry ? (
              <Button size="small" onClick={onRetry}>
                {i18nText('frontstage', 'auto.retry')}
              </Button>
            ) : undefined
          }
        />
      </div>
    );
  }

  if (!readyPreparation?.mountIntent) {
    return (
      <div style={contentViewportStyle}>
        <BlockUiLoadingShell />
      </div>
    );
  }

  return (
    <div style={contentViewportStyle}>
      <div
        ref={setRoot}
        data-testid={`frontstage-native-block-root-${item.blockId}`}
        style={{ width: '100%', minWidth: 0 }}
      />
      {instanceState.status === 'unmounted' ||
      instanceState.status === 'mounting' ? (
        <BlockUiLoadingShell />
      ) : null}
    </div>
  );
}

function RenderPlanSlot({
  item,
  runtimePreparation,
  signalCoordinator,
  signalRevision,
  runtimeContext,
  pageContent,
  onSignalRevision,
  runtimeSessionEntry,
  isSelected,
  onSelectBlock,
  isDesignMode,
  designActions,
  toolbarDisabled,
  onAutoHeightChange,
  onRuntimeDemandChange,
  onRuntimeRetry
}: RenderPlanSlotProps) {
  const [isHovered, setIsHovered] = useState(false);
  const blockRef = useRef<HTMLDivElement>(null);
  const rendererVersionError = resolveRendererVersionError(item);
  const isFixedHeight = item.presentation.heightMode === 'fixed';

  useEffect(() => {
    if (isSelected) {
      onRuntimeDemandChange?.(item.blockId, 0);
      return;
    }

    const node = blockRef.current;
    if (!node || typeof IntersectionObserver === 'undefined') {
      onRuntimeDemandChange?.(item.blockId, 1);
      return;
    }

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (!entry?.isIntersecting) {
          onRuntimeDemandChange?.(item.blockId, 3);
          return;
        }
        const rect = entry.boundingClientRect;
        const visible =
          rect.bottom > 0 &&
          rect.top < window.innerHeight &&
          rect.right > 0 &&
          rect.left < window.innerWidth;
        onRuntimeDemandChange?.(item.blockId, visible ? 1 : 2);
      },
      { rootMargin: '400px 0px' }
    );
    observer.observe(node);
    return () => observer.disconnect();
  }, [isSelected, item.blockId, onRuntimeDemandChange]);

  useEffect(() => {
    const node = blockRef.current;
    if (!node || isFixedHeight || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(([entry]) => {
      if (entry) {
        onAutoHeightChange?.(item.blockId, entry.contentRect.height);
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [isFixedHeight, item.blockId, onAutoHeightChange]);

  // Determine border style based on mode
  let borderStyle: CSSProperties;
  if (isDesignMode) {
    if (isSelected) {
      borderStyle = {
        border: `2px solid ${FRONTSTAGE_DESIGN_BLUE.borderSelected}`,
        background: FRONTSTAGE_DESIGN_BLUE.bgSelected
      };
    } else if (isHovered) {
      borderStyle = {
        border: `1px solid ${FRONTSTAGE_DESIGN_BLUE.borderHover}`,
        background: FRONTSTAGE_DESIGN_BLUE.bgHover
      };
    } else {
      borderStyle = {
        border: `1px solid ${FRONTSTAGE_DESIGN_BLUE.borderIdle}`
      };
    }
  } else {
    borderStyle = isSelected
      ? { border: '2px solid #1677ff' }
      : { border: '1px solid transparent' };
  }

  const isToolbarVisible = !!(isDesignMode && (isHovered || isSelected));
  const contentViewportStyle: CSSProperties = {
    height: isFixedHeight ? '100%' : 'auto',
    boxSizing: 'border-box',
    overflow: isFixedHeight ? 'auto' : 'visible',
    padding: isDesignMode ? '40px 24px 20px' : 12
  };

  const handleSelect = () => {
    onSelectBlock?.(item.blockId);
  };

  // Render the actual block content
  const renderBlockContent = () => {
    if (rendererVersionError) {
      return (
        <div style={contentViewportStyle}>
          <Alert
            type="error"
            showIcon
            message={rendererVersionError.message}
            description={rendererVersionError.description}
          />
        </div>
      );
    }

    if (runtimePreparation) {
      return (
        <NativeRuntimeSlotSurface
          item={item}
          preparation={runtimePreparation}
          signalCoordinator={signalCoordinator}
          signalRevision={signalRevision}
          runtimeContext={runtimeContext}
          pageContent={pageContent}
          onSignalRevision={onSignalRevision}
          contentViewportStyle={contentViewportStyle}
          onRetry={
            onRuntimeRetry ? () => onRuntimeRetry(item.blockId) : undefined
          }
        />
      );
    }

    if (runtimeSessionEntry && 'snapshot' in runtimeSessionEntry) {
      return (
        <div style={contentViewportStyle}>
          <RestrictedBlockRuntimePreview
            snapshot={runtimeSessionEntry.snapshot}
            onRetry={
              onRuntimeRetry ? () => onRuntimeRetry(item.blockId) : undefined
            }
          />
        </div>
      );
    }

    if (runtimeSessionEntry?.status === 'factory_failed') {
      return (
        <div style={contentViewportStyle}>
          <Alert
            type="error"
            showIcon
            message={i18nText('frontstage', 'auto.runtime_preview_unavailable')}
            description={i18nText(
              'frontstage',
              'auto.restricted_runtime_session_create_failed'
            )}
          />
        </div>
      );
    }

    const isSourceLoading =
      runtimeSessionEntry?.status === 'skipped' &&
      (runtimeSessionEntry.skipReason === 'artifact_lookup_pending' ||
        (runtimeSessionEntry.skipReason === 'source_not_ready' &&
          (runtimeSessionEntry.sourceStatus === 'loading' ||
            runtimeSessionEntry.sourceStatus === 'dormant')));

    if (runtimeSessionEntry?.status === 'skipped' && !isSourceLoading) {
      return (
        <div style={contentViewportStyle}>
          <Typography.Text type="secondary" style={{ fontSize: 13 }}>
            {i18nText('frontstage', 'auto.block_skipped_run')}
          </Typography.Text>
        </div>
      );
    }

    return (
      <div style={contentViewportStyle}>
        <BlockUiLoadingShell />
      </div>
    );
  };

  return (
    <div
      ref={blockRef}
      style={{
        ...blockFrameBaseStyle,
        height: isFixedHeight
          ? `calc(100% - ${FRONTSTAGE_GRID_ROW_GAP}px)`
          : 'auto',
        overflow: isFixedHeight ? 'hidden' : 'visible',
        ...borderStyle,
        position: 'relative',
        transition: 'border-color 0.15s, background 0.15s'
      }}
      data-testid={`block-slot-${item.blockId}`}
      aria-label={
        isDesignMode
          ? i18nText('frontstage', 'auto.block_with_id', {
              value1: item.blockId
            })
          : undefined
      }
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onClick={handleSelect}
      role={isDesignMode ? 'button' : undefined}
      tabIndex={isDesignMode ? 0 : -1}
      onKeyDown={(event) => {
        if (isDesignMode && (event.key === 'Enter' || event.key === ' ')) {
          event.preventDefault();
          handleSelect();
        }
      }}
    >
      {isDesignMode ? (
        <span style={blockLabelStyle}>
          {i18nText('frontstage', 'auto.js_block')}
        </span>
      ) : null}
      {renderBlockContent()}

      {isDesignMode && designActions && isToolbarVisible && (
        <BlockHoverToolbar
          blockId={item.blockId}
          onEditCode={() => designActions.onEditCode(item.blockId)}
          onDelete={() => designActions.onDelete(item.blockId)}
          isVisible={isToolbarVisible}
          disabled={toolbarDisabled}
        />
      )}
    </div>
  );
}

// ─── PageCanvas ─────────────────────────────────────────────────────

export const PageCanvas: FC<PageCanvasProps> = ({
  content,
  isLoading,
  hasError,
  isPermissionDenied,
  selectedBlockId = null,
  onSelectBlock,
  onRetry,
  runtimeSessionEntries,
  runtimePreparations,
  runtimeContext,
  isDesignMode = false,
  designActions,
  toolbarDisabled = false,
  showTitle = true,
  onResponsiveLayoutSave,
  onRuntimeDemandChange,
  onRuntimeRetry
}) => {
  const document = useMemo(
    () => (content ? createFrontstagePageDocument(content) : null),
    [content]
  );
  const renderPlan = useMemo(
    () => (document ? createFrontstagePageRenderPlan(document) : null),
    [document]
  );
  const renderItems = useMemo(() => renderPlan?.items ?? [], [renderPlan]);
  const signalSession = useMemo(
    () => createFrontstagePageSignalSession(),
    [content?.page.id]
  );
  const signalCoordinator = useMemo(
    () =>
      document && content
        ? new FrontstageSignalRuntimeCoordinator(
            document.blocks,
            content.tab.id,
            signalSession
          )
        : null,
    [content?.tab.id, document?.page.id, signalSession]
  );
  const [signalRevision, setSignalRevision] = useState(0);
  useEffect(() => {
    signalCoordinator?.updateBlocks(document?.blocks ?? []);
    setSignalRevision(signalCoordinator?.revision ?? 0);
  }, [document?.blocks, signalCoordinator]);
  const handleSignalRevision = useCallback((revision: number) => {
    setSignalRevision(revision);
  }, []);
  const { width: measuredWidth, containerRef } = useContainerWidth({
    initialWidth: 1280
  });
  const gridWidth = measuredWidth > 0 ? measuredWidth : 1280;
  const interactionCompactor = useMemo(
    () => createFrontstageInteractionCompactor(document?.layoutMode ?? 'auto'),
    [document?.layoutMode]
  );
  const [autoHeights, setAutoHeights] = useState<Record<string, number>>({});
  const layouts = useMemo(() => {
    const responsiveLayouts = createFrontstageResponsiveLayouts(
      renderItems,
      autoHeights
    );
    return document?.layoutMode === 'free'
      ? responsiveLayouts
      : normalizeFrontstageAutomaticResponsiveLayouts(responsiveLayouts);
  }, [autoHeights, document?.layoutMode, renderItems]);
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

  const updateAutoHeight = (blockId: string, height: number) => {
    setAutoHeights((current) =>
      Math.abs((current[blockId] ?? 0) - height) < 1
        ? current
        : { ...current, [blockId]: height }
    );
  };

  if (isLoading) {
    return (
      <div
        style={{
          background: '#fafafa',
          border: '1px solid #f0f0f0',
          borderRadius: 6,
          padding: 12
        }}
      >
        <Space direction="vertical" size={4}>
          <Typography.Text strong>
            {i18nText('frontstage', 'auto.page_content_loading')}
          </Typography.Text>
          <Typography.Text type="secondary">
            {i18nText('frontstage', 'auto.reading_page_content_and_blocks')}
          </Typography.Text>
        </Space>
      </div>
    );
  }

  if (hasError) {
    if (isPermissionDenied) {
      return <PermissionDeniedState />;
    }

    return (
      <Alert
        type="error"
        showIcon
        message={i18nText('frontstage', 'auto.page_content_load_failed')}
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
    <Space direction="vertical" size={12} style={{ width: '100%' }}>
      {showTitle ? (
        <Typography.Title level={4} style={{ margin: 0 }}>
          {formatPageTitle(content)}
        </Typography.Title>
      ) : null}

      <div
        ref={containerRef}
        className="frontstage-page-canvas-grid"
        data-testid="page-canvas-render-slots"
      >
        {renderPlan.isEmpty && isDesignMode ? (
          <div data-testid="page-canvas-design-empty-state" />
        ) : renderPlan.isEmpty ? (
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
                interactionCompactor.begin(currentLayout, draggedItem.i);
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
                  'resize'
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
            {renderItems.map((item, slotIndex) => (
              <div key={item.blockId}>
                <RenderPlanSlot
                  item={item}
                  runtimePreparation={
                    runtimePreparations?.find(
                      (preparation) => preparation.blockId === item.blockId
                    ) ?? null
                  }
                  signalCoordinator={signalCoordinator}
                  signalRevision={signalRevision}
                  runtimeContext={runtimeContext}
                  pageContent={content}
                  onSignalRevision={handleSignalRevision}
                  runtimeSessionEntry={findRuntimeSessionEntryForSlot({
                    item,
                    slotIndex,
                    runtimeSessionEntries
                  })}
                  isSelected={item.blockId === selectedBlockId}
                  onSelectBlock={onSelectBlock}
                  isDesignMode={isDesignMode}
                  designActions={designActions}
                  toolbarDisabled={toolbarDisabled}
                  onAutoHeightChange={updateAutoHeight}
                  onRuntimeDemandChange={onRuntimeDemandChange}
                  onRuntimeRetry={onRuntimeRetry}
                />
              </div>
            ))}
          </ResponsiveGridLayout>
        )}
      </div>
    </Space>
  );
};
