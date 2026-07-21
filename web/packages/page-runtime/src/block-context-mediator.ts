import type { BlockRuntimeErrorCode } from '@1flowbase/page-protocol';

import type { JsBlockWorkerEffect } from './js-block-worker-runtime';

export type BlockContextMediatorRejectionCode =
  | Extract<BlockRuntimeErrorCode, 'event_denied' | 'interface_denied'>
  | 'payload_invalid'
  | 'effect_invalid';

export type BlockContextJsonValue =
  | string
  | number
  | boolean
  | null
  | BlockContextJsonValue[]
  | { [key: string]: BlockContextJsonValue };

export interface BlockContextMediatorPolicy {
  allowedEvents?: readonly string[];
  allowedInterfaces?: readonly string[];
  maxEventChainDepth?: number;
}

export interface BlockContextMediatorContext {
  tickId?: string;
}

export interface BlockContextMediatorState {
  eventChains: Record<string, number>;
}

export type BlockContextMediatorResult =
  | {
      ok: true;
      requestId: string;
      effect: JsBlockWorkerEffect;
    }
  | {
      ok: false;
      requestId?: string;
      code: BlockContextMediatorRejectionCode;
      path: string;
      message: string;
    };

export interface BlockContextMediatorTransition {
  state: BlockContextMediatorState;
  result: BlockContextMediatorResult;
}

export interface BlockContextMediator {
  getState(): BlockContextMediatorState;
  handle(
    effect: unknown,
    context?: BlockContextMediatorContext
  ): BlockContextMediatorTransition;
}

type NormalizedEffect = JsBlockWorkerEffect;

type JsonNormalizationResult =
  | { ok: true; value: BlockContextJsonValue }
  | {
      ok: false;
      path: string;
      message: string;
    };

const DEFAULT_MAX_EVENT_CHAIN_DEPTH = 32;

export function createBlockContextMediatorState(): BlockContextMediatorState {
  return {
    eventChains: {}
  };
}

export function createBlockContextMediator(
  policy: BlockContextMediatorPolicy,
  initialState: BlockContextMediatorState = createBlockContextMediatorState()
): BlockContextMediator {
  let state = initialState;

  return {
    getState() {
      return state;
    },
    handle(effect, context) {
      const transition = reduceBlockContextMediator(
        state,
        effect,
        policy,
        context
      );
      state = transition.state;
      return transition;
    }
  };
}

export function reduceBlockContextMediator(
  state: BlockContextMediatorState,
  effect: unknown,
  policy: BlockContextMediatorPolicy,
  context: BlockContextMediatorContext = {}
): BlockContextMediatorTransition {
  const effectResult = normalizeEffect(effect);
  if (!effectResult.ok) {
    return {
      state,
      result: effectResult.result
    };
  }

  const normalizedEffect = effectResult.effect;
  switch (normalizedEffect.type) {
    case 'event':
      return reduceEventEffect(state, normalizedEffect, policy, context);
    case 'interface':
      return reduceInterfaceEffect(state, normalizedEffect, policy);
  }
}

function reduceEventEffect(
  state: BlockContextMediatorState,
  effect: Extract<NormalizedEffect, { type: 'event' }>,
  policy: BlockContextMediatorPolicy,
  context: BlockContextMediatorContext
): BlockContextMediatorTransition {
  if (!toSet(policy.allowedEvents).has(effect.name)) {
    return reject(state, {
      requestId: effect.requestId,
      code: 'event_denied',
      path: 'event.name',
      message: `Event is not allowed: ${effect.name}.`
    });
  }

  const payloadResult = normalizeOptionalPayload(effect.payload);
  if (!payloadResult.ok) {
    return rejectPayload(state, effect.requestId, payloadResult);
  }

  const chainKey = getEventChainKey(effect.requestId, context.tickId);
  const currentDepth = state.eventChains[chainKey] ?? 0;
  const nextDepth = currentDepth + 1;
  const maxDepth = getMaxEventChainDepth(policy);
  if (nextDepth > maxDepth) {
    return reject(state, {
      requestId: effect.requestId,
      code: 'event_denied',
      path: 'event.chain',
      message: `Event chain exceeded the maximum depth of ${maxDepth}.`
    });
  }

  const nextState = {
    ...state,
    eventChains: {
      ...state.eventChains,
      [chainKey]: nextDepth
    }
  };

  return allow(nextState, {
    type: 'event',
    requestId: effect.requestId,
    name: effect.name,
    ...(payloadResult.value === undefined
      ? {}
      : { payload: payloadResult.value })
  });
}

