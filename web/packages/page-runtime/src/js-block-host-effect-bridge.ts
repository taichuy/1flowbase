import {
  BLOCK_RUNTIME_ERROR_CODES,
  type BlockRuntimeErrorCode
} from '@1flowbase/page-protocol';

import type {
  BlockContextMediator,
  BlockContextMediatorContext,
  BlockContextMediatorResult,
  BlockContextMediatorState,
  BlockContextMediatorTransition
} from './block-context-mediator';
import type {
  JsBlockRunError,
  JsBlockWorkerEffect,
  JsBlockWorkerEffectResultMessage
} from './js-block-worker-runtime';

export type JsBlockHostInterfaceEffect = Extract<
  JsBlockWorkerEffect,
  { type: 'interface' }
>;
export type JsBlockHostResolvableEffect = JsBlockHostInterfaceEffect;
type JsBlockHostEffectWithId<Effect extends JsBlockHostResolvableEffect> =
  Effect & {
    effectId: string;
  };

export type JsBlockHostEffectHandler<
  Effect extends JsBlockHostResolvableEffect
> = (effect: Effect) => unknown | Promise<unknown>;

export interface JsBlockHostEffectHandlers {
  interface?: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
  disposeRequest?: (requestId?: string) => void;
}

export interface JsBlockHostEffectBridgeOptions {
  mediator: BlockContextMediator;
  resolveEffect(message: JsBlockWorkerEffectResultMessage): void;
  handlers?: JsBlockHostEffectHandlers;
  onInterfaceCall?: (trace: JsBlockInterfaceCallTrace) => void;
}

export interface JsBlockInterfaceCallTrace {
  requestId: string;
  effectId: string;
  interfaceId: string;
  schemaDigest: string;
  request?: unknown;
  response?: unknown;
  status: 'succeeded' | 'failed';
  durationMs: number;
  error?: string;
}

export type JsBlockHostEffectBridgeHandleResult =
  | { handled: false }
  | {
      handled: true;
      transition: BlockContextMediatorTransition;
    };

export interface JsBlockHostEffectBridge {
  getMediatorState(): BlockContextMediatorState;
  handle(
    message: unknown,
    context?: BlockContextMediatorContext
  ): JsBlockHostEffectBridgeHandleResult;
}

const RUNTIME_ERROR_CODES = new Set<string>(BLOCK_RUNTIME_ERROR_CODES);
const MAX_INTERFACE_TRACE_JSON_LENGTH = 16_000;

export function createJsBlockHostEffectBridge(
  options: JsBlockHostEffectBridgeOptions
): JsBlockHostEffectBridge {
  const resolveEffect = options.resolveEffect;
  const interfaceHandler = options.handlers?.interface;

  return {
    getMediatorState() {
      return options.mediator.getState();
    },
    handle(message, context) {
      if (!isWorkerEffectMessage(message)) {
        return { handled: false };
      }

      const transition = options.mediator.handle(message, context);
      const result = transition.result;
      if (!result.ok) {
        resolveDeniedEffect(message, result, resolveEffect);
        return { handled: true, transition };
      }

      const effect = result.effect;
      if (effect.type === 'event' || !hasEffectId(effect)) {
        return { handled: true, transition };
      }

      if (!interfaceHandler) {
        resolveMissingHandler(
          effect,
          'interface_denied',
          'interface.handler',
          resolveEffect
        );
        return { handled: true, transition };
      }
      resolveAllowedEffect(
        effect,
        interfaceHandler,
        resolveEffect,
        options.onInterfaceCall
      );
      return { handled: true, transition };
    }
  };
}

function resolveMissingHandler(
  effect: JsBlockHostEffectWithId<JsBlockHostResolvableEffect>,
  code: BlockRuntimeErrorCode,
  path: string,
  resolveEffect: (message: JsBlockWorkerEffectResultMessage) => void
): void {
  const message = `Host handler is not registered for ${effect.type} capability.`;
  resolveEffect({
    direction: 'host_to_worker',
    type: 'effect_result',
    requestId: effect.requestId,
    effectId: effect.effectId,
    ok: false,
    error: {
      kind: 'runtime_error',
      message,
      errors: [{ code, path, message }]
    }
  });
}

function resolveAllowedEffect<Effect extends JsBlockHostResolvableEffect>(
  effect: JsBlockHostEffectWithId<Effect>,
  handler: JsBlockHostEffectHandler<Effect>,
  resolveEffect: (message: JsBlockWorkerEffectResultMessage) => void,
  onInterfaceCall?: (trace: JsBlockInterfaceCallTrace) => void
): void {
  const startedAt = Date.now();
  try {
    const value = handler(effect);
    if (isPromiseLike(value)) {
      void value.then(
        (resolvedValue) => {
          resolveEffect(createEffectSuccessMessage(effect, resolvedValue));
          emitInterfaceTrace(effect, startedAt, onInterfaceCall, {
            status: 'succeeded',
            response: resolvedValue
          });
        },
        (error) => {
          resolveEffect(createHandlerFailureMessage(effect, error));
          emitInterfaceTrace(effect, startedAt, onInterfaceCall, {
            status: 'failed',
            error:
              error instanceof Error ? error.message : 'Interface call failed.'
          });
        }
      );
      return;
    }

    resolveEffect(createEffectSuccessMessage(effect, value));
    emitInterfaceTrace(effect, startedAt, onInterfaceCall, {
      status: 'succeeded',
      response: value
    });
  } catch (error) {
    resolveEffect(createHandlerFailureMessage(effect, error));
    emitInterfaceTrace(effect, startedAt, onInterfaceCall, {
      status: 'failed',
      error: error instanceof Error ? error.message : 'Interface call failed.'
    });
  }
}

