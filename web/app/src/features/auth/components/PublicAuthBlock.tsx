import { useCallback, useMemo } from 'react';

import {
  JsBlockTrialPanel,
  type JsBlockTrialPanelProps,
  type NativeTrialBlockContextInput
} from '../../frontstage/components/JsBlockTrialPanel';
import { createFrontstageUnavailableBlockContext } from '../../frontstage/lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../../frontstage/lib/native-trusted-block-runtime-factory';
import type { FrontstageBlockInstance } from '../../frontstage/lib/page-document';
import type { PublicLoginInstance } from '../api/session';
import {
  createPublicAuthInputs,
  createPublicAuthNativeBlockContextCapabilities,
  dispatchPublicAuthApi
} from './public-auth-block-host';

export interface PublicAuthSession {
  csrf_token: string;
  effective_display_role: string;
  current_workspace_id: string;
}

export interface PublicAuthBlockProps {
  instance: PublicLoginInstance;
  onAuthenticated: (session: PublicAuthSession) => void | Promise<void>;
  nativeCompiler?: JsBlockTrialPanelProps['nativeCompiler'];
  nativeModuleRegistryFactory?: JsBlockTrialPanelProps['nativeModuleRegistryFactory'];
}

export function PublicAuthBlock({
  instance,
  onAuthenticated,
  nativeCompiler,
  nativeModuleRegistryFactory = createFrontstageNativeReactModuleRegistry
}: PublicAuthBlockProps) {
  const block = useMemo(
    () => createPublicAuthNativeBlock(instance),
    [instance]
  );
  const createBlockContext = useCallback(
    ({
      requestId,
      instanceEpoch,
      plan,
      isCurrentInstance
    }: NativeTrialBlockContextInput) => {
      const unavailable = createFrontstageUnavailableBlockContext(plan);
      const capabilities = createPublicAuthNativeBlockContextCapabilities({
        requestId,
        instanceEpoch,
        isCurrentInstance,
        outputs: unavailable.outputs,
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
      return {
        ...unavailable,
        workspace: { id: 'public-auth' },
        application: null,
        inputs: createPublicAuthInputs(instance.id, instance.public_variables),
        ...capabilities
      };
    },
    [instance.id, instance.public_variables, onAuthenticated]
  );

  return (
    <JsBlockTrialPanel
      block={block}
      catalogEntry={null}
      code={instance.public_ui_block}
      revision={`public-auth:${instance.id}`}
      createBlockContext={createBlockContext}
      nativeCompiler={nativeCompiler}
      nativeDependencyLock={[]}
      nativeModuleRegistryFactory={nativeModuleRegistryFactory}
    />
  );
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

function isPublicAuthSession(value: unknown): value is PublicAuthSession {
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
