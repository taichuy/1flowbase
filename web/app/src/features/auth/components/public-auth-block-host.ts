import { apiFetch } from '@1flowbase/api-client';
import type { BlockRendererActionEvent } from '@1flowbase/block-renderer';
import type {
  JsBlockHostEffectHandler,
  JsBlockHostInterfaceEffect,
  JsBlockRunRequest
} from '@1flowbase/page-runtime';

import { getAuthApiBaseUrl, type PublicLoginInstance } from '../api/session';

export const PUBLIC_AUTH_RUNTIME_LIMITS = {
  timeoutMs: 10_000,
  maxRenderDepth: 32,
  maxRenderNodes: 500
} as const;

export function createPublicAuthInputs(
  authenticatorId: string,
  publicVariables: Record<string, unknown>,
  event?: BlockRendererActionEvent
): Record<string, unknown> {
  return {
    authenticator_id: authenticatorId,
    public_variables: publicVariables,
    ...(event
      ? {
          auth_event: {
            action_id: event.actionId,
            values: event.formValues ?? {},
            ...(event.payload === undefined ? {} : { payload: event.payload })
          }
        }
      : {})
  };
}

export function createPublicAuthRunRequest(
  instance: PublicLoginInstance,
  sequence: number,
  event?: BlockRendererActionEvent
): JsBlockRunRequest {
  return {
    requestId: `public-auth:${instance.id}:${sequence}`,
    blockId: `public-auth:${instance.id}`,
    program: {
      kind: 'source',
      source: instance.public_ui_block,
      allowedImports: [
        '@1flowbase/block-sdk',
        '@1flowbase/block-renderer/antd-facade'
      ]
    },
    inputs: createPublicAuthInputs(instance.id, instance.public_variables, event),
    props: {},
    state: {},
    contextSnapshot: {},
    limits: PUBLIC_AUTH_RUNTIME_LIMITS
  };
}

interface PublicAuthPreviewRunAuthorization {
  confirmWrite: () => Promise<boolean>;
  writeConfirmed: boolean;
  writeConfirmation: Promise<boolean> | null;
}

export interface PublicAuthPreviewCapabilityHandlers {
  interface: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
  prepareDraftRun(input: {
    runId: string;
    confirmWrite: () => Promise<boolean>;
  }): Promise<void>;
  revokeDraftRun(runId: string): void;
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
        throw new Error('Public authentication preview streaming is not supported.');
      }
      if (isPublicAuthWriteMethod(effect.method)) {
        const confirmed = await confirmPublicAuthPreviewWrite(draftRun);
        if (!confirmed) {
          throw new Error('Public authentication preview write was cancelled.');
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
    throw new Error('Public authentication Block requested a forbidden API path.');
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

function toStringRecord(value: Record<string, unknown>): Record<string, string> {
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
