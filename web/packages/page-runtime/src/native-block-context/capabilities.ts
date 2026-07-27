import type {
  BlockApiMethod,
  BlockApiRequest,
  BlockContext,
  BlockContextApi,
  BlockContextEvents,
  BlockContextOutputs,
  BlockContextOutputPublishResult,
  BlockProtocolError
} from '@1flowbase/page-protocol';

import type {
  BlockHostEffectHandler,
  BlockHostInterfaceEffect
} from './effects';

export type NativeBlockContextApiCallStatus =
  | 'pending'
  | 'succeeded'
  | 'failed';

export interface NativeBlockContextApiCallObservation {
  capability: 'api';
  requestId: string;
  instanceEpoch: string;
  callId: string;
  method: BlockApiMethod;
  path: string;
  status: NativeBlockContextApiCallStatus;
  durationMs: number;
  error?: string;
}

export interface NativeBlockContextCapabilityDiagnostic {
  requestId: string;
  instanceEpoch: string;
  capability: 'api' | 'events' | 'outputs';
  error: BlockProtocolError;
}

export interface NativeBlockContextEventInput {
  requestId: string;
  instanceEpoch: string;
  name: string;
  payload?: Record<string, unknown>;
}

export interface CreateNativeBlockContextCapabilitiesOptions {
  requestId: string;
  instanceEpoch: string;
  isCurrentInstance(): boolean;
  interfaceHandler: BlockHostEffectHandler<BlockHostInterfaceEffect>;
  outputs: BlockContextOutputs;
  emitEvent?(event: NativeBlockContextEventInput): void;
  observeApiCall?(observation: NativeBlockContextApiCallObservation): void;
  reportDiagnostic?(diagnostic: NativeBlockContextCapabilityDiagnostic): void;
  now?: () => number;
}

export type NativeBlockContextCapabilities = Pick<
  BlockContext,
  'api' | 'events' | 'outputs'
>;

