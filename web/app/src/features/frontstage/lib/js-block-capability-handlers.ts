import {
  dispatchFrontstageAction,
  dispatchFrontstageQuery,
  getDefaultApiBaseUrl,
  type ApiBaseUrlLocation
} from '@1flowbase/api-client';
import type {
  JsBlockHostActionEffect,
  JsBlockHostDataEffect,
  JsBlockHostEffectHandler
} from '@1flowbase/page-runtime';

export interface FrontstageJsBlockCapabilityClient {
  dispatchFrontstageQuery: typeof dispatchFrontstageQuery;
  dispatchFrontstageAction: typeof dispatchFrontstageAction;
}

export interface CreateFrontstageJsBlockCapabilityHandlersOptions {
  workspaceId: string;
  pageId: string;
  tabId: string;
  csrfToken?: string | null;
  baseUrl?: string;
  locationLike?: ApiBaseUrlLocation;
  client?: FrontstageJsBlockCapabilityClient;
}

const defaultClient: FrontstageJsBlockCapabilityClient = {
  dispatchFrontstageQuery,
  dispatchFrontstageAction
};

export function getFrontstageJsBlockCapabilityApiBaseUrl(
  locationLike: ApiBaseUrlLocation | undefined =
    typeof window !== 'undefined' ? window.location : undefined
): string {
  return import.meta.env.VITE_API_BASE_URL ?? getDefaultApiBaseUrl(locationLike);
}

export function createFrontstageJsBlockCapabilityHandlers(
  options: CreateFrontstageJsBlockCapabilityHandlersOptions
): {
  data: JsBlockHostEffectHandler<JsBlockHostDataEffect>;
  action: JsBlockHostEffectHandler<JsBlockHostActionEffect>;
} {
  const client = options.client ?? defaultClient;
  const baseUrl =
    options.baseUrl ?? getFrontstageJsBlockCapabilityApiBaseUrl(options.locationLike);

  return {
    data: (effect) =>
      client.dispatchFrontstageQuery(
        options.workspaceId,
        options.pageId,
        options.tabId,
        { query_id: effect.queryId, params: effect.params ?? {} },
        baseUrl
      ),
    action: (effect) =>
      client.dispatchFrontstageAction(
        options.workspaceId,
        options.pageId,
        options.tabId,
        { action_id: effect.actionId, params: effect.payload ?? {} },
        requireCsrfToken(options.csrfToken),
        baseUrl
      )
  };
}

function requireCsrfToken(csrfToken: string | null | undefined): string {
  if (typeof csrfToken !== 'string' || csrfToken.length === 0) {
    throw new Error('JS Block action capability requires csrfToken.');
  }
  return csrfToken;
}
