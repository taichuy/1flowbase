import type { BlockContextSeed } from '@1flowbase/page-protocol';
import { frontstageNativeInstanceRenderKey } from '../../hooks/use-frontstage-native-block-instance';
import { NativeRuntimeSlotSurface } from '../page-canvas/NativeRuntimeSlotSurface';

import { Alert } from 'antd';
import { BlockUiLoadingShell } from '@1flowbase/block-renderer';

import type { IsolatedFrontendBlockCapabilityHandlers } from '@1flowbase/page-runtime';
import type { CSSProperties } from 'react';
import {
  memo,
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore
} from 'react';

import type { FrontstagePageContent } from '../../api/page-content';
import { BlockHoverToolbar } from '../BlockHoverToolbar';

import { type FrontstageBlockRenderPlanItem } from '../../lib/page-canvas/render-plan';
import { i18nText } from '../../../../shared/i18n/text';

import { FRONTSTAGE_DESIGN_BLUE } from '../../lib/design-mode-theme';
import {
  FRONTSTAGE_DEFAULT_AUTO_HEIGHT_PX,
  FRONTSTAGE_GRID_ROW_GAP
} from '../../lib/responsive-grid-layout';

import type { FrontstageRuntimeDemandPriority } from '../../lib/page-canvas/runtime-demand';
import { resolveFrontstageViewportDemandPriority } from '../../lib/page-canvas/runtime-demand';

import type { FrontstageIntrinsicMeasurementContext } from '../../lib/page-canvas/auto-height-layout';
import type { FrontstageNativePreparationSource } from '../../lib/page-canvas/native-runtime-preparation';

import { FrontstageSignalRuntimeCoordinator } from '../../lib/page-canvas/signal-runtime';
import type { FrontstageNativeBlockContextHost } from '../../lib/page-canvas/native-block-context-host';
import { FrontstageIsolatedFrontendBlockHost } from '../../lib/isolated-frontend-block-react-adapter';
import type { PreparedFrontstageIsolatedContribution } from '../../lib/isolated-frontend-block-contribution';

export type DesignBlockActions = {
  onEditCode: (blockId: string) => void;
  onDelete: (blockId: string) => void;
};

// ─── RenderPlanSlot ─────────────────────────────────────────────────

type RenderPlanSlotProps = {
  item: FrontstageBlockRenderPlanItem;
  runtimePreparations?: FrontstageNativePreparationSource | null;
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
    context: FrontstageIntrinsicMeasurementContext
  ) => void;
  intrinsicHeight?: number;
  allocatedHeightEpoch?: number;
  onRuntimeDemandChange?: (
    blockId: string,
    priority: FrontstageRuntimeDemandPriority
  ) => void;
  onRuntimeRetry?: (blockId: string) => void;
  onRuntimeRefresh?: (blockId: string) => void;
};

const FRONTSTAGE_DESIGN_PREVIEW_MIN_HEIGHT = 160;

const FRONTSTAGE_DEMAND_ENTER_MARGIN = 400;

const FRONTSTAGE_DEMAND_EXIT_MARGIN = 800;

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