function reduceInterfaceEffect(
  state: BlockContextMediatorState,
  effect: Extract<NormalizedEffect, { type: 'interface' }>,
  policy: BlockContextMediatorPolicy
): BlockContextMediatorTransition {
  if (!toSet(policy.allowedInterfaces).has(effect.bindingAlias)) {
    return reject(state, {
      requestId: effect.requestId,
      code: 'interface_denied',
      path: 'interface.bindingAlias',
      message: `Interface binding is not allowed: ${effect.bindingAlias}.`
    });
  }

  const payloadResult = normalizeOptionalPayload(effect.request, 'request');
  if (!payloadResult.ok) {
    return rejectPayload(state, effect.requestId, payloadResult);
  }

  return allow(state, {
    type: 'interface',
    requestId: effect.requestId,
    ...(effect.effectId ? { effectId: effect.effectId } : {}),
    bindingAlias: effect.bindingAlias,
    ...(effect.operation ? { operation: effect.operation } : {}),
    ...(effect.streamId ? { streamId: effect.streamId } : {}),
    ...(payloadResult.value === undefined
      ? {}
      : { request: payloadResult.value })
  });
}

function allow(
  state: BlockContextMediatorState,
  effect: JsBlockWorkerEffect
): BlockContextMediatorTransition {
  return {
    state,
    result: {
      ok: true,
      requestId: effect.requestId,
      effect
    }
  };
}

function reject(
  state: BlockContextMediatorState,
  result: Omit<Exclude<BlockContextMediatorResult, { ok: true }>, 'ok'>
): BlockContextMediatorTransition {
  return {
    state,
    result: {
      ok: false,
      ...result
    }
  };
}

function rejectPayload(
  state: BlockContextMediatorState,
  requestId: string,
  payloadResult: Extract<JsonNormalizationResult, { ok: false }>
): BlockContextMediatorTransition {
  return reject(state, {
    requestId,
    code: 'payload_invalid',
    path: payloadResult.path,
    message: payloadResult.message
  });
}

function normalizeEffect(
  value: unknown
):
  | { ok: true; effect: NormalizedEffect }
  | { ok: false; result: Exclude<BlockContextMediatorResult, { ok: true }> } {
  if (!isRecord(value)) {
    return effectInvalid('effect', 'Worker effect must be an object.');
  }

  const type = readStringProperty(value, 'type', 'effect.type');
  if (!type.ok) {
    return effectInvalid(type.path, type.message);
  }

  const requestId = readStringProperty(value, 'requestId', 'effect.requestId');
  if (!requestId.ok) {
    return effectInvalid(requestId.path, requestId.message);
  }

  if (type.value === 'event') {
    const name = readStringProperty(value, 'name', 'effect.name');
    if (!name.ok) {
      return effectInvalid(name.path, name.message, requestId.value);
    }
    const payload = readOptionalProperty(value, 'payload');
    if (!payload.ok) {
      return effectInvalid(payload.path, payload.message, requestId.value);
    }

    return {
      ok: true,
      effect: {
        type: 'event',
        requestId: requestId.value,
        name: name.value,
        ...(payload.hasValue ? { payload: payload.value } : {})
      }
    };
  }

  if (type.value === 'interface') {
    const bindingAlias = readStringProperty(
      value,
      'bindingAlias',
      'effect.bindingAlias'
    );
    if (!bindingAlias.ok) {
      return effectInvalid(
        bindingAlias.path,
        bindingAlias.message,
        requestId.value
      );
    }
    const request = readOptionalProperty(value, 'request');
    if (!request.ok) {
      return effectInvalid(request.path, request.message, requestId.value);
    }
    const effectId = readOptionalStringProperty(
      value,
      'effectId',
      'effect.effectId'
    );
    if (!effectId.ok) {
      return effectInvalid(effectId.path, effectId.message, requestId.value);
    }
    const operation = readOptionalStringProperty(
      value,
      'operation',
      'effect.operation'
    );
    if (
      !operation.ok ||
      (operation.value !== undefined &&
        !['call', 'stream_open', 'stream_next', 'stream_cancel'].includes(
          operation.value
        ))
    ) {
      return effectInvalid(
        'effect.operation',
        'Interface operation is invalid.',
        requestId.value
      );
    }
    const streamId = readOptionalStringProperty(
      value,
      'streamId',
      'effect.streamId'
    );
    if (!streamId.ok) {
      return effectInvalid(streamId.path, streamId.message, requestId.value);
    }
    if (
      (operation.value === 'stream_next' ||
        operation.value === 'stream_cancel') &&
      !streamId.value
    ) {
      return effectInvalid(
        'effect.streamId',
        'Interface stream id is required.',
        requestId.value
      );
    }

    return {
      ok: true,
      effect: {
        type: 'interface',
        requestId: requestId.value,
        ...(effectId.value ? { effectId: effectId.value } : {}),
        bindingAlias: bindingAlias.value,
        ...(operation.value
          ? {
              operation: operation.value as
                | 'call'
                | 'stream_open'
                | 'stream_next'
                | 'stream_cancel'
            }
          : {}),
        ...(streamId.value ? { streamId: streamId.value } : {}),
        ...(request.hasValue ? { request: request.value } : {})
      }
    };
  }

  return effectInvalid(
    'effect.type',
    `Worker effect type is unsupported: ${type.value}.`,
    requestId.value
  );
}

