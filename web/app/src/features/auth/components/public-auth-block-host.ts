import { apiFetch } from '@1flowbase/api-client';
import type { BlockRendererActionEvent } from '@1flowbase/block-renderer';
import type { JsBlockRunRequest } from '@1flowbase/page-runtime';

import { getAuthApiBaseUrl, type PublicLoginInstance } from '../api/session';

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
    inputs: {
      authenticator_id: instance.id,
      public_variables: instance.public_variables,
      ...(event
        ? {
            auth_event: {
              action_id: event.actionId,
              values: event.formValues ?? {},
              ...(event.payload === undefined ? {} : { payload: event.payload })
            }
          }
        : {})
    },
    props: {},
    state: {},
    contextSnapshot: {},
    limits: { timeoutMs: 10_000, maxRenderDepth: 32, maxRenderNodes: 500 }
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
    !normalizedUrl.pathname.startsWith('/api/public/auth/')
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