function emitInterfaceTrace(
  effect: JsBlockHostEffectWithId<JsBlockHostResolvableEffect>,
  startedAt: number,
  callback: ((trace: JsBlockInterfaceCallTrace) => void) | undefined,
  result: Pick<JsBlockInterfaceCallTrace, 'status' | 'response' | 'error'>
): void {
  callback?.({
    requestId: effect.requestId,
    effectId: effect.effectId,
    interfaceId: effect.interfaceId,
    schemaDigest: effect.schemaDigest,
    ...(effect.request === undefined
      ? {}
      : { request: sanitizeTraceValue(effect.request) }),
    ...(result.response === undefined
      ? {}
      : { response: sanitizeTraceValue(result.response) }),
    status: result.status,
    durationMs: Math.max(0, Date.now() - startedAt),
    ...(result.error ? { error: result.error } : {})
  });
}

function sanitizeTraceValue(value: unknown): unknown {
  try {
    const serialized = JSON.stringify(value, (key, item) => {
      if (/authorization|cookie|token|secret|api[_-]?key/i.test(key)) {
        return '[REDACTED]';
      }
      if (key === 'base64' && typeof item === 'string') {
        return `[BASE64 ${item.length} chars]`;
      }
      if (item instanceof Uint8Array) {
        return { type: 'Uint8Array', byte_length: item.byteLength };
      }
      if (typeof item === 'string' && item.length > 4_000) {
        return `${item.slice(0, 4_000)}…[TRUNCATED ${item.length - 4_000} chars]`;
      }
      return item;
    });
    if (serialized === undefined) return '[Unserializable]';
    if (serialized.length > MAX_INTERFACE_TRACE_JSON_LENGTH) {
      return {
        type: 'truncated',
        character_length: serialized.length,
        preview: serialized.slice(0, MAX_INTERFACE_TRACE_JSON_LENGTH)
      };
    }
    return JSON.parse(serialized);
  } catch {
    return '[Unserializable]';
  }
}

function resolveDeniedEffect(
  message: WorkerEffectMessage,
  result: Exclude<BlockContextMediatorResult, { ok: true }>,
  resolveEffect: (message: JsBlockWorkerEffectResultMessage) => void
): void {
  if (
    message.type !== 'interface' ||
    typeof message.effectId !== 'string' ||
    message.effectId.length === 0 ||
    typeof result.requestId !== 'string'
  ) {
    return;
  }

  resolveEffect({
    direction: 'host_to_worker',
    type: 'effect_result',
    requestId: result.requestId,
    effectId: message.effectId,
    ok: false,
    error: createDeniedRunError(result)
  });
}

function createEffectSuccessMessage(
  effect: JsBlockHostEffectWithId<JsBlockHostResolvableEffect>,
  value: unknown
): JsBlockWorkerEffectResultMessage {
  return {
    direction: 'host_to_worker',
    type: 'effect_result',
    requestId: effect.requestId,
    effectId: effect.effectId,
    ok: true,
    ...(value === undefined ? {} : { value })
  };
}

function createDeniedRunError(
  result: Exclude<BlockContextMediatorResult, { ok: true }>
): JsBlockRunError {
  return {
    kind: 'runtime_error',
    message: result.message,
    errors: [
      {
        code: toBlockRuntimeErrorCode(result.code),
        path: result.path,
        message: result.message
      }
    ]
  };
}

function createHandlerFailureMessage(
  effect: JsBlockHostEffectWithId<JsBlockHostResolvableEffect>,
  error: unknown
): JsBlockWorkerEffectResultMessage {
  const message =
    error instanceof Error ? error.message : 'Host effect handler failed.';

  return {
    direction: 'host_to_worker',
    type: 'effect_result',
    requestId: effect.requestId,
    effectId: effect.effectId,
    ok: false,
    error: {
      kind: 'runtime_error',
      message,
      errors: [
        {
          code: 'runtime_error',
          path: `${effect.type}.handler`,
          message
        }
      ]
    }
  };
}

function toBlockRuntimeErrorCode(code: string): BlockRuntimeErrorCode {
  return RUNTIME_ERROR_CODES.has(code)
    ? (code as BlockRuntimeErrorCode)
    : 'runtime_error';
}

function hasEffectId<Effect extends JsBlockHostResolvableEffect>(
  effect: Effect
): effect is JsBlockHostEffectWithId<Effect> {
  return typeof effect.effectId === 'string' && effect.effectId.length > 0;
}

type WorkerEffectMessage = Record<string, unknown> & {
  direction: 'worker_to_host';
  type: 'event' | 'interface';
};

function isWorkerEffectMessage(value: unknown): value is WorkerEffectMessage {
  if (!isRecord(value) || value.direction !== 'worker_to_host') {
    return false;
  }

  return value.type === 'event' || value.type === 'interface';
}

function isPromiseLike(value: unknown): value is PromiseLike<unknown> {
  return (
    typeof value === 'object' &&
    value !== null &&
    'then' in value &&
    typeof value.then === 'function'
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
