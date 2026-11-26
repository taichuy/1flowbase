import { BlockUiLoadingShell } from '@1flowbase/block-renderer/loading-shell';
import {
  diagnoseLegacyBlockModuleSource,
  sha256Text,
  type NativeReactResolvedModuleAsset,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime/browser';
import type { BlockContextSeed } from '@1flowbase/page-protocol';
import Space from 'antd/es/space';
import { lazy, useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  type NativeReactBrowserCompilerWorkerFactory,
  compileNativeReactComponentInBrowser
} from '../../../shared/code-block/native-react-compiler-browser';
import {
  prepareNativeReactSource,
  type NativeReactModuleRegistryFactory
} from '../../../shared/code-block/native-react-source-preparation';
import {
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../frontstage/lib/native-adapter/native-trusted-block-react-adapter';
import { createFrontstageUnavailableBlockContext } from '../../frontstage/lib/native-trusted-block/unavailable-context';
import type { FrontstageBlockInstance } from '../../frontstage/lib/page-document';
import type { PasswordSignInResponse, PublicLoginEntry } from '../api/session';
import { LoadingState } from '../../../shared/ui/loading-state/LoadingState';
import { BuiltinPasswordSignIn } from './BuiltinPasswordSignIn';
import { defaultLoginImports } from '../lib/default-login-imports';
import { matchingDefaultLoginArtifact } from '../lib/default-login-artifact';
import { publicAuthCompilation } from '../lib/public-auth-compilation';
import {
  createPublicAuthInputs,
  createPublicAuthNativeBlockContextCapabilities,
  dispatchPublicAuthApi
} from './public-auth-block-host';

const PUBLIC_AUTH_PREPARATION_TIMEOUT_MS = 10_000;
type PublicAuthAttempt = 0 | 1;

export interface PublicAuthBlockProps {
  instance: PublicLoginEntry;
  loginEntrySelector?: { request: () => void } | null;
  onAuthenticated: (session: PasswordSignInResponse) => void | Promise<void>;
  invitationToken?: string;
  nativeCompiler?: typeof compileNativeReactComponentInBrowser;
  nativeCompilerWorkerFactory?: NativeReactBrowserCompilerWorkerFactory;
  nativeModuleRegistryFactory?: NativeReactModuleRegistryFactory;
}

interface ActivePublicAuthInstance {
  requestId: string;
}

type ReadyPublicAuthSnapshot = {
  component: FrontstageNativeTrustedBlockReactComponent;
  context: BlockContextSeed & { auth: { redirectToExternalLogin(): void } };
  moduleAssets: NativeReactResolvedModuleAsset[];
  moduleSources: string[];
  plan: NativeTrustedBlockPreparePlan;
  renderEpoch: string;
  attempt: PublicAuthAttempt;
};

type PublicAuthRenderSnapshot =
  | { status: 'discovering' | 'compiling' }
  | { status: 'failed'; diagnostic: 'compile_failed' | 'mount_failed' }
  | {
      status: 'mounting' | 'ready';
      value: ReadyPublicAuthSnapshot;
    };

// Resolve the exact module payload before mounting the configured runtime; a
// slow download is not a compiler failure. Unexpected imports use the full host.
const ShippedPasswordBlock = lazy(async () => {
  const registry = await import('../lib/default-login-registry').catch(
    () => null
  );
  return {
    default: registry
      ? (props: PublicAuthBlockProps) => (
          <PublicAuthBlockRuntime
            {...props}
            nativeModuleRegistryFactory={registry.createDefaultLoginRegistry}
          />
        )
      : PublicAuthBlockRuntime
  };
});

export function PublicAuthBlock(props: PublicAuthBlockProps) {
  const shipped = useMemo(
    () =>
      matchingDefaultLoginArtifact(
        props.instance.public_ui_block,
        defaultLoginImports
      ),
    [props.instance.public_ui_block]
  );
  return shipped &&
    !props.nativeModuleRegistryFactory &&
    !props.nativeCompiler &&
    !props.nativeCompilerWorkerFactory ? (
    <ShippedPasswordBlock {...props} />
  ) : (
    <PublicAuthBlockRuntime {...props} />
  );
}

function PublicAuthBlockRuntime({
  instance,
  loginEntrySelector = null,
  onAuthenticated,
  invitationToken,
  nativeCompiler = publicAuthCompilation.compile,
  nativeCompilerWorkerFactory,
  nativeModuleRegistryFactory
}: PublicAuthBlockProps) {
  const [renderRoot, setRenderRoot] = useState<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<PublicAuthRenderSnapshot>({
    status: 'discovering'
  });
  const generationRef = useRef(0);
  const activeInstanceRef = useRef<ActivePublicAuthInstance | null>(null);
  const block = useMemo(
    () => createPublicAuthNativeBlock(instance),
    [instance]
  );
  const escapeFallbackVisible = snapshot.status === 'failed';

  const markContentCommitted = useCallback((renderEpoch: string) => {
    setSnapshot((current) =>
      current.status === 'mounting' && current.value.renderEpoch === renderEpoch
        ? { status: 'ready', value: current.value }
        : current
    );
  }, []);

  const prepare = useCallback(
    async (initialAttempt: PublicAuthAttempt = 0) => {
      if (!renderRoot) return;
      const generation = generationRef.current + 1;
      generationRef.current = generation;
      setSnapshot({ status: 'compiling' });

      const failAttempt = (
        attempt: PublicAuthAttempt,
        activeInstance: ActivePublicAuthInstance
      ) => {
        if (
          generationRef.current !== generation ||
          activeInstanceRef.current !== activeInstance
        ) {
          return;
        }
        activeInstanceRef.current = null;
        if (attempt === 0) {
          void runAttempt(1);
        } else {
          setSnapshot({ status: 'failed', diagnostic: 'compile_failed' });
        }
      };

      async function runAttempt(attempt: PublicAuthAttempt): Promise<void> {
        const requestId = `public-auth:${instance.id}:${generation}:${attempt}:${sha256Text(instance.public_ui_block)}`;
        const activeInstance: ActivePublicAuthInstance = { requestId };
        activeInstanceRef.current = activeInstance;

        if (diagnoseLegacyBlockModuleSource(instance.public_ui_block)) {
          failAttempt(attempt, activeInstance);
          return;
        }

        const preparationTimeout = window.setTimeout(() => {
          failAttempt(attempt, activeInstance);
        }, PUBLIC_AUTH_PREPARATION_TIMEOUT_MS);

        try {
          const prepared = await prepareNativeReactSource({
            frozenSource: instance.public_ui_block,
            requestId,
            compiler: nativeCompiler,
            ...(nativeCompilerWorkerFactory
              ? { workerFactory: nativeCompilerWorkerFactory }
              : {}),
            registryFactory:
              nativeModuleRegistryFactory ??
              (await import('../../frontstage/lib/native-modules/registry'))
                .createFrontstageNativeReactModuleRegistry
          });
          if (
            generationRef.current !== generation ||
            activeInstanceRef.current !== activeInstance
          ) {
            return;
          }
          if (!prepared.ok) {
            failAttempt(attempt, activeInstance);
            return;
          }

          const plan = createPublicAuthPlan(block, instance.public_ui_block);
          const renderEpoch = `${requestId}:epoch`;
          const unavailable = createFrontstageUnavailableBlockContext(plan);
          const capabilities = createPublicAuthNativeBlockContextCapabilities({
            loginEntryId: instance.id,
            invitationToken,
            requestId,
            instanceEpoch: renderEpoch,
            isCurrentInstance: () =>
              activeInstanceRef.current === activeInstance,
            outputs: unavailable.outputs,
            emitEvent: ({ name }) => {
              if (
                name === 'authenticator_selector_requested' &&
                loginEntrySelector
              ) {
                loginEntrySelector.request();
              }
            },
            interfaceHandler: async (effect) => {
              const response = await dispatchPublicAuthApi(
                effect.method,
                effect.path,
                effect.request,
                invitationToken
              );
              if (
                isAuthenticationCompletionPath(effect.path) &&
                isPublicAuthSession(response)
              ) {
                await onAuthenticated(response);
              }
              return response;
            }
          });
          setSnapshot({
            status: 'mounting',
            value: {
              component:
                prepared.component as FrontstageNativeTrustedBlockReactComponent,
              context: {
                ...unavailable,
                workspace: { id: 'public-auth' },
                application: null,
                inputs: createPublicAuthInputs(
                  instance.id,
                  instance.public_variables,
                  Boolean(loginEntrySelector),
                  invitationToken
                ),
                ...capabilities
              },
              moduleAssets: prepared.moduleAssets,
              moduleSources: prepared.artifact.program.injectedModules.map(
                ({ source }) => source
              ),
              plan,
              renderEpoch,
              attempt
            }
          });
        } catch {
          failAttempt(attempt, activeInstance);
        } finally {
          window.clearTimeout(preparationTimeout);
        }
      }

      await runAttempt(initialAttempt);
    },
    [
      block,
      invitationToken,
      loginEntrySelector,
      instance,
      nativeCompiler,
      nativeCompilerWorkerFactory,
      nativeModuleRegistryFactory,
      onAuthenticated,
      renderRoot
    ]
  );

  useEffect(() => {
    void prepare();
    return () => {
      generationRef.current += 1;
      activeInstanceRef.current = null;
    };
  }, [prepare]);

  return (
    <Space orientation="vertical" size="small" style={{ width: '100%' }}>
      <div
        ref={setRenderRoot}
        data-public-auth-diagnostic={
          snapshot.status === 'failed' ? snapshot.diagnostic : undefined
        }
        data-public-auth-phase={snapshot.status}
        data-testid="native-react-public-auth-root"
        style={{ width: '100%' }}
      />
      {(snapshot.status === 'mounting' || snapshot.status === 'ready') &&
      renderRoot &&
      !escapeFallbackVisible ? (
        <FrontstageNativeTrustedBlockPortalHost
          root={renderRoot}
          renderEpoch={snapshot.value.renderEpoch}
          plan={snapshot.value.plan}
          component={snapshot.value.component}
          ctx={snapshot.value.context}
          moduleAssets={snapshot.value.moduleAssets}
          moduleSources={snapshot.value.moduleSources}
          onInitialContentCommit={
            snapshot.status === 'mounting' ? markContentCommitted : undefined
          }
          onRuntimeError={() => {
            activeInstanceRef.current = null;
            if (snapshot.value.attempt === 0) {
              void prepare(1);
            } else {
              setSnapshot({ status: 'failed', diagnostic: 'mount_failed' });
            }
          }}
        />
      ) : null}
      {(snapshot.status === 'discovering' || snapshot.status === 'compiling') &&
      !escapeFallbackVisible ? (
        <BlockUiLoadingShell />
      ) : null}
      {snapshot.status === 'mounting' ? <LoadingState compact /> : null}
      {escapeFallbackVisible ? (
        <BuiltinPasswordSignIn
          loginEntryId={instance.id}
          loginEntrySelector={loginEntrySelector}
          onAuthenticated={onAuthenticated}
          invitationToken={invitationToken}
        />
      ) : null}
    </Space>
  );
}

function createPublicAuthPlan(
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

function createPublicAuthNativeBlock(
  instance: PublicLoginEntry
): FrontstageBlockInstance {
  return {
    id: `public-auth:${instance.id}`,
    rendererVersion: 'v1',
    sourceId: `public-auth:${instance.id}`,
    codeRef: `public-auth:${instance.id}`,
    sourceCodeRef: `public-auth:${instance.id}`,
    catalog: {
      providerCode: '1flowbase',
      installationId: 'builtin-installation'
    },
    contribution: {
      pluginId: 'builtin-auth',
      pluginVersion: '1.0.0',
      code: 'public-auth'
    },
    props: {},
    presentation: { heightMode: 'auto', height: null },
    layout: { order: 0 },
    order: 0,
    runtime: {
      kind: 'native_trusted_block',
      entry: 'default',
      hint: 'native_trusted_block'
    }
  };
}

function isAuthenticationCompletionPath(path: string): boolean {
  return (
    path === '/api/public/auth/sign-in' || path === '/api/public/auth/sign-up'
  );
}

function isPublicAuthSession(value: unknown): value is PasswordSignInResponse {
  return (
    isRecord(value) &&
    typeof value.csrf_token === 'string' &&
    typeof value.effective_display_role === 'string' &&
    typeof value.current_workspace_id === 'string'
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
