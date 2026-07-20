import {
  dispatchFrontstageCallable,
  getDefaultApiBaseUrl,
  issueFrontstageCallableWriteGrant,
  type ApiBaseUrlLocation,
  type FrontstageCallableRequest
} from '@1flowbase/api-client';
import type {
  JsBlockHostEffectHandler,
  JsBlockHostInterfaceEffect
} from '@1flowbase/page-runtime';
import type { FrontstageBlockInterfaceBinding } from './page-document';

export interface FrontstageJsBlockCapabilityClient {
  dispatchFrontstageCallable: typeof dispatchFrontstageCallable;
  issueFrontstageCallableWriteGrant: typeof issueFrontstageCallableWriteGrant;
}

export interface CreateFrontstageJsBlockCapabilityHandlersOptions {
  workspaceId: string;
  pageId: string;
  tabId: string;
  csrfToken?: string | null;
  resolveBinding(
    requestId: string,
    bindingAlias: string
  ): { blockId: string; binding: FrontstageBlockInterfaceBinding } | null;
  baseUrl?: string;
  locationLike?: ApiBaseUrlLocation;
  client?: FrontstageJsBlockCapabilityClient;
}

const defaultClient: FrontstageJsBlockCapabilityClient = {
  dispatchFrontstageCallable,
  issueFrontstageCallableWriteGrant
};

interface DraftRunAuthorization {
  draftHash: string;
  grantsByAlias: Map<string, string>;
}

export interface FrontstageJsBlockCapabilityHandlers {
  interface: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
  prepareDraftRun(input: {
    blockId: string;
    runId: string;
    draftHash: string;
    bindings: readonly FrontstageBlockInterfaceBinding[];
  }): Promise<void>;
  revokeDraftRun(runId: string): void;
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
): FrontstageJsBlockCapabilityHandlers {
  const client = options.client ?? defaultClient;
  const baseUrl =
    options.baseUrl ??
    getFrontstageJsBlockCapabilityApiBaseUrl(options.locationLike);
  const draftRuns = new Map<string, DraftRunAuthorization>();

  return {
    async prepareDraftRun({ blockId, runId, draftHash, bindings }) {
      const csrfToken = requireCsrfToken(options.csrfToken);
      const grants = await Promise.all(
        bindings
          .filter((binding) => binding.risk_level === 'high')
          .map(async (binding) => {
            const grant = await client.issueFrontstageCallableWriteGrant(
              options.workspaceId,
              options.pageId,
              options.tabId,
              {
                block_id: blockId,
                binding_alias: binding.alias,
                schema_digest: binding.schema_digest,
                run_id: runId,
                draft_hash: draftHash
              },
              csrfToken,
              baseUrl
            );
            return [binding.alias, grant.grant_token] as const;
          })
      );
      draftRuns.set(runId, {
        draftHash,
        grantsByAlias: new Map(grants)
      });
    },
    revokeDraftRun(runId) {
      draftRuns.delete(runId);
    },
    interface: (effect) => {
      const resolved = options.resolveBinding(
        effect.requestId,
        effect.bindingAlias
      );
      if (!resolved) {
        throw new Error(
          `Interface binding is not registered: ${effect.bindingAlias}.`
        );
      }
      const draftRun = draftRuns.get(effect.requestId);
      return client.dispatchFrontstageCallable(
        options.workspaceId,
        options.pageId,
        options.tabId,
        {
          block_id: resolved.blockId,
          binding_alias: resolved.binding.alias,
          schema_digest: resolved.binding.schema_digest,
          run_id: effect.requestId,
          draft_hash: draftRun?.draftHash ?? 'runtime',
          ...(effect.request === undefined
            ? {}
            : { request: effect.request as FrontstageCallableRequest }),
          ...(draftRun?.grantsByAlias.has(resolved.binding.alias)
            ? {
                write_grant: draftRun.grantsByAlias.get(resolved.binding.alias)
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
