import { Alert, Button, Empty, Space, Typography } from 'antd';
import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import type {
  BlockContextSizing,
  BlockContextSeed,
  BlockProtocolError
} from '@1flowbase/page-protocol';
import {
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  type IsolatedFrontendBlockCapabilityHandlers,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { CSSProperties, FC, Ref } from 'react';
import {
  memo,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore
} from 'react';
import {
  ResponsiveGridLayout,
  type Layout,
  type ResizeHandleAxis
} from 'react-grid-layout/react';
import 'react-grid-layout/css/styles.css';
import './page-canvas.css';

import type { FrontstagePageContent } from '../api/page-content';
import { BlockHoverToolbar } from './BlockHoverToolbar';
import { createFrontstagePageDocument } from '../lib/page-document';
import {
  createFrontstageBlockRenderPlanItems,
  createFrontstagePageRenderPlan,
  type FrontstageBlockRenderPlanItem
} from '../lib/page-canvas/render-plan';
import { i18nText } from '../../../shared/i18n/text';
import { PermissionDeniedState } from '../../../shared/ui/PermissionDeniedState';
import { FRONTSTAGE_DESIGN_BLUE } from '../lib/design-mode-theme';
import {
  createFrontstagePersistedGridLayout,
  createFrontstageResponsiveLayouts,
  FRONTSTAGE_DEFAULT_AUTO_HEIGHT_PX,
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
import type {
  FrontstageBlockInstance,
  FrontstageBlockPresentation
} from '../lib/page-document';
import {
  createFrontstageInteractionCompactor,
  frontstageLayoutsEqualForCommit
} from '../lib/page-canvas/frontstage-block-interaction';
import type { FrontstageRuntimeDemandPriority } from '../lib/page-canvas/runtime-demand';
import { resolveFrontstageViewportDemandPriority } from '../lib/page-canvas/runtime-demand';
import {
  FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES,
  FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
  FrontstageAutoHeightBatch,
  resolveFrontstageAutoHeightScrollDelta
} from '../lib/page-canvas/auto-height-layout';
import type { FrontstageNativePreparationSnapshot } from '../lib/page-canvas/native-runtime-preparation';
import {
  frontstageNativeInstanceRenderKey,
  useFrontstageNativeBlockInstance
} from '../hooks/use-frontstage-native-block-instance';
import {
  createFrontstageUnavailableBlockContext,
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../lib/native-trusted-block-react-adapter';
import {
  createFrontstagePageSignalSession,
  FrontstageSignalRuntimeCoordinator
} from '../lib/page-canvas/signal-runtime';
import {
  createFrontstageNativeBlockContextCapabilities,
  type FrontstageNativeBlockContextHost
} from '../lib/page-canvas/native-block-context-host';
import { FrontstageIsolatedFrontendBlockHost } from '../lib/isolated-frontend-block-react-adapter';
import type { PreparedFrontstageIsolatedContribution } from '../lib/isolated-frontend-block-contribution';
import { createFrontstageAssistantDomRuntime } from '../lib/assistant-frontstage-runtime-dom';
import { registerFrontstageAssistantRuntime } from '../lib/assistant-frontstage-runtime';

export type FrontstagePageCanvasRuntimeContext = Pick<
  BlockContextSeed,
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
  runtimePreparations?: readonly FrontstageNativePreparationSnapshot[] | null;
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

const FRONTSTAGE_CANVAS_INITIAL_WIDTH = 1280;

function useFrontstagePageCanvasWidth() {
  const [containerNode, setContainerNode] = useState<HTMLDivElement | null>(
    null
  );
  const [width, setWidth] = useState(FRONTSTAGE_CANVAS_INITIAL_WIDTH);
  const containerRef = useCallback((node: HTMLDivElement | null) => {
    setContainerNode(node);
  }, []);

  useEffect(() => {
    if (!containerNode) return;

    const updateWidth = (nextWidth: number) => {
      if (!Number.isFinite(nextWidth) || nextWidth <= 0) return;
      setWidth((currentWidth) =>
        Math.abs(currentWidth - nextWidth) < 0.5 ? currentWidth : nextWidth
      );
    };

    updateWidth(containerNode.offsetWidth);
    if (typeof ResizeObserver === 'undefined') return;

    const observer = new ResizeObserver(([entry]) => {
      if (entry) updateWidth(entry.contentRect.width);
    });
    observer.observe(containerNode);
    return () => observer.disconnect();
  }, [containerNode]);

  return { width, containerNode, containerRef };
}

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

// ─── RenderPlanSlot ─────────────────────────────────────────────────

type RenderPlanSlotProps = {
  item: FrontstageBlockRenderPlanItem;
  runtimePreparation?: FrontstageNativePreparationSnapshot | null;
  isolatedPreparation?: PreparedFrontstageIsolatedContribution | null;
  isolatedPreparationError?: Error | null;
  isolatedCapabilityHandlers?: IsolatedFrontendBlockCapabilityHandlers;
  signalCoordinator?: FrontstageSignalRuntimeCoordinator | null;
  runtimeInputValues?: Readonly<Record<string, unknown>>;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
  nativeContextHost?: FrontstageNativeBlockContextHost;
  pageContent?: FrontstagePageContent;
  isSelected: boolean;
  onSelectBlock?: (blockId: string | null) => void;
  isDesignMode?: boolean;
  designActions?: DesignBlockActions;
  toolbarDisabled?: boolean;
  onAutoHeightChange?: (
    blockId: string,
    height: number,
    renderIdentity: string | null
  ) => void;
  intrinsicHeight?: number;
  onRuntimeDemandChange?: (
    blockId: string,
    priority: FrontstageRuntimeDemandPriority
  ) => void;
  onRuntimeRetry?: (blockId: string) => void;
  onRuntimeRefresh?: (blockId: string) => void;
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

const FRONTSTAGE_DESIGN_PREVIEW_MIN_HEIGHT = 160;
const FRONTSTAGE_DEMAND_ENTER_MARGIN = 400;
const FRONTSTAGE_DEMAND_EXIT_MARGIN = 800;

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

function IsolatedRuntimeSlotSurface({
  preparation,
  capabilityHandlers,
  contentViewportStyle
}: {
  preparation: PreparedFrontstageIsolatedContribution;
  capabilityHandlers?: IsolatedFrontendBlockCapabilityHandlers;
  contentViewportStyle: CSSProperties;
}) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const [runtimeError, setRuntimeError] = useState<Error | null>(null);
  useEffect(
    () => setRuntimeError(null),
    [preparation.contributionId, preparation.program.source]
  );
  if (runtimeError) {
    return (
      <div
        className="frontstage-native-block-state frontstage-native-block-state--error"
        style={contentViewportStyle}
      >
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'auto.runtime_preview_unavailable')}
          description={runtimeError.message}
        />
      </div>
    );
  }
  return (
    <div style={contentViewportStyle}>
      <div
        ref={setRoot}
        data-testid={`frontstage-isolated-block-root-${preparation.blockInstanceId}`}
        style={{ width: '100%', minWidth: 0, height: '100%' }}
      />
      {root ? (
        <FrontstageIsolatedFrontendBlockHost
          root={root}
          preparation={preparation}
          capabilityHandlers={capabilityHandlers}
          onRuntimeError={setRuntimeError}
        />
      ) : (
        <BlockUiLoadingShell />
      )}
    </div>
  );
}

function IsolatedRuntimeErrorSurface({
  error,
  contentViewportStyle
}: {
  error: Error;
  contentViewportStyle: CSSProperties;
}) {
  return (
    <div
      className="frontstage-native-block-state frontstage-native-block-state--error"
      style={contentViewportStyle}
    >
      <Alert
        type="error"
        showIcon
        title={i18nText('frontstage', 'auto.runtime_preview_unavailable')}
        description={error.message}
      />
    </div>
  );
}

function NativeRuntimeSlotSurface({
  item,
  preparation,
  signalCoordinator,
  runtimeInputValues,
  runtimeContext,
  nativeContextHost,
  pageContent,
  contentViewportStyle,
  surfaceLayoutEpoch,
  fillsAvailableHeight,
  onIntrinsicSizeReport,
  onRetry
}: {
  item: FrontstageBlockRenderPlanItem;
  preparation: FrontstageNativePreparationSnapshot;
  contentViewportStyle: CSSProperties;
  surfaceLayoutEpoch: string;
  onRetry?: () => void;
  signalCoordinator?: FrontstageSignalRuntimeCoordinator | null;
  runtimeInputValues?: Readonly<Record<string, unknown>>;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
  nativeContextHost?: FrontstageNativeBlockContextHost;
  pageContent?: FrontstagePageContent;
  fillsAvailableHeight: boolean;
  onIntrinsicSizeReport?: (height: number) => void;
}) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const [viewport, setViewport] = useState<HTMLDivElement | null>(null);
  const [availableSize, setAvailableSize] = useState({ width: 0, height: 0 });
  const [runtimeError, setRuntimeError] = useState<BlockProtocolError | null>(
    null
  );
  const [retryGeneration, setRetryGeneration] = useState(0);
  const readyPreparation = preparation.status === 'ready' ? preparation : null;
  const renderIdentity = readyPreparation?.mountIntent
    ? frontstageNativeInstanceRenderKey(readyPreparation.mountIntent)
    : null;
  useEffect(() => {
    if (!viewport || typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(([entry]) => {
      if (!entry) return;
      const { width, height } = entry.contentRect;
      if (!Number.isFinite(width) || !Number.isFinite(height)) return;
      setAvailableSize((current) =>
        Math.abs(current.width - width) < 0.5 &&
        Math.abs(current.height - height) < 0.5
          ? current
          : { width, height }
      );
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [viewport]);
  const reportIntrinsicSize = useCallback(
    ({ height }: { height: number }) => onIntrinsicSizeReport?.(height),
    [onIntrinsicSizeReport]
  );
  const runtimeSizing = useMemo<BlockContextSizing>(
    () => ({ available: availableSize, reportIntrinsicSize }),
    [availableSize, reportIntrinsicSize]
  );
  useEffect(() => setRuntimeError(null), [renderIdentity]);
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
  if (preparation.status === 'failed' || runtimeError) {
    const retry =
      preparation.status === 'failed'
        ? onRetry
        : () => {
            setRuntimeError(null);
            setRetryGeneration((current) => current + 1);
          };
    return (
      <div
        className="frontstage-native-block-state frontstage-native-block-state--error"
        style={contentViewportStyle}
      >
        <Alert
          type="error"
          showIcon
          title={i18nText('frontstage', 'auto.runtime_preview_unavailable')}
          description={
            preparation.status === 'failed'
              ? preparation.error.message
              : runtimeError?.message
          }
          action={
            retry ? (
              <Button size="small" onClick={retry}>
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
      <div
        className="frontstage-native-block-state frontstage-native-block-state--loading"
        style={contentViewportStyle}
      >
        <BlockUiLoadingShell />
      </div>
    );
  }

  return (
    <div ref={setViewport} style={contentViewportStyle}>
      <div
        ref={setRoot}
        data-testid={`frontstage-native-block-root-${item.blockId}`}
        style={{
          width: '100%',
          maxWidth: '100%',
          minWidth: 0,
          height: fillsAvailableHeight ? '100%' : undefined
        }}
      />
      {root ? (
        <FrontstageNativeRuntimeInstance
          key={`${renderIdentity}:${preparation.generation}:${retryGeneration}`}
          root={root}
          item={item}
          plan={plan}
          preparation={readyPreparation}
          signalCoordinator={signalCoordinator}
          runtimeInputValues={runtimeInputValues}
          runtimeContext={runtimeContext}
          nativeContextHost={nativeContextHost}
          pageContent={pageContent}
          runtimeSizing={runtimeSizing}
          surfaceLayoutEpoch={surfaceLayoutEpoch}
          onRuntimeError={setRuntimeError}
        />
      ) : (
        <BlockUiLoadingShell />
      )}
    </div>
  );
}

const EMPTY_BLOCK_SIGNAL_SNAPSHOT = Object.freeze({
  revision: 0,
  inputs: Object.freeze({})
});

function FrontstageNativeRuntimeInstance({
  root,
  item,
  plan,
  preparation,
  signalCoordinator,
  runtimeInputValues,
  runtimeContext,
  nativeContextHost,
  pageContent,
  runtimeSizing,
  surfaceLayoutEpoch,
  onRuntimeError
}: {
  root: Element;
  item: FrontstageBlockRenderPlanItem;
  plan: NativeTrustedBlockPreparePlan;
  preparation: Extract<
    FrontstageNativePreparationSnapshot,
    { status: 'ready' }
  >;
  signalCoordinator?: FrontstageSignalRuntimeCoordinator | null;
  runtimeInputValues?: Readonly<Record<string, unknown>>;
  runtimeContext?: FrontstagePageCanvasRuntimeContext;
  nativeContextHost?: FrontstageNativeBlockContextHost;
  pageContent?: FrontstagePageContent;
  runtimeSizing: BlockContextSizing;
  surfaceLayoutEpoch: string;
  onRuntimeError(error: BlockProtocolError): void;
}) {
  const { instanceEpoch, isCurrentInstance } = useFrontstageNativeBlockInstance(
    {
      blockId: item.blockId,
      signalCoordinator,
      observationContext: preparation.observationContext,
      cacheTier: preparation.prepared.artifactCacheTier,
      preparationGeneration: preparation.generation
    }
  );
  const subscribe = useCallback(
    (listener: () => void) =>
      signalCoordinator?.subscribeBlock(item.blockId, listener) ?? (() => {}),
    [item.blockId, signalCoordinator]
  );
  const getSnapshot = useCallback(
    () =>
      signalCoordinator?.getBlockSnapshot(item.blockId) ??
      EMPTY_BLOCK_SIGNAL_SNAPSHOT,
    [item.blockId, signalCoordinator]
  );
  const signalSnapshot = useSyncExternalStore(
    subscribe,
    getSnapshot,
    getSnapshot
  );
  const unavailable = createFrontstageUnavailableBlockContext(plan);
  const [runtimeState] = useState(() => {
    const state: Record<string, unknown> = {};
    return {
      state,
      patch: (patch: Record<string, unknown>) => {
        Object.assign(state, patch);
      }
    };
  });
  const outputs = signalCoordinator
    ? signalCoordinator.outputsFor(item.blockId, instanceEpoch)
    : unavailable.outputs;
  const capabilities = nativeContextHost
    ? createFrontstageNativeBlockContextCapabilities({
        host: nativeContextHost,
        pageId: pageContent?.page.id ?? unavailable.page.id,
        tabId: pageContent?.tab.id ?? '',
        blockId: item.blockId,
        instanceEpoch,
        isCurrentInstance,
        outputs
      })
    : {
        api: unavailable.api,
        events: unavailable.events,
        navigation: unavailable.navigation,
        outputs
      };
  const context: BlockContextSeed = {
    ...unavailable,
    ...(runtimeContext ?? {}),
    page: {
      id: pageContent?.page.id ?? unavailable.page.id,
      route:
        pageContent?.tab.routeSegment ??
        pageContent?.page.id ??
        unavailable.page.route,
      ...(pageContent?.page.title ? { title: pageContent.page.title } : {})
    },
    inputs: { ...signalSnapshot.inputs, ...runtimeInputValues },
    ...capabilities,
    ui: {
      ...unavailable.ui,
      ...(runtimeContext?.ui ?? {}),
      sizing: runtimeSizing
    },
    props: { ...plan.props },
    state: runtimeState.state,
    patch: runtimeState.patch
  };

  return (
    <FrontstageNativeTrustedBlockPortalHost
      root={root}
      renderEpoch={instanceEpoch}
      plan={plan}
      component={
        preparation.prepared
          .component as FrontstageNativeTrustedBlockReactComponent
      }
      ctx={context}
      moduleAssets={preparation.prepared.moduleAssets}
      moduleSources={preparation.prepared.moduleSources}
      contribution={preparation.prepared.contribution}
      surfaceLayoutEpoch={surfaceLayoutEpoch}
      onRuntimeError={onRuntimeError}
    />
  );
}

const RenderPlanSlot = memo(function RenderPlanSlot({
  item,
  runtimePreparation,
  isolatedPreparation,
  isolatedPreparationError,
  isolatedCapabilityHandlers,
  signalCoordinator,
  runtimeInputValues,
  runtimeContext,
  nativeContextHost,
  pageContent,
  isSelected,
  onSelectBlock,
  isDesignMode,
  designActions,
  toolbarDisabled,
  onAutoHeightChange,
  intrinsicHeight,
  onRuntimeDemandChange,
  onRuntimeRetry,
  onRuntimeRefresh
}: RenderPlanSlotProps) {
  const [isHovered, setIsHovered] = useState(false);
  const blockRef = useRef<HTMLDivElement>(null);
  const intrinsicContentRef = useRef<HTMLDivElement>(null);
  const runtimeIntrinsicOwnerRef = useRef<string | null>(null);
  const [runtimeIntrinsicOwner, setRuntimeIntrinsicOwner] = useState<
    string | null
  >(null);
  const runtimeRenderIdentity =
    runtimePreparation?.status === 'ready' &&
    runtimePreparation.mountIntent
      ? frontstageNativeInstanceRenderKey(runtimePreparation.mountIntent)
      : null;
  const fillsAvailableHeight =
    runtimeRenderIdentity !== null &&
    runtimeIntrinsicOwner === runtimeRenderIdentity;
  const rendererVersionError = resolveRendererVersionError(item);
  const isFixedHeight = item.presentation.heightMode === 'fixed';
  const viewportDemand = useRef({
    priority: 3 as FrontstageRuntimeDemandPriority,
    visible: false,
    withinEnterMargin: false,
    withinExitMargin: false
  });

  useEffect(() => {
    if (isSelected) {
      viewportDemand.current.priority = 0;
      onRuntimeDemandChange?.(item.blockId, 0);
      return;
    }

    const node = blockRef.current;
    if (!node || typeof IntersectionObserver === 'undefined') {
      onRuntimeDemandChange?.(item.blockId, 1);
      return;
    }

    const publishDemand = () => {
      const current = viewportDemand.current;
      const priority = resolveFrontstageViewportDemandPriority({
        previousPriority: current.priority,
        visible: current.visible,
        withinEnterMargin: current.withinEnterMargin,
        withinExitMargin: current.withinExitMargin
      });
      if (priority === current.priority) return;
      current.priority = priority;
      onRuntimeDemandChange?.(item.blockId, priority);
    };
    const observeBand = (
      field: 'visible' | 'withinEnterMargin' | 'withinExitMargin',
      rootMargin: string
    ) => {
      const root = node.closest<HTMLElement>(
        '[data-flowbase-frontstage-scroll-owner]'
      );
      const observer = new IntersectionObserver(
        ([entry]) => {
          viewportDemand.current[field] = entry?.isIntersecting === true;
          publishDemand();
        },
        { root, rootMargin }
      );
      observer.observe(node);
      return observer;
    };
    const observers = [
      observeBand('visible', '0px'),
      observeBand(
        'withinEnterMargin',
        `${FRONTSTAGE_DEMAND_ENTER_MARGIN}px 0px`
      ),
      observeBand('withinExitMargin', `${FRONTSTAGE_DEMAND_EXIT_MARGIN}px 0px`)
    ];
    return () => observers.forEach((observer) => observer.disconnect());
  }, [isSelected, item.blockId, onRuntimeDemandChange]);

  useEffect(() => {
    const node = intrinsicContentRef.current;
    if (!node || isFixedHeight || typeof ResizeObserver === 'undefined') {
      return;
    }

    const observer = new ResizeObserver(([entry]) => {
      if (
        entry &&
        runtimeIntrinsicOwnerRef.current !== runtimeRenderIdentity
      ) {
        onAutoHeightChange?.(
          item.blockId,
          entry.contentRect.height,
          runtimeRenderIdentity
        );
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [
    isFixedHeight,
    item.blockId,
    onAutoHeightChange,
    runtimeRenderIdentity
  ]);

  const handleIntrinsicSizeReport = useCallback(
    (height: number) => {
      if (
        !runtimeRenderIdentity ||
        !Number.isFinite(height) ||
        height <= 0
      ) {
        return;
      }
      runtimeIntrinsicOwnerRef.current = runtimeRenderIdentity;
      setRuntimeIntrinsicOwner(runtimeRenderIdentity);
      onAutoHeightChange?.(item.blockId, height, runtimeRenderIdentity);
    },
    [item.blockId, onAutoHeightChange, runtimeRenderIdentity]
  );

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
    width: '100%',
    maxWidth: '100%',
    minWidth: 0,
    height: isFixedHeight || fillsAvailableHeight ? '100%' : 'auto',
    minHeight:
      isDesignMode && !isFixedHeight
        ? FRONTSTAGE_DESIGN_PREVIEW_MIN_HEIGHT
        : undefined,
    boxSizing: 'border-box',
    overflow: isDesignMode ? 'clip' : isFixedHeight ? 'auto' : 'visible',
    padding: isDesignMode ? '40px 24px 20px' : 12,
    ...(isDesignMode
      ? {
          position: 'relative',
          zIndex: 0,
          isolation: 'isolate',
          contain: 'layout paint'
        }
      : {})
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
            title={rendererVersionError.message}
            description={rendererVersionError.description}
          />
        </div>
      );
    }

    if (item.renderMode === 'isolated_iframe') {
      if (isolatedPreparationError) {
        return (
          <IsolatedRuntimeErrorSurface
            error={isolatedPreparationError}
            contentViewportStyle={contentViewportStyle}
          />
        );
      }
      return isolatedPreparation ? (
        <IsolatedRuntimeSlotSurface
          preparation={isolatedPreparation}
          capabilityHandlers={isolatedCapabilityHandlers}
          contentViewportStyle={contentViewportStyle}
        />
      ) : (
        <div style={contentViewportStyle}>
          <BlockUiLoadingShell />
        </div>
      );
    }

    if (runtimePreparation) {
      return (
        <NativeRuntimeSlotSurface
          item={item}
          preparation={runtimePreparation}
          signalCoordinator={signalCoordinator}
          runtimeInputValues={runtimeInputValues}
          runtimeContext={runtimeContext}
          nativeContextHost={nativeContextHost}
          pageContent={pageContent}
          contentViewportStyle={contentViewportStyle}
          surfaceLayoutEpoch={isDesignMode ? 'design' : 'preview'}
          fillsAvailableHeight={fillsAvailableHeight}
          onIntrinsicSizeReport={handleIntrinsicSizeReport}
          onRetry={
            onRuntimeRetry ? () => onRuntimeRetry(item.blockId) : undefined
          }
        />
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
        height: `calc(100% - ${FRONTSTAGE_GRID_ROW_GAP}px)`,
        overflow: isFixedHeight ? 'hidden' : 'visible',
        ...borderStyle,
        position: 'relative',
        transition: 'border-color 0.15s, background 0.15s',
        contentVisibility: 'auto',
        containIntrinsicSize: `auto ${Math.max(
          1,
          intrinsicHeight ?? FRONTSTAGE_DEFAULT_AUTO_HEIGHT_PX
        )}px`
      }}
      data-testid={`block-slot-${item.blockId}`}
      data-flowbase-frontstage-block-id={item.blockId}
      data-flowbase-frontstage-render-status={
        rendererVersionError || isolatedPreparationError
          ? 'failed'
          : (runtimePreparation?.status ??
            (isolatedPreparation ? 'ready' : 'loading'))
      }
      data-flowbase-frontstage-generation={runtimePreparation?.generation ?? 0}
      data-flowbase-frontstage-intrinsic-height={
        intrinsicHeight ?? FRONTSTAGE_DEFAULT_AUTO_HEIGHT_PX
      }
      data-flowbase-frontstage-render-error={
        rendererVersionError?.description ??
        isolatedPreparationError?.message ??
        (runtimePreparation?.status === 'failed'
          ? runtimePreparation.error.message
          : undefined)
      }
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
        <span style={blockLabelStyle}>{item.title ?? item.blockId}</span>
      ) : null}
      {isFixedHeight ? (
        renderBlockContent()
      ) : (
        <div
          ref={intrinsicContentRef}
          data-flowbase-frontstage-intrinsic-content={item.blockId}
          style={{
            width: '100%',
            maxWidth: '100%',
            minWidth: 0,
            height: fillsAvailableHeight ? '100%' : undefined
          }}
        >
          {renderBlockContent()}
        </div>
      )}

      {isDesignMode && designActions && isToolbarVisible && (
        <BlockHoverToolbar
          blockId={item.blockId}
          onEditCode={() => designActions.onEditCode(item.blockId)}
          onDelete={() => designActions.onDelete(item.blockId)}
          onRefresh={
            item.renderMode === 'native_react' && onRuntimeRefresh
              ? () => onRuntimeRefresh(item.blockId)
              : undefined
          }
          isVisible={isToolbarVisible}
          disabled={toolbarDisabled}
        />
      )}
    </div>
  );
});

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
  const autoHeightScopeRef = useRef(content?.page.id);
  const autoHeightBatchRef = useRef(
    new FrontstageAutoHeightBatch({
      settleMs: FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
      settleFrames: FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES
    })
  );
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
    if (autoHeightScopeRef.current === content?.page.id) return;
    autoHeightScopeRef.current = content?.page.id;
    autoRowsRef.current = {};
    autoHeightBatchRef.current = new FrontstageAutoHeightBatch({
      settleMs: FRONTSTAGE_AUTO_HEIGHT_SETTLE_MS,
      settleFrames: FRONTSTAGE_AUTO_HEIGHT_SETTLE_FRAMES
    });
    setAutoRows({});
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
    if (
      !anchor ||
      Math.abs(anchor.scrollOwner.scrollTop - anchor.scrollTop) > 0.5
    ) {
      return;
    }
    anchor.scrollOwner.scrollTop += anchor.layoutDeltaPx;
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
    (blockId: string, height: number, renderIdentity: string | null) => {
      autoHeightBatchRef.current.measure(
        blockId,
        height,
        renderIdentity,
        performance.now()
      );
      if (autoHeightFrameRef.current !== null) return;
      const flush = (nowMs: number) => {
        autoHeightFrameRef.current = null;
        const currentRows = autoRowsRef.current;
        const nextRows = autoHeightBatchRef.current.commit(currentRows, nowMs);
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
        if (autoHeightBatchRef.current.hasPendingMeasurements()) {
          autoHeightFrameRef.current = requestAnimationFrame(flush);
        }
      };
      autoHeightFrameRef.current = requestAnimationFrame(flush);
    },
    [document?.layoutMode, gridContainerNode, renderItems]
  );

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
        <Space orientation="vertical" size={4}>
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
        className="frontstage-page-canvas-grid"
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
        {isRenderEmpty && isDesignMode ? (
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
                  runtimePreparation={
                    runtimePreparations?.find(
                      (preparation) => preparation.blockId === item.blockId
                    ) ?? null
                  }
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
