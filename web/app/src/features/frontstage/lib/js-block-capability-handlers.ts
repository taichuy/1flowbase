import {
  dispatchFrontstageCallable,
  dispatchFrontstageCallableStream,
  getDefaultApiBaseUrl,
  type ApiBaseUrlLocation,
  type FrontstageCallableRequest
} from '@1flowbase/api-client';
import type {
  BlockHostEffectHandler,
  BlockHostInterfaceEffect
} from '@1flowbase/page-runtime';

export interface FrontstageJsBlockCapabilityClient {
  dispatchFrontstageCallable: typeof dispatchFrontstageCallable;
  dispatchFrontstageCallableStream: typeof dispatchFrontstageCallableStream;
}

export interface CreateFrontstageJsBlockCapabilityHandlersOptions {
  workspaceId: string;
  pageId: string;
  tabId: string;
  csrfToken?: string | null;
  resolveBlockId(requestId: string): string | null;
  baseUrl?: string;
  locationLike?: ApiBaseUrlLocation;
  client?: FrontstageJsBlockCapabilityClient;
}

const defaultClient: FrontstageJsBlockCapabilityClient = {
  dispatchFrontstageCallable,
  dispatchFrontstageCallableStream
};

interface DraftRunRegistration {
  blockId: string;
}

export interface FrontstageJsBlockCapabilityHandlers {
  interface: BlockHostEffectHandler<BlockHostInterfaceEffect>;
  disposeRequest(requestId?: string): void;
  prepareDraftRun(input: {
    blockId: string;
    runId: string;
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
  const draftRuns = new Map<string, DraftRunRegistration>();
  const revokedDraftRuns = new Set<string>();
  const streams = new Map<
    string,
    {
      requestId: string;
      method: string;
      path: string;
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
    async prepareDraftRun({ blockId, runId }) {
      requireCsrfToken(options.csrfToken);
      revokedDraftRuns.delete(runId);
      draftRuns.set(runId, { blockId });
    },
    revokeDraftRun(runId) {
      draftRuns.delete(runId);
      revokedDraftRuns.add(runId);
      disposeStreams(runId);
    },
    disposeRequest: disposeStreams,
    interface: async (effect) => {
      if (revokedDraftRuns.has(effect.requestId)) {
        throw new Error('Draft run capability has been revoked.');
      }
      const blockId = options.resolveBlockId(effect.requestId);
      if (!blockId)
        throw new Error('Interface source block is not registered.');
      const draftRun = draftRuns.get(effect.requestId);
      if (draftRun && draftRun.blockId !== blockId) {
        throw new Error('Draft run block identity does not match.');
      }
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
          stream.method !== effect.method ||
          stream.path !== effect.path
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
      const dispatch = async () => {
        const input = createDispatchInput(effect, blockId);
        if (effect.operation === 'stream_open') {
          const iterable = await client.dispatchFrontstageCallableStream(
            options.workspaceId,
            options.pageId,
            options.tabId,
            input,
            requireCsrfToken(options.csrfToken),
            baseUrl
          );
          const streamId = `${effect.requestId}:stream-${nextStreamId++}`;
          streams.set(streamId, {
            requestId: effect.requestId,
            method: effect.method,
            path: effect.path,
            iterator: iterable[Symbol.asyncIterator](),
            cancel: iterable.cancel
          });
          return { stream_id: streamId };
        }
        return client.dispatchFrontstageCallable(
          options.workspaceId,
          options.pageId,
          options.tabId,
          input,
          requireCsrfToken(options.csrfToken),
          baseUrl
        );
      };
      if (revokedDraftRuns.has(effect.requestId)) {
        throw new Error('Draft run capability has been revoked.');
      }
      return dispatch();
    }
  };
}

function createDispatchInput(
  effect: BlockHostInterfaceEffect,
  blockId: string
) {
  return {
    block_id: blockId,
    method: effect.method,
    path: effect.path,
    ...(effect.request === undefined
      ? {}
      : { request: effect.request as FrontstageCallableRequest })
  };
}

function requireCsrfToken(csrfToken: string | null | undefined): string {
  if (typeof csrfToken !== 'string' || csrfToken.length === 0) {
    throw new Error('JS Block callable interface requires csrfToken.');
  }
  return csrfToken;
}
