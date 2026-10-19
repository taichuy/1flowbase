import { Alert, Button } from 'antd';
import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import type { BlockContextSeed, BlockContextSizing, BlockProtocolError } from '@1flowbase/page-protocol';
import { NATIVE_TRUSTED_BLOCK_PERMISSION, NATIVE_TRUSTED_BLOCK_RUNTIME, type NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime';
import { useCallback, useEffect, useLayoutEffect, useMemo, useState, useSyncExternalStore, type CSSProperties } from 'react';
import type { FrontstagePageContent } from '../../api/page-content';
import type { FrontstagePageCanvasRuntimeContext } from '../PageCanvas';
import type { FrontstageBlockRenderPlanItem } from '../../lib/page-canvas/render-plan';
import type { FrontstageNativePreparationSnapshot } from '../../lib/page-canvas/native-runtime-preparation';
import type { FrontstageSignalRuntimeCoordinator } from '../../lib/page-canvas/signal-runtime';
import { frontstageNativeInstanceRenderKey, useFrontstageNativeBlockInstance } from '../../hooks/use-frontstage-native-block-instance';
import { FrontstageNativeTrustedBlockPortalHost, type FrontstageNativeTrustedBlockReactComponent } from '../../lib/native-adapter/native-trusted-block-react-adapter';
import { createFrontstageUnavailableBlockContext } from '../../lib/native-trusted-block/unavailable-context';
import { createFrontstageNativeBlockContextCapabilities, type FrontstageNativeBlockContextHost } from '../../lib/page-canvas/native-block-context-host';
import { i18nText } from '../../../../shared/i18n/text';

export function NativeRuntimeSlotSurface({
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
  useLayoutEffect(() => {
    if (!viewport) return;
    const updateSize = ({
      width,
      height
    }: {
      width: number;
      height: number;
    }) => {
      // display:none reports zero: retain the last measured native allocation.
      if (
        !Number.isFinite(width) ||
        !Number.isFinite(height) ||
        width <= 0 ||
        height <= 0
      )
        return;
      setAvailableSize((current) =>
        Math.abs(current.width - width) < 0.5 &&
        Math.abs(current.height - height) < 0.5
          ? current
          : { width, height }
      );
    };
    updateSize(viewport.getBoundingClientRect());
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(([entry]) => {
      if (entry) updateSize(entry.contentRect);
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
  const preparedSourceSha256 = readyPreparation?.prepared.identityInput.sourceSha256;
  const plan = useMemo<NativeTrustedBlockPreparePlan>(() => {
    const preparedSource = preparedSourceSha256
      ? `/* prepared:${preparedSourceSha256} */`
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
    preparedSourceSha256
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
    <div
      ref={setViewport}
      style={contentViewportStyle}
      data-flowbase-frontstage-allocation-viewport={item.blockId}
      data-flowbase-frontstage-available-width={availableSize.width}
      data-flowbase-frontstage-available-height={availableSize.height}
    >
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
