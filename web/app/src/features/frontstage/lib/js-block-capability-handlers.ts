import {
  dispatchFrontstageCallable,
  dispatchFrontstageCallableBinary,
  dispatchFrontstageCallableStream,
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
  dispatchFrontstageCallableBinary: typeof dispatchFrontstageCallableBinary;
  dispatchFrontstageCallableStream: typeof dispatchFrontstageCallableStream;
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
  dispatchFrontstageCallableBinary,
  dispatchFrontstageCallableStream,
  issueFrontstageCallableWriteGrant
};

interface DraftRunAuthorization {
  draftHash: string;
  grantsByAlias: Map<string, string>;
}

export interface FrontstageJsBlockCapabilityHandlers {
  interface: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
  disposeRequest(requestId?: string): void;
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
  const streams = new Map<
    string,
    {
      requestId: string;
      bindingAlias: string;
      iterator: AsyncIterator<unknown>;
      cancel: () => void;
    }
  >();
  let nextStreamId = 1;

  const disposeStreams = (requestId?: string) => {
    for (const [streamId, stream] of streams) {
      if (requestId !== undefined && stream.requestId !== requestId) continue;
      streams.delete(streamId);
      stream.cancel();
      void stream.iterator.return?.();
    }
  };

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
      disposeStreams(runId);
    },
    disposeRequest: disposeStreams,
    interface: async (effect) => {
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
      if (
        effect.operation === 'stream_next' ||
        effect.operation === 'stream_cancel'
      ) {
        const stream = effect.streamId
          ? streams.get(effect.streamId)
          : undefined;
        if (
          !stream ||
          stream.requestId !== effect.requestId ||
          stream.bindingAlias !== effect.bindingAlias
        ) {
          throw new Error('Interface stream is not registered for this run.');
        }
        if (effect.operation === 'stream_cancel') {
          streams.delete(effect.streamId as string);
          stream.cancel();
          await stream.iterator.return?.();
          return undefined;
        }
        const item = await stream.iterator.next();
        if (item.done) streams.delete(effect.streamId as string);
        return item.done ? { done: true } : { done: false, value: item.value };
      }
      if (effect.operation === 'stream_open') {
        if (resolved.binding.response_media_type !== 'text/event-stream') {
          throw new Error('Bound interface does not return an event stream.');
        }
        const iterable = await client.dispatchFrontstageCallableStream(
          options.workspaceId,
          options.pageId,
          options.tabId,
          createDispatchInput(effect, resolved, draftRun),
          requireCsrfToken(options.csrfToken),
          baseUrl
        );
        const streamId = `${effect.requestId}:stream-${nextStreamId++}`;
        streams.set(streamId, {
          requestId: effect.requestId,
          bindingAlias: effect.bindingAlias,
          iterator: iterable[Symbol.asyncIterator](),
          cancel: iterable.cancel
        });
        return { stream_id: streamId };
      }
      const dispatch = isBinaryResponse(resolved.binding.response_media_type)
        ? client.dispatchFrontstageCallableBinary
        : client.dispatchFrontstageCallable;
      return dispatch(
        options.workspaceId,
        options.pageId,
        options.tabId,
        createDispatchInput(effect, resolved, draftRun),
        requireCsrfToken(options.csrfToken),
        baseUrl
      );
    }
  };
}

function createDispatchInput(
  effect: JsBlockHostInterfaceEffect,
  resolved: { blockId: string; binding: FrontstageBlockInterfaceBinding },
  draftRun: DraftRunAuthorization | undefined
) {
  return {
    block_id: resolved.blockId,
    binding_alias: resolved.binding.alias,
    schema_digest: resolved.binding.schema_digest,
    run_id: effect.requestId,
    draft_hash: draftRun?.draftHash ?? 'runtime',
    ...(effect.request === undefined
      ? {}
      : { request: effect.request as FrontstageCallableRequest }),
    ...(draftRun?.grantsByAlias.has(resolved.binding.alias)
      ? { write_grant: draftRun.grantsByAlias.get(resolved.binding.alias) }
      : {})
  };
}

function isBinaryResponse(mediaType: string | null): boolean {
  if (mediaType === null || mediaType === 'text/event-stream') return false;
  const normalized = mediaType.split(';', 1)[0]?.trim().toLocaleLowerCase();
  return normalized !== 'application/json' && !normalized?.endsWith('+json');
}

function requireCsrfToken(csrfToken: string | null | undefined): string {
  if (typeof csrfToken !== 'string' || csrfToken.length === 0) {
    throw new Error('JS Block callable interface requires csrfToken.');
  }
  return csrfToken;
}
