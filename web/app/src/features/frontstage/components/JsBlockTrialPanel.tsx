import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import {
  evaluateNativeReactComponentArtifactWithRegistry,
  sha256Text,
  diagnoseLegacyBlockModuleSource,
  type NativeReactCompileDiagnostic,
  type NativeReactRuntimeDiagnostic,
  type NativeReactCatalogDependencyLock,
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
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../lib/native-trusted-block-runtime-factory';
import type { FrontstageBlockInstance } from '../lib/page-document';

type NativeTrialDiagnostic =
  | NativeReactCompileDiagnostic
  | NativeReactRuntimeDiagnostic;
const EMPTY_NATIVE_REACT_DEPENDENCY_LOCK: NativeReactCatalogDependencyLock = [];

interface NativeTrialPendingSnapshot {
  status: 'compiling' | 'failed';
  requestId: string;
  diagnostics: NativeTrialDiagnostic[];
}

interface NativeTrialReadySnapshot {
  status: 'ready';
  requestId: string;
  diagnostics: [];
  component: FrontstageNativeTrustedBlockReactComponent;
  plan: NativeTrustedBlockPreparePlan;
  context: BlockContext;
  renderEpoch: string;
}

type NativeTrialSnapshot =
  | NativeTrialPendingSnapshot
  | NativeTrialReadySnapshot;

interface ActiveNativeTrialRun {
  block: FrontstageBlockInstance;
  source: string;
  revision: string;
  requestId: string;
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
  const [previewRoot, setPreviewRoot] = useState<HTMLDivElement | null>(null);
  const generationRef = useRef(0);
  const activeRunRef = useRef<ActiveNativeTrialRun | null>(null);
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
    if (active) onRevokeDraftRunRef.current?.(active.requestId);
  }, []);

  const runFrozenRevision = useCallback(
    async ({
      frozenBlock,
      frozenSource,
      frozenRevision
    }: {
      frozenBlock: FrontstageBlockInstance;
      frozenSource: string;
      frozenRevision: string;
    }) => {
      const generation = generationRef.current + 1;
      generationRef.current = generation;
      disposeActiveRun();
      if (generationRef.current !== generation) return;

      const requestId = onPrepareDraftRunRef.current
        ? `draft:${frozenBlock.id}:${generation}`
        : `native:${frozenBlock.id}:${frozenRevision}`;
      const instanceEpoch = `${requestId}:epoch`;
      const activeRun: ActiveNativeTrialRun = {
        block: frozenBlock,
        source: frozenSource,
        revision: frozenRevision,
        requestId
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
      if (generationRef.current !== generation) return;

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

      const plan = createNativeTrialPlan(frozenBlock, frozenSource);
      const context =
        createBlockContextRef.current?.({
          requestId,
          instanceEpoch,
          plan,
          isCurrentInstance: () => activeRunRef.current === activeRun
        }) ?? createFrontstageUnavailableBlockContext(plan);
      setSnapshot({
        status: 'ready',
        requestId,
        diagnostics: [],
        component:
          evaluated.component as FrontstageNativeTrustedBlockReactComponent,
        plan,
        context,
        renderEpoch: instanceEpoch
      });
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
    if (!previewRoot) return;
    const frozen = latestDraftRef.current;
    void runFrozenRevision({
      frozenBlock: frozen.block,
      frozenSource: frozen.code,
      frozenRevision: frozen.revision
    });
  }, [previewRoot, revision, runFrozenRevision]);

  useEffect(
    () => () => {
      generationRef.current += 1;
      disposeActiveRun();
    },
    [disposeActiveRun]
  );

  const retry = useCallback(() => {
    const active = activeRunRef.current;
    if (!active) return;
    void runFrozenRevision({
      frozenBlock: active.block,
      frozenSource: active.source,
      frozenRevision: active.revision
    });
  }, [runFrozenRevision]);

  const failed = snapshot?.status === 'failed';
  const preview = (
    <Space direction="vertical" size="small" style={{ width: '100%' }}>
      <div
        ref={setPreviewRoot}
        data-testid="native-react-trial-root"
        style={{ width: '100%' }}
      />
      {snapshot?.status === 'ready' && previewRoot ? (
        <FrontstageNativeTrustedBlockPortalHost
          root={previewRoot}
          renderEpoch={snapshot.renderEpoch}
          plan={snapshot.plan}
          component={snapshot.component}
          ctx={snapshot.context}
          onRuntimeError={(error) => {
            const active = activeRunRef.current;
            if (!active || active.requestId !== snapshot.requestId) return;
            setSnapshot({
              status: 'failed',
              requestId: snapshot.requestId,
              diagnostics: [{ phase: 'runtime', ...error }]
            });
          }}
        />
      ) : null}
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
