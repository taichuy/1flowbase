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

export type JsBlockHostEffectHandler<Effect extends JsBlockHostResolvableEffect> = (
  effect: Effect
) => unknown | Promise<unknown>;

export interface JsBlockHostEffectHandlers {
  interface?: JsBlockHostEffectHandler<JsBlockHostInterfaceEffect>;
}

export interface JsBlockHostEffectBridgeOptions {
  mediator: BlockContextMediator;
  resolveEffect(message: JsBlockWorkerEffectResultMessage): void;
  handlers?: JsBlockHostEffectHandlers;
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
      resolveAllowedEffect(effect, interfaceHandler, resolveEffect);
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
  resolveEffect: (message: JsBlockWorkerEffectResultMessage) => void
): void {
  try {
    const value = handler(effect);
    if (isPromiseLike(value)) {
      void value.then(
        (resolvedValue) =>
          resolveEffect(createEffectSuccessMessage(effect, resolvedValue)),
        (error) => resolveEffect(createHandlerFailureMessage(effect, error))
      );
      return;
    }

    resolveEffect(createEffectSuccessMessage(effect, value));
  } catch (error) {
    resolveEffect(createHandlerFailureMessage(effect, error));
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

  return (
    value.type === 'event' || value.type === 'interface'
  );
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
