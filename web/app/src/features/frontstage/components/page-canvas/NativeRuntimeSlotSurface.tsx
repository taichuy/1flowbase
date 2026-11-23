import { Alert, Button } from 'antd';
import { BlockUiLoadingShell } from '@1flowbase/block-renderer/loading-shell';
import type { BlockContextSeed, BlockContextSizing, BlockProtocolError } from '@1flowbase/page-protocol';
import { NATIVE_TRUSTED_BLOCK_PERMISSION, NATIVE_TRUSTED_BLOCK_RUNTIME, type NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime/browser';
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, useSyncExternalStore, type CSSProperties } from 'react';
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

export type FrontstageNativeSlotLayoutPhase =
  | 'loading'
  | 'initial_api_pending'
  | 'settled';

const INITIAL_API_RATCHET_SAFETY_TIMEOUT_MS = 5_000;

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
  onLayoutPhaseChange,
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
  onLayoutPhaseChange?: (phase: FrontstageNativeSlotLayoutPhase) => void;
}) {
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const [viewport, setViewport] = useState<HTMLDivElement | null>(null);
  const [availableSize, setAvailableSize] = useState({ width: 0, height: 0 });
  const [runtimeError, setRuntimeError] = useState<BlockProtocolError | null>(
    null
  );
  const [retryGeneration, setRetryGeneration] = useState(0);
  const [committedInstanceKey, setCommittedInstanceKey] = useState<
    string | null
  >(null);
  const readyPreparation = preparation.status === 'ready' ? preparation : null;
  const renderIdentity = readyPreparation?.mountIntent
    ? frontstageNativeInstanceRenderKey(readyPreparation.mountIntent)
    : null;
  const instanceKey = `${renderIdentity}:${preparation.generation}:${retryGeneration}`;
  const isInitialContentCommitted = Boolean(
    readyPreparation?.mountIntent && committedInstanceKey === instanceKey
  );
  const handleInitialContentReady = useCallback(() => {
    setCommittedInstanceKey(instanceKey);
  }, [instanceKey]);
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
          ...(isInitialContentCommitted
            ? { height: fillsAvailableHeight ? '100%' : undefined }
            : { height: 0, overflow: 'hidden' })
        }}
      />
      {root ? (
        <FrontstageNativeRuntimeInstance
          key={instanceKey}
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
          onInitialContentReady={handleInitialContentReady}
          onLayoutPhaseChange={onLayoutPhaseChange}
          onRuntimeError={setRuntimeError}
        />
      ) : null}
      {!root || !isInitialContentCommitted ? <BlockUiLoadingShell /> : null}
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
  onInitialContentReady,
  onLayoutPhaseChange,
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
  onInitialContentReady(): void;
  onLayoutPhaseChange?: (phase: FrontstageNativeSlotLayoutPhase) => void;
  onRuntimeError(error: BlockProtocolError): void;
}) {
  const contentCommittedRef = useRef(false);
  const initialWindowClosedRef = useRef(false);
  const pendingInitialApiCallsRef = useRef(new Set<string>());
  const commitCheckTimerRef = useRef<ReturnType<typeof setTimeout> | null>(
    null
  );
  const settleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const safetyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const onInitialContentReadyRef = useRef(onInitialContentReady);
  const onLayoutPhaseChangeRef = useRef(onLayoutPhaseChange);
  useLayoutEffect(() => {
    onInitialContentReadyRef.current = onInitialContentReady;
    onLayoutPhaseChangeRef.current = onLayoutPhaseChange;
  }, [onInitialContentReady, onLayoutPhaseChange]);
  useEffect(
    () => () => {
      if (commitCheckTimerRef.current !== null) {
        clearTimeout(commitCheckTimerRef.current);
      }
      if (settleTimerRef.current !== null) {
        clearTimeout(settleTimerRef.current);
      }
      if (safetyTimerRef.current !== null) {
        clearTimeout(safetyTimerRef.current);
      }
    },
    []
  );
  const finalizeInitialWindow = useCallback(() => {
    if (initialWindowClosedRef.current) return;
    initialWindowClosedRef.current = true;
    pendingInitialApiCallsRef.current.clear();
    if (commitCheckTimerRef.current !== null) {
      clearTimeout(commitCheckTimerRef.current);
      commitCheckTimerRef.current = null;
    }
    if (settleTimerRef.current !== null) {
      clearTimeout(settleTimerRef.current);
      settleTimerRef.current = null;
    }
    if (safetyTimerRef.current !== null) {
      clearTimeout(safetyTimerRef.current);
      safetyTimerRef.current = null;
    }
    onLayoutPhaseChangeRef.current?.('settled');
  }, []);
  const handleInitialContentCommit = useCallback(() => {
    if (contentCommittedRef.current) return;
    contentCommittedRef.current = true;
    onInitialContentReadyRef.current();
    commitCheckTimerRef.current = setTimeout(() => {
      commitCheckTimerRef.current = null;
      if (initialWindowClosedRef.current) return;
      if (pendingInitialApiCallsRef.current.size > 0) {
        onLayoutPhaseChangeRef.current?.('initial_api_pending');
        safetyTimerRef.current = setTimeout(
          finalizeInitialWindow,
          INITIAL_API_RATCHET_SAFETY_TIMEOUT_MS
        );
      } else {
        finalizeInitialWindow();
      }
    }, 0);
  }, [finalizeInitialWindow]);
  const observedHost = useMemo<FrontstageNativeBlockContextHost | undefined>(
    () => {
      if (!nativeContextHost) return undefined;
      return {
        ...nativeContextHost,
        observeApiCall: (observation) => {
          nativeContextHost.observeApiCall?.(observation);
          if (initialWindowClosedRef.current) return;
          if (observation.status === 'pending') {
            pendingInitialApiCallsRef.current.add(observation.callId);
            if (settleTimerRef.current !== null) {
              clearTimeout(settleTimerRef.current);
              settleTimerRef.current = null;
            }
            return;
          }
          pendingInitialApiCallsRef.current.delete(observation.callId);
          if (
            contentCommittedRef.current &&
            pendingInitialApiCallsRef.current.size === 0 &&
            settleTimerRef.current === null
          ) {
            settleTimerRef.current = setTimeout(() => {
              settleTimerRef.current = null;
              if (pendingInitialApiCallsRef.current.size === 0) {
                finalizeInitialWindow();
              }
            }, 0);
          }
        }
      };
    },
    [finalizeInitialWindow, nativeContextHost]
  );
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
  const capabilities = observedHost
    ? createFrontstageNativeBlockContextCapabilities({
        host: observedHost,
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
      onInitialContentCommit={handleInitialContentCommit}
      onRuntimeError={onRuntimeError}
    />
  );
}
