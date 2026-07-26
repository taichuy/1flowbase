import {
  BlockUiLoadingShell,
  type BlockRendererActionEvent
} from '@1flowbase/block-renderer';
import {
  createNativeTrustedBlockHost,
  evaluateNativeReactComponentArtifactWithRegistry,
  type JsBlockHostEffectHandlers,
  type NativeReactCompileDiagnostic,
  type NativeReactRuntimeDiagnostic,
  type NativeReactCatalogDependencyLock,
  type NativeTrustedBlockHost,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import { Alert, Button, Space } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import {
  compileNativeReactComponentInBrowser,
  type NativeReactBrowserCompilerWorkerFactory
} from '../../../shared/code-block/native-react-compiler-browser';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import {
  createFrontstageNativeTrustedBlockReactAdapter,
  type FrontstageNativeTrustedBlockReactComponent
} from '../lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import type { FrontstageBlockInstance } from '../lib/page-document';
import type { RestrictedBlockLoaderLimits } from '../lib/restricted-block-loader';
import type { createFrontstageRestrictedBlockRuntimeSession } from '../lib/frontstage-restricted-block-runtime-host';
import { JsBlockPreviewConsole } from './JsBlockPreviewConsole';

type NativeTrialDiagnostic =
  | NativeReactCompileDiagnostic
  | NativeReactRuntimeDiagnostic;
const EMPTY_NATIVE_REACT_DEPENDENCY_LOCK: NativeReactCatalogDependencyLock = [];

interface NativeTrialSnapshot {
  status: 'compiling' | 'ready' | 'failed';
  requestId: string;
  logs: [];
  diagnostics: NativeTrialDiagnostic[];
}

interface ActiveNativeTrialRun {
  block: FrontstageBlockInstance;
  source: string;
  revision: string;
  requestId: string;
  root: HTMLElement;
  host?: NativeTrustedBlockHost;
  runtimeFailed?: boolean;
}

export interface JsBlockTrialPanelProps {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  code: string;
  contextSnapshot: Record<string, unknown>;
  createRunInputs?: (
    event?: BlockRendererActionEvent
  ) => Record<string, unknown>;
  handlers?: JsBlockHostEffectHandlers;
  onPrepareDraftRun?: (input: {
    blockId: string;
    runId: string;
    draftHash: string;
    confirmWrite: () => Promise<boolean>;
  }) => Promise<void>;
  onRevokeDraftRun?: (runId: string) => void;
  limits: RestrictedBlockLoaderLimits;
  revision: string;
  runtimeSessionFactory?: typeof createFrontstageRestrictedBlockRuntimeSession;
  nativeCompiler?: typeof compileNativeReactComponentInBrowser;
  nativeCompilerWorkerFactory?: NativeReactBrowserCompilerWorkerFactory;
  nativeDependencyLock?: NativeReactCatalogDependencyLock;
  nativeDependencyLockError?: string | null;
  nativeModuleRegistryFactory?: typeof createFrontstageNativeReactModuleRegistry;
}

export function JsBlockTrialPanel({
  block,
  code,
  revision,
  nativeCompiler = compileNativeReactComponentInBrowser,
  nativeCompilerWorkerFactory,
  nativeDependencyLock = EMPTY_NATIVE_REACT_DEPENDENCY_LOCK,
  nativeDependencyLockError = null,
  nativeModuleRegistryFactory = createFrontstageNativeReactModuleRegistry
}: JsBlockTrialPanelProps) {
  const previewRootRef = useRef<HTMLDivElement | null>(null);
  const generationRef = useRef(0);
  const activeRunRef = useRef<ActiveNativeTrialRun | null>(null);
  const resolvedComponentRef =
    useRef<FrontstageNativeTrustedBlockReactComponent | null>(null);
  const runtimeErrorHandlerRef = useRef<
    ((error: NativeReactRuntimeDiagnostic) => void) | null
  >(null);
  const adapterRef = useRef<ReturnType<
    typeof createFrontstageNativeTrustedBlockReactAdapter
  > | null>(null);
  if (!adapterRef.current) {
    adapterRef.current = createFrontstageNativeTrustedBlockReactAdapter({
      resolveComponent: () => {
        if (!resolvedComponentRef.current) {
          throw new Error('Native React trial component is unavailable.');
        }
        return resolvedComponentRef.current;
      },
      onRuntimeError(error) {
        runtimeErrorHandlerRef.current?.({ phase: 'runtime', ...error });
      }
    });
  }
  const latestDraftRef = useRef({ block, code, revision });
  latestDraftRef.current = { block, code, revision };
  const [snapshot, setSnapshot] = useState<NativeTrialSnapshot | null>(null);

  const disposeActiveRun = useCallback(() => {
    const active = activeRunRef.current;
    activeRunRef.current = null;
    return active?.host?.dispose() ?? Promise.resolve();
  }, []);

  const runFrozenRevision = useCallback(
    async ({
      frozenBlock,
      frozenSource,
      frozenRevision,
      root
    }: {
      frozenBlock: FrontstageBlockInstance;
      frozenSource: string;
      frozenRevision: string;
      root: HTMLElement;
    }) => {
      const generation = generationRef.current + 1;
      generationRef.current = generation;
      await disposeActiveRun();
      if (generationRef.current !== generation) return;

      const requestId = `native:${frozenBlock.id}:${frozenRevision}`;
      const activeRun: ActiveNativeTrialRun = {
        block: frozenBlock,
        source: frozenSource,
        revision: frozenRevision,
        requestId,
        root
      };
      activeRunRef.current = activeRun;
      setSnapshot({
        status: 'compiling',
        requestId,
        logs: [],
        diagnostics: []
      });

      if (nativeDependencyLockError) {
        setSnapshot({
          status: 'failed',
          requestId,
          logs: [],
          diagnostics: [
            {
              phase: 'compile',
              code: 'import_denied',
              path: 'catalog.code_modules',
              message: nativeDependencyLockError
            }
          ]
        });
        return;
      }

      const compiled = await nativeCompiler({
        source: frozenSource,
        requestId,
        ...(nativeCompilerWorkerFactory
          ? { workerFactory: nativeCompilerWorkerFactory }
          : {}),
        dependencyLock: nativeDependencyLock
      });
      if (generationRef.current !== generation) return;
      if (!compiled.ok) {
        setSnapshot({
          status: 'failed',
          requestId,
          logs: [],
          diagnostics: compiled.diagnostics
        });
        return;
      }

      const evaluated = await evaluateNativeReactComponentArtifactWithRegistry(
        compiled.artifact,
        nativeModuleRegistryFactory(compiled.artifact.dependencyLock)
      );
      if (generationRef.current !== generation) return;
      if (!evaluated.ok) {
        setSnapshot({
          status: 'failed',
          requestId,
          logs: [],
          diagnostics: evaluated.diagnostics
        });
        return;
      }

      resolvedComponentRef.current =
        evaluated.component as FrontstageNativeTrustedBlockReactComponent;
      runtimeErrorHandlerRef.current = (error) => {
        if (generationRef.current !== generation) return;
        activeRun.runtimeFailed = true;
        setSnapshot({
          status: 'failed',
          requestId,
          logs: [],
          diagnostics: [error]
        });
      };
      const plan = createNativeTrialPlan(frozenBlock, frozenSource);
      const host = createNativeTrustedBlockHost({
        adapter: adapterRef.current!
      });
      activeRun.host = host;
      const hostState = await host.mount(plan, root);
      if (generationRef.current !== generation) return;
      if (activeRun.runtimeFailed) return;
      if (hostState.status === 'failed') {
        setSnapshot({
          status: 'failed',
          requestId,
          logs: [],
          diagnostics: [
            {
              phase: 'runtime',
              ...(hostState.error ?? {
                code: 'runtime_error',
                path: 'runtime.mount',
                message: 'Native React preview mount failed.'
              })
            }
          ]
        });
        return;
      }
      setSnapshot({ status: 'ready', requestId, logs: [], diagnostics: [] });
    },
    [
      disposeActiveRun,
      nativeCompiler,
      nativeCompilerWorkerFactory,
      nativeDependencyLock,
      nativeDependencyLockError,
      nativeModuleRegistryFactory
    ]
  );

  useEffect(() => {
    const root = previewRootRef.current;
    if (!root) return;
    const frozen = latestDraftRef.current;
    void runFrozenRevision({
      frozenBlock: frozen.block,
      frozenSource: frozen.code,
      frozenRevision: frozen.revision,
      root
    });
  }, [revision, runFrozenRevision]);

  useEffect(
    () => () => {
      generationRef.current += 1;
      void disposeActiveRun();
    },
    [disposeActiveRun]
  );

  const retry = useCallback(async () => {
    const active = activeRunRef.current;
    if (!active) return;
    if (!active.host) {
      await runFrozenRevision({
        frozenBlock: active.block,
        frozenSource: active.source,
        frozenRevision: active.revision,
        root: active.root
      });
      return;
    }
    setSnapshot({
      status: 'compiling',
      requestId: active.requestId,
      logs: [],
      diagnostics: []
    });
    active.runtimeFailed = false;
    const hostState = await active.host.retry();
    if (activeRunRef.current !== active) return;
    if (active.runtimeFailed) return;
    setSnapshot(
      hostState.status === 'failed'
        ? {
            status: 'failed',
            requestId: active.requestId,
            logs: [],
            diagnostics: [
              {
                phase: 'runtime',
                ...(hostState.error ?? {
                  code: 'runtime_error',
                  path: 'runtime.retry',
                  message: 'Native React preview retry failed.'
                })
              }
            ]
          }
        : {
            status: 'ready',
            requestId: active.requestId,
            logs: [],
            diagnostics: []
          }
    );
  }, [runFrozenRevision]);

  const failed = snapshot?.status === 'failed';
  return (
    <JsBlockPreviewConsole
      snapshot={snapshot}
      preview={
        <Space direction="vertical" size="small" style={{ width: '100%' }}>
          <div
            ref={previewRootRef}
            data-testid="native-react-trial-root"
            style={{ width: '100%' }}
          />
          {snapshot?.status === 'compiling' ? <BlockUiLoadingShell /> : null}
          {failed ? (
            <Alert
              type="error"
              showIcon
              message={i18nText('frontstage', 'auto.run_failed')}
              action={
                <Button size="small" onClick={() => void retry()}>
                  {i18nText('frontstage', 'auto.retry')}
                </Button>
              }
            />
          ) : null}
        </Space>
      }
    />
  );
}

function createNativeTrialPlan(
  block: FrontstageBlockInstance,
  source: string
): NativeTrustedBlockPreparePlan {
  return {
    runtime: 'native_trusted_block',
    blockId: block.id,
    entry: block.runtime.entry ?? 'default',
    source,
    normalizedSource: source.trim(),
    props: { ...block.props },
    requiredPermissions: ['ui_block.javascript.native']
  };
}