export function createNativeBlockContextCapabilities(
  options: CreateNativeBlockContextCapabilitiesOptions
): NativeBlockContextCapabilities {
  const now = options.now ?? Date.now;
  let nextCallSequence = 0;

  const call = async <TResponse>(
    method: BlockApiMethod,
    path: string,
    request?: BlockApiRequest,
    operation: BlockHostInterfaceEffect['operation'] = 'call',
    streamId?: string
  ): Promise<TResponse> => {
    const callId = `${options.requestId}:call-${++nextCallSequence}`;
    const startedAt = now();
    requireCurrentInstance(options, 'api', 'interface.instance_epoch');
    observe(options, {
      capability: 'api',
      requestId: options.requestId,
      instanceEpoch: options.instanceEpoch,
      callId,
      method,
      path,
      status: 'pending',
      durationMs: 0
    });
    try {
      const response = await options.interfaceHandler({
        type: 'interface',
        requestId: options.requestId,
        effectId: callId,
        method,
        path,
        operation,
        ...(streamId ? { streamId } : {}),
        ...(request === undefined ? {} : { request })
      });
      requireCurrentInstance(options, 'api', 'interface.instance_epoch');
      observe(options, {
        capability: 'api',
        requestId: options.requestId,
        instanceEpoch: options.instanceEpoch,
        callId,
        method,
        path,
        status: 'succeeded',
        durationMs: elapsed(now, startedAt)
      });
      return response as TResponse;
    } catch (error) {
      const message = getErrorMessage(error);
      observe(options, {
        capability: 'api',
        requestId: options.requestId,
        instanceEpoch: options.instanceEpoch,
        callId,
        method,
        path,
        status: 'failed',
        durationMs: elapsed(now, startedAt),
        error: message
      });
      report(options, 'api', {
        code: 'runtime_error',
        path: 'interface.handler',
        message
      });
      throw error;
    }
  };

  const api: BlockContextApi = {
    get: (path, request) => call('GET', path, request),
    post: (path, request) => call('POST', path, request),
    put: (path, request) => call('PUT', path, request),
    patch: (path, request) => call('PATCH', path, request),
    delete: (path, request) => call('DELETE', path, request),
    head: (path, request) => call('HEAD', path, request),
    options: (path, request) => call('OPTIONS', path, request),
    stream: <TEvent>(
      method: BlockApiMethod,
      path: string,
      request?: BlockApiRequest
    ): AsyncIterable<TEvent> => ({
      [Symbol.asyncIterator]() {
        let streamId: string | null = null;
        let done = false;
        const open = async () => {
          if (streamId) return streamId;
          const opened = await call<{ stream_id?: unknown }>(
            method,
            path,
            request,
            'stream_open'
          );
          if (typeof opened?.stream_id !== 'string' || !opened.stream_id) {
            throw new Error('Native Block stream did not return a stream id.');
          }
          streamId = opened.stream_id;
          return streamId;
        };
        return {
          async next() {
            if (done) return { done: true, value: undefined };
            const activeStreamId = await open();
            const item = await call<{ done: boolean; value?: TEvent }>(
              method,
              path,
              undefined,
              'stream_next',
              activeStreamId
            );
            done = item.done;
            return item.done
              ? { done: true, value: undefined }
              : { done: false, value: item.value as TEvent };
          },
          async return() {
            if (!done && streamId) {
              await call(method, path, undefined, 'stream_cancel', streamId);
            }
            done = true;
            return { done: true, value: undefined };
          }
        };
      }
    })
  };

  const events: BlockContextEvents = {
    emit(name, payload) {
      requireCurrentInstance(options, 'events', 'event.instance_epoch');
      if (!options.emitEvent) {
        const error = new Error(
          'Native Block ctx.events.emit is not registered by this Host.'
        );
        report(options, 'events', {
          code: 'event_denied',
          path: 'event.handler',
          message: error.message
        });
        throw error;
      }
      options.emitEvent({
        requestId: options.requestId,
        instanceEpoch: options.instanceEpoch,
        name,
        ...(payload === undefined ? {} : { payload })
      });
    }
  };

  return {
    api,
    events,
    outputs: {
      publish(values) {
        if (!options.isCurrentInstance()) {
          return { ok: false, stale: true };
        }
        const published = options.outputs.publish(values);
        if (isPromiseLike<BlockContextOutputPublishResult>(published)) {
          return Promise.resolve(published).then((result) =>
            options.isCurrentInstance() ? result : { ok: false, stale: true }
          );
        }
        return options.isCurrentInstance()
          ? published
          : { ok: false, stale: true };
      }
    }
  };
}

function requireCurrentInstance(
  options: CreateNativeBlockContextCapabilitiesOptions,
  capability: NativeBlockContextCapabilityDiagnostic['capability'],
  path: string
): void {
  if (options.isCurrentInstance()) return;
  const error = new Error(
    'Native Block capability belongs to a stale instance.'
  );
  report(options, capability, {
    code: capability === 'events' ? 'event_denied' : 'interface_denied',
    path,
    message: error.message
  });
  throw error;
}

function observe(
  options: CreateNativeBlockContextCapabilitiesOptions,
  observation: NativeBlockContextApiCallObservation
): void {
  options.observeApiCall?.(observation);
}

function report(
  options: CreateNativeBlockContextCapabilitiesOptions,
  capability: NativeBlockContextCapabilityDiagnostic['capability'],
  error: BlockProtocolError
): void {
  options.reportDiagnostic?.({
    requestId: options.requestId,
    instanceEpoch: options.instanceEpoch,
    capability,
    error
  });
}

function elapsed(now: () => number, startedAt: number): number {
  return Math.max(0, now() - startedAt);
}

function getErrorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : 'Native Block capability call failed.';
}

function isPromiseLike<T>(value: unknown): value is PromiseLike<T> {
  return (
    (typeof value === 'object' || typeof value === 'function') &&
    value !== null &&
    typeof (value as { then?: unknown }).then === 'function'
  );
}
