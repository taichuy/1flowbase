import { BlockUiLoadingShell } from '@1flowbase/block-renderer';
import {
  diagnoseLegacyBlockModuleSource,
  sha256Text,
  type NativeReactResolvedModuleAsset,
  type NativeTrustedBlockPreparePlan
} from '@1flowbase/page-runtime';
import type { BlockContext } from '@1flowbase/page-protocol';
import { Alert, Button, Space } from 'antd';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

import {
  type NativeReactBrowserCompilerWorkerFactory,
  compileNativeReactComponentInBrowser
} from '../../../shared/code-block/native-react-compiler-browser';
import {
  prepareNativeReactSource,
  type NativeReactModuleRegistryFactory
} from '../../../shared/code-block/native-react-source-preparation';
import { i18nText } from '../../../shared/i18n/text';
import {
  createFrontstageUnavailableBlockContext,
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../frontstage/lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../../frontstage/lib/native-trusted-block-runtime-factory';
import type { FrontstageBlockInstance } from '../../frontstage/lib/page-document';
import type {
  PasswordSignInResponse,
  PublicLoginInstance
} from '../api/session';
import { BuiltinPasswordSignIn } from './BuiltinPasswordSignIn';
import {
  createPublicAuthInputs,
  createPublicAuthNativeBlockContextCapabilities,
  dispatchPublicAuthApi
} from './public-auth-block-host';

const PUBLIC_AUTH_PREPARATION_TIMEOUT_MS = 10_000;
type PublicAuthAttempt = 0 | 1;

export interface PublicAuthBlockProps {
  instance: PublicLoginInstance;
  authenticatorSelector?: { request: () => void } | null;
  onAuthenticated: (session: PasswordSignInResponse) => void | Promise<void>;
  nativeCompiler?: typeof compileNativeReactComponentInBrowser;
  nativeCompilerWorkerFactory?: NativeReactBrowserCompilerWorkerFactory;
  nativeModuleRegistryFactory?: NativeReactModuleRegistryFactory;
}

interface ActivePublicAuthInstance {
  requestId: string;
}

type PublicAuthRenderSnapshot =
  | { status: 'preparing' }
  | { status: 'failed' }
  | {
      status: 'ready';
      component: FrontstageNativeTrustedBlockReactComponent;
      context: BlockContext;
      moduleAssets: NativeReactResolvedModuleAsset[];
      plan: NativeTrustedBlockPreparePlan;
      renderEpoch: string;
      attempt: PublicAuthAttempt;
    };

export function PublicAuthBlock({
  instance,
  authenticatorSelector = null,
  onAuthenticated,
  nativeCompiler = compileNativeReactComponentInBrowser,
  nativeCompilerWorkerFactory,
  nativeModuleRegistryFactory = createFrontstageNativeReactModuleRegistry
}: PublicAuthBlockProps) {
  const [renderRoot, setRenderRoot] = useState<HTMLDivElement | null>(null);
  const [snapshot, setSnapshot] = useState<PublicAuthRenderSnapshot>({
    status: 'preparing'
  });
  const generationRef = useRef(0);
  const activeInstanceRef = useRef<ActivePublicAuthInstance | null>(null);
  const block = useMemo(
    () => createPublicAuthNativeBlock(instance),
    [instance]
  );
  const builtinPasswordFallbackEligible =
    instance.is_builtin && instance.auth_type === 'password-local';
  const builtinPasswordFallbackVisible =
    builtinPasswordFallbackEligible && snapshot.status === 'failed';

  const prepare = useCallback(
    async (initialAttempt: PublicAuthAttempt = 0) => {
      if (!renderRoot) return;
      const generation = generationRef.current + 1;
      generationRef.current = generation;
      setSnapshot({ status: 'preparing' });

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
        if (builtinPasswordFallbackEligible && attempt === 0) {
          void runAttempt(1);
        } else {
          setSnapshot({ status: 'failed' });
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
            dependencyLock: [],
            compiler: nativeCompiler,
            ...(nativeCompilerWorkerFactory
              ? { workerFactory: nativeCompilerWorkerFactory }
              : {}),
            registryFactory: nativeModuleRegistryFactory
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
            requestId,
            instanceEpoch: renderEpoch,
            isCurrentInstance: () =>
              activeInstanceRef.current === activeInstance,
            outputs: unavailable.outputs,
            emitEvent: ({ name }) => {
              if (
                name === 'authenticator_selector_requested' &&
                authenticatorSelector
              ) {
                authenticatorSelector.request();
              }
            },
            interfaceHandler: async (effect) => {
              const response = await dispatchPublicAuthApi(
                effect.method,
                effect.path,
                effect.request
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
            status: 'ready',
            component:
              prepared.component as FrontstageNativeTrustedBlockReactComponent,
            context: {
              ...unavailable,
              workspace: { id: 'public-auth' },
              application: null,
              inputs: createPublicAuthInputs(
                instance.id,
                instance.public_variables,
                Boolean(authenticatorSelector)
              ),
              ...capabilities
            },
            moduleAssets: prepared.moduleAssets,
            plan,
            renderEpoch,
            attempt
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
      authenticatorSelector,
      builtinPasswordFallbackEligible,
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
    <Space direction="vertical" size="small" style={{ width: '100%' }}>
      <div
        ref={setRenderRoot}
        data-testid="native-react-public-auth-root"
        style={{ width: '100%' }}
      />
      {snapshot.status === 'ready' &&
      renderRoot &&
      !builtinPasswordFallbackVisible ? (
        <FrontstageNativeTrustedBlockPortalHost
          root={renderRoot}
          renderEpoch={snapshot.renderEpoch}
          plan={snapshot.plan}
          component={snapshot.component}
          ctx={snapshot.context}
          moduleAssets={snapshot.moduleAssets}
          onRuntimeError={() => {
            activeInstanceRef.current = null;
            if (builtinPasswordFallbackEligible && snapshot.attempt === 0) {
              void prepare(1);
            } else {
              setSnapshot({ status: 'failed' });
            }
          }}
        />
      ) : null}
      {snapshot.status === 'preparing' && !builtinPasswordFallbackVisible ? (
        <BlockUiLoadingShell />
      ) : null}
      {builtinPasswordFallbackVisible ? (
        <BuiltinPasswordSignIn
          authenticatorId={instance.id}
          authenticatorSelector={authenticatorSelector}
          onAuthenticated={onAuthenticated}
        />
      ) : null}
      {snapshot.status === 'failed' && !builtinPasswordFallbackEligible ? (
        <Alert
          type="error"
          showIcon
          message={i18nText('auth', 'sign_in.login_instances_load_failed')}
          action={
            <Button size="small" onClick={() => void prepare()}>
              {i18nText('frontstage', 'auto.retry')}
            </Button>
          }
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
  instance: PublicLoginInstance
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
