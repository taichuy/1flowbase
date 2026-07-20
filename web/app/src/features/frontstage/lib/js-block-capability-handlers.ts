import {
  dispatchFrontstageCallable,
  getDefaultApiBaseUrl,
  type ApiBaseUrlLocation,
  type FrontstageCallableRequest
} from '@1flowbase/api-client';
import type {
  JsBlockHostEffectHandler,
  JsBlockHostInterfaceEffect
} from '@1flowbase/page-runtime';

export interface FrontstageJsBlockCapabilityClient {
  dispatchFrontstageCallable: typeof dispatchFrontstageCallable;
}

export interface CreateFrontstageJsBlockCapabilityHandlersOptions {
  workspaceId: string;
  pageId: string;
  tabId: string;
  csrfToken?: string | null;
  resolveOperationId(requestId: string, bindingAlias: string): string | null;
  baseUrl?: string;
  locationLike?: ApiBaseUrlLocation;
  client?: FrontstageJsBlockCapabilityClient;
}

const defaultClient: FrontstageJsBlockCapabilityClient = {
  dispatchFrontstageCallable
};
const draftRunWriteAuthorizations = new Map<string, Set<string>>();

export function authorizeFrontstageDraftRunWrites(
  runId: string,
  operationIds: readonly string[]
): void {
  draftRunWriteAuthorizations.set(runId, new Set(operationIds));
}

export function revokeFrontstageDraftRunWrites(runId: string): void {
  draftRunWriteAuthorizations.delete(runId);
}

export function getFrontstageJsBlockCapabilityApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined = typeof window !== 'undefined'
    ? window.location
    : undefined
): string {
  return (
    import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike)
  );
}

export function createFrontstageJsBlockCapabilityHandlers(
  options: CreateFrontstageJsBlockCapabilityHandlersOptions
): {
  interface: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
} {
  const client = options.client ?? defaultClient;
  const baseUrl =
    options.baseUrl ??
    getFrontstageJsBlockCapabilityApiBaseUrl(options.locationLike);

  return {
    interface: (effect) => {
      const operationId = options.resolveOperationId(
        effect.requestId,
        effect.bindingAlias
      );
      if (!operationId) {
        throw new Error(
          `Interface binding is not registered: ${effect.bindingAlias}.`
        );
      }
      return client.dispatchFrontstageCallable(
        options.workspaceId,
        options.pageId,
        options.tabId,
        {
          operation_id: operationId,
          ...(effect.request === undefined
            ? {}
            : { request: effect.request as FrontstageCallableRequest }),
          ...(draftRunWriteAuthorizations
            .get(effect.requestId)
            ?.has(operationId)
            ? {
                run_authorization: {
                  run_id: effect.requestId,
                  operation_id: operationId,
                  confirmed: true
                }
              }
            : {})
        },
        requireCsrfToken(options.csrfToken),
        baseUrl
      );
    }
  };
}

function requireCsrfToken(csrfToken: string | null | undefined): string {
  if (typeof csrfToken !== 'string' || csrfToken.length === 0) {
    throw new Error('JS Block callable interface requires csrfToken.');
  }
  return csrfToken;
}
