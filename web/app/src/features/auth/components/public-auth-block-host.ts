import { apiFetch } from '@1flowbase/api-client';
import type {
  BlockHostEffectHandler,
  BlockHostInterfaceEffect,
  NativeBlockContextApiCallObservation,
  NativeBlockContextCapabilityDiagnostic,
  NativeBlockContextEventInput
} from '@1flowbase/page-runtime';
import { createNativeBlockContextCapabilities } from '@1flowbase/page-runtime';
import type { BlockContextOutputs } from '@1flowbase/page-protocol';

import { getAuthApiBaseUrl } from '../api/session';

export function createPublicAuthInputs(
  authenticatorId: string,
  publicVariables: Record<string, unknown>,
  authenticatorSelectionAvailable = false
): Record<string, unknown> {
  return {
    authenticator_id: authenticatorId,
    authenticator_selection_available: authenticatorSelectionAvailable,
    public_variables: publicVariables
  };
}

export interface PublicAuthPreviewCapabilityHandlers {
  interface: BlockHostEffectHandler<BlockHostInterfaceEffect>;
  prepareDraftRun(input: { runId: string }): Promise<void>;
  revokeDraftRun(runId: string): void;
}

export function createPublicAuthNativeBlockContextCapabilities(input: {
  requestId: string;
  instanceEpoch: string;
  isCurrentInstance(): boolean;
  outputs: BlockContextOutputs;
  interfaceHandler?: BlockHostEffectHandler<BlockHostInterfaceEffect>;
  emitEvent?(event: NativeBlockContextEventInput): void;
  observeApiCall?(observation: NativeBlockContextApiCallObservation): void;
  reportDiagnostic?(diagnostic: NativeBlockContextCapabilityDiagnostic): void;
}) {
  return createNativeBlockContextCapabilities({
    requestId: input.requestId,
    instanceEpoch: input.instanceEpoch,
    isCurrentInstance: input.isCurrentInstance,
    outputs: input.outputs,
    interfaceHandler:
      input.interfaceHandler ??
      ((effect) =>
        dispatchPublicAuthApi(effect.method, effect.path, effect.request)),
    emitEvent: input.emitEvent,
    observeApiCall: input.observeApiCall,
    reportDiagnostic: input.reportDiagnostic
  });
}

export function createPublicAuthPreviewCapabilityHandlers(): PublicAuthPreviewCapabilityHandlers {
  const draftRuns = new Set<string>();

  return {
    async prepareDraftRun({ runId }) {
      draftRuns.add(runId);
    },
    revokeDraftRun(runId) {
      draftRuns.delete(runId);
    },
    async interface(effect) {
      if (!draftRuns.has(effect.requestId)) {
        throw new Error('Public authentication preview run is not registered.');
      }
      if (effect.operation && effect.operation !== 'call') {
        throw new Error(
          'Public authentication preview streaming is not supported.'
        );
      }
      return dispatchPublicAuthApi(effect.method, effect.path, effect.request);
    }
  };
}

export async function dispatchPublicAuthApi(
  method: string,
  path: string,
  request: unknown
): Promise<unknown> {
  const normalizedUrl = new URL(path, 'http://public-auth.local');
  if (
    normalizedUrl.origin !== 'http://public-auth.local' ||
    !normalizedUrl.pathname.startsWith('/api/public/')
  ) {
    throw new Error(
      'Public authentication Block requested a forbidden API path.'
    );
  }
  const options = isRecord(request) ? request : {};
  const query = isRecord(options.query) ? options.query : undefined;
  const queryString = query
    ? new URLSearchParams(toStringRecord(query)).toString()
    : '';
  return apiFetch({
    path: `${normalizedUrl.pathname}${queryString ? `?${queryString}` : normalizedUrl.search}`,
    method,
    body: options.body,
    headers: isRecord(options.headers)
      ? toStringRecord(options.headers)
      : undefined,
    baseUrl: getAuthApiBaseUrl()
  });
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function toStringRecord(
  value: Record<string, unknown>
): Record<string, string> {
  return Object.fromEntries(
    Object.entries(value).map(([key, item]) => [key, String(item)])
  );
}
