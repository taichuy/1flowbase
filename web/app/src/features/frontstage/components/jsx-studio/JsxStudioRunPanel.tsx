import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import {
  sha256Text,
  diagnoseLegacyBlockModuleSource,
  type NativeReactRuntimeDiagnostic,
  type NativeReactCatalogDependencyLock,
  type NativeBlockContextApiCallObservation,
  type NativeReactResolvedModuleAsset,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { BlockContext } from '@1flowbase/page-protocol';
import { Alert, Button, Modal, Space } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  compileNativeReactComponentInBrowser,
  type NativeReactBrowserCompilerWorkerFactory
} from '../../../../shared/code-block/native-react-compiler-browser';
import {
  prepareNativeReactSource,
  type NativeReactModuleRegistryFactory,
  type NativeReactSourcePreparationDiagnostic
} from '../../../../shared/code-block/native-react-source-preparation';
import {
  createFrontstageUnavailableBlockContext,
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../../lib/native-trusted-block-runtime-factory';
import type { FrontstageBlockInstance } from '../../lib/page-document';
import { JsxStudioPreviewConsole } from './JsxStudioPreviewConsole';

type StudioRunDiagnostic =
  | NativeReactSourcePreparationDiagnostic
  | NativeReactRuntimeDiagnostic;
const EMPTY_NATIVE_REACT_DEPENDENCY_LOCK: NativeReactCatalogDependencyLock = [];

interface StudioRunPendingSnapshot {
  status: 'compiling' | 'failed';
  requestId: string;
  diagnostics: StudioRunDiagnostic[];
}

interface StudioRunReadySnapshot {
  status: 'ready';
  requestId: string;
  diagnostics: [];
  component: FrontstageNativeTrustedBlockReactComponent;
  plan: NativeTrustedBlockPreparePlan;
  context: BlockContext;
  renderEpoch: string;
  moduleAssets: NativeReactResolvedModuleAsset[];
}

type StudioRunSnapshot = StudioRunPendingSnapshot | StudioRunReadySnapshot;

interface ActiveStudioRun {
  block: FrontstageBlockInstance;
  source: string;
  revision: string;
  requestId: string;
}

export interface JsxStudioRunBlockContextInput {
  requestId: string;
  instanceEpoch: string;
  plan: NativeTrustedBlockPreparePlan;
  isCurrentInstance(): boolean;
  observeApiCall(observation: NativeBlockContextApiCallObservation): void;
}

export interface JsxStudioRunPanelProps {
  block: FrontstageBlockInstance;
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
  nativeModuleRegistryFactory?: NativeReactModuleRegistryFactory;
  createBlockContext?(input: JsxStudioRunBlockContextInput): BlockContext;
}

export function JsxStudioRunPanel({
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
}: JsxStudioRunPanelProps) {
  const [previewRoot, setPreviewRoot] = useState<HTMLDivElement | null>(null);
  const generationRef = useRef(0);
  const activeRunRef = useRef<ActiveStudioRun | null>(null);
  const latestDraftRef = useRef({ block, code, revision });
  latestDraftRef.current = { block, code, revision };
  const createBlockContextRef = useRef(createBlockContext);
  createBlockContextRef.current = createBlockContext;
  const onPrepareDraftRunRef = useRef(onPrepareDraftRun);
  onPrepareDraftRunRef.current = onPrepareDraftRun;
  const onRevokeDraftRunRef = useRef(onRevokeDraftRun);
  onRevokeDraftRunRef.current = onRevokeDraftRun;
  const [snapshot, setSnapshot] = useState<StudioRunSnapshot | null>(null);
  const [apiCalls, setApiCalls] = useState<
    NativeBlockContextApiCallObservation[]
  >([]);

  const observeApiCall = useCallback(
    (observation: NativeBlockContextApiCallObservation) => {
      if (activeRunRef.current?.requestId !== observation.requestId) return;
      setApiCalls((current) => [...current, observation]);
    },
    []
  );

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
      const activeRun: ActiveStudioRun = {
        block: frozenBlock,
        source: frozenSource,
        revision: frozenRevision,
        requestId
      };
      activeRunRef.current = activeRun;
      setApiCalls([]);
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

      const prepared = await prepareNativeReactSource({
        frozenSource,
        requestId,
        dependencyLock: nativeDependencyLock,
        compiler: nativeCompiler,
        ...(nativeCompilerWorkerFactory
          ? { workerFactory: nativeCompilerWorkerFactory }
          : {}),
        registryFactory: nativeModuleRegistryFactory
      });
      if (generationRef.current !== generation) return;
      if (!prepared.ok) {
        setSnapshot({
          status: 'failed',
          requestId,
          diagnostics: prepared.diagnostics
        });
        return;
      }

      const plan = createStudioRunPlan(frozenBlock, frozenSource);
      if (generationRef.current !== generation) return;
      const context =
        createBlockContextRef.current?.({
          requestId,
          instanceEpoch,
          plan,
          isCurrentInstance: () => activeRunRef.current === activeRun,
          observeApiCall
        }) ?? createFrontstageUnavailableBlockContext(plan);
      setSnapshot({
        status: 'ready',
        requestId,
        diagnostics: [],
        component:
          prepared.component as FrontstageNativeTrustedBlockReactComponent,
        plan,
        context,
        renderEpoch: instanceEpoch,
        moduleAssets: prepared.moduleAssets
      });
    },
    [
      disposeActiveRun,
      nativeCompiler,
      nativeCompilerWorkerFactory,
      nativeDependencyLock,
      nativeDependencyLockError,
      nativeModuleRegistryFactory,
      observeApiCall
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
        data-testid="native-react-studio-preview-root"
        style={{ width: '100%' }}
      />
      {snapshot?.status === 'ready' && previewRoot ? (
        <FrontstageNativeTrustedBlockPortalHost
          root={previewRoot}
          renderEpoch={snapshot.renderEpoch}
          plan={snapshot.plan}
          component={snapshot.component}
          ctx={snapshot.context}
          moduleAssets={snapshot.moduleAssets}
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
          action={
            <Button size="small" onClick={() => void retry()}>
              {i18nText('frontstage', 'auto.retry')}
            </Button>
          }
        />
      ) : null}
    </Space>
  );
  return (
    <JsxStudioPreviewConsole
      preview={preview}
      snapshot={{
        diagnostics: snapshot?.diagnostics ?? [],
        apiCalls
      }}
    />
  );
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

function createStudioRunPlan(
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