function normalizeOptionalPayload(
  value: unknown,
  path = 'payload'
):
  | { ok: true; value?: BlockContextJsonValue }
  | Extract<JsonNormalizationResult, { ok: false }> {
  if (value === undefined) {
    return { ok: true };
  }

  return normalizeJsonValue(value, path, new WeakSet<object>());
}

function normalizeJsonValue(
  value: unknown,
  path: string,
  seen: WeakSet<object>
): JsonNormalizationResult {
  if (value === null) {
    return { ok: true, value: null };
  }

  if (typeof value === 'string' || typeof value === 'boolean') {
    return { ok: true, value };
  }

  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      return invalidJson(path, 'Payload numbers must be finite.');
    }

    return { ok: true, value };
  }

  if (typeof value === 'function' || typeof value === 'symbol') {
    return invalidJson(path, 'Payload values must be JSON-compatible data.');
  }

  if (typeof value === 'bigint' || value === undefined) {
    return invalidJson(path, 'Payload values must be JSON-compatible data.');
  }

  if (!isRecordLike(value)) {
    return invalidJson(path, 'Payload values must be JSON-compatible data.');
  }

  if (seen.has(value)) {
    return invalidJson(path, 'Payload must not contain circular references.');
  }

  seen.add(value);

  if (Array.isArray(value)) {
    return normalizeJsonArray(value, path, seen);
  }

  return normalizeJsonObject(value, path, seen);
}

function normalizeJsonArray(
  value: unknown[],
  path: string,
  seen: WeakSet<object>
): JsonNormalizationResult {
  const output: BlockContextJsonValue[] = [];

  for (let index = 0; index < value.length; index += 1) {
    const descriptor = getOwnDescriptor(value, `${index}`, `${path}[${index}]`);
    if (!descriptor.ok) {
      return descriptor;
    }

    if (!descriptor.descriptor || !('value' in descriptor.descriptor)) {
      return invalidJson(
        `${path}[${index}]`,
        'Payload accessors are not JSON-compatible data.'
      );
    }

    const item = normalizeJsonValue(
      descriptor.descriptor.value,
      `${path}[${index}]`,
      seen
    );
    if (!item.ok) {
      return item;
    }
    output.push(item.value);
  }

  return { ok: true, value: output };
}

function normalizeJsonObject(
  value: object,
  path: string,
  seen: WeakSet<object>
): JsonNormalizationResult {
  const prototype = safeGetPrototypeOf(value, path);
  if (!prototype.ok) {
    return prototype;
  }

  if (prototype.value !== null && prototype.value !== Object.prototype) {
    return invalidJson(path, 'Payload objects must be plain JSON objects.');
  }

  const symbolKeys = safeGetOwnPropertySymbols(value, path);
  if (!symbolKeys.ok) {
    return symbolKeys;
  }

  if (symbolKeys.value.length > 0) {
    return invalidJson(path, 'Payload objects must not contain symbol keys.');
  }

  const stringKeys = safeObjectKeys(value, path);
  if (!stringKeys.ok) {
    return stringKeys;
  }

  const output: { [key: string]: BlockContextJsonValue } = {};
  for (const key of stringKeys.value) {
    const descriptor = getOwnDescriptor(value, key, `${path}.${key}`);
    if (!descriptor.ok) {
      return descriptor;
    }

    if (!descriptor.descriptor || !('value' in descriptor.descriptor)) {
      return invalidJson(
        `${path}.${key}`,
        'Payload accessors are not JSON-compatible data.'
      );
    }

    const property = normalizeJsonValue(
      descriptor.descriptor.value,
      `${path}.${key}`,
      seen
    );
    if (!property.ok) {
      return property;
    }

    output[key] = property.value;
  }

  return { ok: true, value: output };
}

function readStringProperty(
  record: Record<string, unknown>,
  key: string,
  path: string
): { ok: true; value: string } | { ok: false; path: string; message: string } {
  const value = readRequiredProperty(record, key, path);
  if (!value.ok) {
    return value;
  }

  if (typeof value.value !== 'string' || value.value.length === 0) {
    return {
      ok: false,
      path,
      message: `${key} must be a non-empty string.`
    };
  }

  return { ok: true, value: value.value };
}

