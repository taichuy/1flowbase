import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import {
  createNativeTrustedBlockHost,
  evaluateNativeReactComponentArtifactWithRegistry,
  sha256Text,
  diagnoseLegacyBlockModuleSource,
  type NativeReactCompileDiagnostic,
  type NativeReactRuntimeDiagnostic,
  type NativeReactCatalogDependencyLock,
  type NativeTrustedBlockHost,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { BlockContext } from '@1flowbase/page-protocol';
import { Alert, Button, Modal, Space } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';

import { i18nText } from '../../../shared/i18n/text';
import {
  compileNativeReactComponentInBrowser,
  type NativeReactBrowserCompilerWorkerFactory
} from '../../../shared/code-block/native-react-compiler-browser';
import type { NormalizedFrontstageBlockCatalogEntry } from '../lib/block-catalog';
import {
  createFrontstageUnavailableBlockContext,
  createFrontstageNativeTrustedBlockReactAdapter,
  type FrontstageNativeTrustedBlockReactComponent
} from '../lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import type { FrontstageBlockInstance } from '../lib/page-document';

type NativeTrialDiagnostic =
  | NativeReactCompileDiagnostic
  | NativeReactRuntimeDiagnostic;
const EMPTY_NATIVE_REACT_DEPENDENCY_LOCK: NativeReactCatalogDependencyLock = [];

interface NativeTrialSnapshot {
  status: 'compiling' | 'ready' | 'failed';
  requestId: string;
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

export interface NativeTrialBlockContextInput {
  requestId: string;
  instanceEpoch: string;
  plan: NativeTrustedBlockPreparePlan;
  isCurrentInstance(): boolean;
}

export interface JsBlockTrialPanelProps {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  code: string;
  onPrepareDraftRun?: (input: {
    blockId: string;
    runId: string;
    draftHash: string;
    confirmWrite: () => Promise<boolean>;
  }) => Promise<void>;
  onRevokeDraftRun?: (runId: string) => void;
  revision: string;
  nativeCompiler?: typeof compileNativeReactComponentInBrowser;
  nativeCompilerWorkerFactory?: NativeReactBrowserCompilerWorkerFactory;
  nativeDependencyLock?: NativeReactCatalogDependencyLock;
  nativeDependencyLockError?: string | null;
  nativeModuleRegistryFactory?: typeof createFrontstageNativeReactModuleRegistry;
  createBlockContext?(input: NativeTrialBlockContextInput): BlockContext;
}

export function JsBlockTrialPanel({
  block,
  code,
  revision,
  nativeCompiler = compileNativeReactComponentInBrowser,
  nativeCompilerWorkerFactory,
  nativeDependencyLock = EMPTY_NATIVE_REACT_DEPENDENCY_LOCK,
  nativeDependencyLockError = null,
  nativeModuleRegistryFactory = createFrontstageNativeReactModuleRegistry,
  createBlockContext,
  onPrepareDraftRun,
  onRevokeDraftRun
}: JsBlockTrialPanelProps) {
  const previewRootRef = useRef<HTMLDivElement | null>(null);
  const generationRef = useRef(0);
  const activeRunRef = useRef<ActiveNativeTrialRun | null>(null);
  const resolvedComponentRef =
    useRef<FrontstageNativeTrustedBlockReactComponent | null>(null);
  const runtimeErrorHandlerRef = useRef<
    ((error: NativeReactRuntimeDiagnostic) => void) | null
  >(null);
  const blockContextRef = useRef<BlockContext | null>(null);
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
      resolveBlockContext: (context) =>
        blockContextRef.current ??
        createFrontstageUnavailableBlockContext(context.plan),
      onRuntimeError(error) {
        runtimeErrorHandlerRef.current?.({ phase: 'runtime', ...error });
      }
    });
  }
  const latestDraftRef = useRef({ block, code, revision });
  latestDraftRef.current = { block, code, revision };
  const createBlockContextRef = useRef(createBlockContext);
  createBlockContextRef.current = createBlockContext;
  const onPrepareDraftRunRef = useRef(onPrepareDraftRun);
  onPrepareDraftRunRef.current = onPrepareDraftRun;
  const onRevokeDraftRunRef = useRef(onRevokeDraftRun);
  onRevokeDraftRunRef.current = onRevokeDraftRun;
  const [snapshot, setSnapshot] = useState<NativeTrialSnapshot | null>(null);

  const disposeActiveRun = useCallback(() => {
    const active = activeRunRef.current;
    activeRunRef.current = null;
    blockContextRef.current = null;
    if (active) onRevokeDraftRunRef.current?.(active.requestId);
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

      const requestId = onPrepareDraftRunRef.current
        ? `draft:${frozenBlock.id}:${generation}`
        : `native:${frozenBlock.id}:${frozenRevision}`;
      const instanceEpoch = `${requestId}:epoch`;
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
        diagnostics: []
      });

      const legacyDiagnostic = diagnoseLegacyBlockModuleSource(frozenSource);
      if (legacyDiagnostic) {
        setSnapshot({
          status: 'failed',
          requestId,
          diagnostics: [legacyDiagnostic]
        });
        return;
      }

      try {
        await onPrepareDraftRunRef.current?.({
          blockId: frozenBlock.id,
          runId: requestId,
          draftHash: sha256Text(frozenSource),
          confirmWrite: confirmWriteRun
        });
      } catch (error) {
        if (generationRef.current !== generation) return;
        setSnapshot({
          status: 'failed',
          requestId,
          diagnostics: [
            {
              phase: 'runtime',
              code: 'runtime_error',
              path: 'runtime.authorization',
              message: getErrorMessage(error)
            }
          ]
        });
        return;
      }

      if (nativeDependencyLockError) {
        setSnapshot({
          status: 'failed',
          requestId,
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
          diagnostics: [error]
        });
      };
      const plan = createNativeTrialPlan(frozenBlock, frozenSource);
      blockContextRef.current =
        createBlockContextRef.current?.({
          requestId,
          instanceEpoch,
          plan,
          isCurrentInstance: () => activeRunRef.current === activeRun
        }) ?? createFrontstageUnavailableBlockContext(plan);
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
      setSnapshot({ status: 'ready', requestId, diagnostics: [] });
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
            diagnostics: []
          }
    );
  }, [runFrozenRevision]);

  const failed = snapshot?.status === 'failed';
  const preview = (
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
          description={snapshot?.diagnostics[0]?.message}
          action={
            <Button size="small" onClick={() => void retry()}>
              {i18nText('frontstage', 'auto.retry')}
            </Button>
          }
        />
      ) : null}
    </Space>
  );
  return preview;
}

function confirmWriteRun(): Promise<boolean> {
  return new Promise((resolve) => {
    Modal.confirm({
      title: i18nText('frontstage', 'auto.confirm_write_run'),
      content: i18nText('frontstage', 'auto.confirm_write_run_description'),
      onOk: () => resolve(true),
      onCancel: () => resolve(false)
    });
  });
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : 'Native React preview authorization failed.';
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
