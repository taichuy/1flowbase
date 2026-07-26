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
  publicVariables: Record<string, unknown>
): Record<string, unknown> {
  return {
    authenticator_id: authenticatorId,
    public_variables: publicVariables
  };
}

interface PublicAuthPreviewRunAuthorization {
  confirmWrite: () => Promise<boolean>;
  writeConfirmed: boolean;
  writeConfirmation: Promise<boolean> | null;
}

export interface PublicAuthPreviewCapabilityHandlers {
  interface: BlockHostEffectHandler<BlockHostInterfaceEffect>;
  prepareDraftRun(input: {
    runId: string;
    confirmWrite: () => Promise<boolean>;
  }): Promise<void>;
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
  const draftRuns = new Map<string, PublicAuthPreviewRunAuthorization>();

  return {
    async prepareDraftRun({ runId, confirmWrite }) {
      draftRuns.set(runId, {
        confirmWrite,
        writeConfirmed: false,
        writeConfirmation: null
      });
    },
    revokeDraftRun(runId) {
      draftRuns.delete(runId);
    },
    async interface(effect) {
      const draftRun = draftRuns.get(effect.requestId);
      if (!draftRun) {
        throw new Error('Public authentication preview run is not registered.');
      }
      if (effect.operation && effect.operation !== 'call') {
        throw new Error(
          'Public authentication preview streaming is not supported.'
        );
      }
      if (isPublicAuthWriteMethod(effect.method)) {
        const confirmed = await confirmPublicAuthPreviewWrite(draftRun);
        if (!confirmed) {
          throw new Error('Public authentication preview write was cancelled.');
        }
        if (draftRuns.get(effect.requestId) !== draftRun) {
          throw new Error(
            'Public authentication preview run is not registered.'
          );
        }
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

function isPublicAuthWriteMethod(method: string): boolean {
  return !['GET', 'HEAD', 'OPTIONS'].includes(method.toUpperCase());
}

async function confirmPublicAuthPreviewWrite(
  draftRun: PublicAuthPreviewRunAuthorization
): Promise<boolean> {
  if (draftRun.writeConfirmed) return true;
  draftRun.writeConfirmation ??= draftRun.confirmWrite();
  const confirmed = await draftRun.writeConfirmation;
  draftRun.writeConfirmation = null;
  if (confirmed) draftRun.writeConfirmed = true;
  return confirmed;
}