function readOptionalStringProperty(
  record: Record<string, unknown>,
  key: string,
  path: string
): { ok: true; value?: string } | { ok: false; path: string; message: string } {
  const value = readOptionalProperty(record, key);
  if (!value.ok) {
    return value;
  }

  if (!value.hasValue) {
    return { ok: true };
  }

  if (typeof value.value !== 'string' || value.value.length === 0) {
    return {
      ok: false,
      path,
      message: `${key} must be a non-empty string.`
    };
  }

  return { ok: true, value: value.value };
}

function readRequiredProperty(
  record: Record<string, unknown>,
  key: string,
  path: string
): { ok: true; value: unknown } | { ok: false; path: string; message: string } {
  const property = readOptionalProperty(record, key);
  if (!property.ok) {
    return property;
  }

  if (!property.hasValue) {
    return {
      ok: false,
      path,
      message: `${key} is required.`
    };
  }

  return { ok: true, value: property.value };
}

function readOptionalProperty(
  record: Record<string, unknown>,
  key: string
):
  | { ok: true; hasValue: false }
  | { ok: true; hasValue: true; value: unknown }
  | { ok: false; path: string; message: string } {
  const descriptor = getOwnDescriptor(record, key, `effect.${key}`);
  if (!descriptor.ok) {
    return descriptor;
  }

  if (!descriptor.descriptor) {
    return { ok: true, hasValue: false };
  }

  if (!('value' in descriptor.descriptor)) {
    return {
      ok: false,
      path: `effect.${key}`,
      message: `${key} accessors are not supported.`
    };
  }

  return {
    ok: true,
    hasValue: true,
    value: descriptor.descriptor.value
  };
}

function getOwnDescriptor(
  record: object,
  key: string,
  path: string
):
  | { ok: true; descriptor?: PropertyDescriptor }
  | { ok: false; path: string; message: string } {
  try {
    return {
      ok: true,
      descriptor: Object.getOwnPropertyDescriptor(record, key)
    };
  } catch (error) {
    return {
      ok: false,
      path,
      message: getUnknownAccessMessage(error)
    };
  }
}

function safeGetPrototypeOf(
  value: object,
  path: string
):
  | { ok: true; value: object | null }
  | { ok: false; path: string; message: string } {
  try {
    return { ok: true, value: Object.getPrototypeOf(value) };
  } catch (error) {
    return {
      ok: false,
      path,
      message: getUnknownAccessMessage(error)
    };
  }
}

function safeGetOwnPropertySymbols(
  value: object,
  path: string
):
  | { ok: true; value: symbol[] }
  | { ok: false; path: string; message: string } {
  try {
    return { ok: true, value: Object.getOwnPropertySymbols(value) };
  } catch (error) {
    return {
      ok: false,
      path,
      message: getUnknownAccessMessage(error)
    };
  }
}

function safeObjectKeys(
  value: object,
  path: string
):
  | { ok: true; value: string[] }
  | { ok: false; path: string; message: string } {
  try {
    return { ok: true, value: Object.keys(value) };
  } catch (error) {
    return {
      ok: false,
      path,
      message: getUnknownAccessMessage(error)
    };
  }
}

function effectInvalid(
  path: string,
  message: string,
  requestId?: string
): { ok: false; result: Exclude<BlockContextMediatorResult, { ok: true }> } {
  return {
    ok: false,
    result: {
      ok: false,
      requestId,
      code: 'effect_invalid',
      path,
      message
    }
  };
}

function invalidJson(
  path: string,
  message: string
): Extract<JsonNormalizationResult, { ok: false }> {
  return {
    ok: false,
    path,
    message
  };
}

function getUnknownAccessMessage(error: unknown): string {
  return error instanceof Error
    ? `Payload access failed: ${error.message}`
    : 'Payload access failed.';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isRecordLike(value: unknown): value is object {
  return typeof value === 'object' && value !== null;
}

function toSet(values: readonly string[] | undefined): ReadonlySet<string> {
  return new Set(values ?? []);
}

function getMaxEventChainDepth(policy: BlockContextMediatorPolicy): number {
  const depth = policy.maxEventChainDepth;
  if (typeof depth !== 'number' || !Number.isFinite(depth) || depth < 1) {
    return DEFAULT_MAX_EVENT_CHAIN_DEPTH;
  }

  return Math.floor(depth);
}

function getEventChainKey(
  requestId: string,
  tickId: string | undefined
): string {
  return `${requestId}::${tickId ?? 'default'}`;
}