export const RenderPlanSlot = memo(function RenderPlanSlot({
  item,
  runtimePreparations,
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
  allocatedHeightEpoch,
  onRuntimeDemandChange,
  onRuntimeRetry,
  onRuntimeRefresh
}: RenderPlanSlotProps) {
  const subscribePreparation = useCallback(
    (listener: () => void) =>
      runtimePreparations?.subscribeBlock(item.blockId, listener) ?? (() => {}),
    [item.blockId, runtimePreparations]
  );
  const getPreparationSnapshot = useCallback(
    () => runtimePreparations?.getBlockSnapshot(item.blockId) ?? null,
    [item.blockId, runtimePreparations]
  );
  const runtimePreparation = useSyncExternalStore(
    subscribePreparation,
    getPreparationSnapshot,
    getPreparationSnapshot
  );
  const [isHovered, setIsHovered] = useState(false);
  const blockRef = useRef<HTMLDivElement>(null);
  const intrinsicContentRef = useRef<HTMLDivElement>(null);
  const runtimeRenderIdentity =
    runtimePreparation?.status === 'ready' && runtimePreparation.mountIntent
      ? frontstageNativeInstanceRenderKey(runtimePreparation.mountIntent)
      : null;
  const measurementEpochRef = useRef({ key: '', value: 0 });
  const measurementEpochKey = `${runtimeRenderIdentity ?? 'unprepared'}:${runtimePreparation?.generation ?? 0}`;
  if (measurementEpochRef.current.key !== measurementEpochKey) {
    measurementEpochRef.current = {
      key: measurementEpochKey,
      value: measurementEpochRef.current.value + 1
    };
  }
  const measurementEpoch = measurementEpochRef.current.value;
  const rendererVersionError = resolveRendererVersionError(item);
  const isFixedHeight = item.presentation.heightMode === 'fixed';
  const fillsAvailableHeight =
    isFixedHeight || allocatedHeightEpoch === measurementEpoch;
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
    if (
      !node ||
      isFixedHeight ||
      typeof ResizeObserver === 'undefined' ||
      fillsAvailableHeight
    ) {
      return;
    }

    const observer = new ResizeObserver(([entry]) => {
      if (entry) {
        onAutoHeightChange?.(item.blockId, entry.contentRect.height, {
          epoch: measurementEpoch,
          source: 'dom-intrinsic'
        });
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, [
    isFixedHeight,
    fillsAvailableHeight,
    item.blockId,
    measurementEpoch,
    onAutoHeightChange
  ]);

  const handleIntrinsicSizeReport = useCallback(
    (height: number) => {
      if (!Number.isFinite(height) || height <= 0) {
        return;
      }
      onAutoHeightChange?.(item.blockId, height, {
        epoch: measurementEpoch,
        source: 'runtime-explicit'
      });
    },
    [item.blockId, measurementEpoch, onAutoHeightChange]
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
      data-flowbase-frontstage-allocation-epoch={allocatedHeightEpoch}
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
      onMouseEnter={isDesignMode ? () => setIsHovered(true) : undefined}
      onMouseLeave={isDesignMode ? () => setIsHovered(false) : undefined}
      onClick={(event) => {
        // Keyboard/assistive activation belongs to the originating control.
        // Pointer selection keeps the existing design-canvas behavior.
        if (
          event.defaultPrevented ||
          (event.detail === 0 &&
            event.nativeEvent.composedPath()[0] !== event.currentTarget)
        )
          return;
        handleSelect();
      }}
      role={isDesignMode ? 'button' : undefined}
      tabIndex={isDesignMode ? 0 : -1}
      onKeyDown={(event) => {
        // The slot owns only its own activation, never keys from runtime
        // controls or portals. composedPath avoids Shadow DOM retargeting;
        // unprevented input events still belong to the browser, not the canvas.
        if (
          !isDesignMode ||
          event.defaultPrevented ||
          event.nativeEvent.isComposing ||
          event.nativeEvent.keyCode === 229 ||
          event.ctrlKey ||
          event.metaKey ||
          event.altKey ||
          event.shiftKey ||
          event.nativeEvent.composedPath()[0] !== event.currentTarget ||
          (event.key !== 'Enter' && event.key !== ' ')
        )
          return;
        event.preventDefault();
        handleSelect();
      }}
    >
      {isDesignMode ? (
        <span style={blockLabelStyle}>
          {item.title != null
            ? (runtimeContext?.i18n?.t(item.title) ?? item.title)
            : item.blockId}
        </span>
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

export type FrontstagePageCanvasRuntimeContext = Pick<
  BlockContextSeed,
  'currentUser' | 'workspace' | 'application' | 'theme' | 'ui' | 'i18n'
>;

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
