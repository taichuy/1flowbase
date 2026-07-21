import type { FrontstageBlockInstance } from '../page-document';
import type { FrontstageSignalAddress } from './types';

export interface FrontstageSignalSnapshot {
  revision: number;
  values: ReadonlyMap<string, unknown>;
}

export interface FrontstageSignalCommitResult {
  ok: boolean;
  snapshot: FrontstageSignalSnapshot;
  error?: string;
}

export function createFrontstageSignalSnapshot(): FrontstageSignalSnapshot {
  return { revision: 0, values: new Map() };
}

export function commitFrontstageBlockOutputs({
  block,
  outputs,
  scope,
  tabId,
  snapshot
}: {
  block: FrontstageBlockInstance;
  outputs: Record<string, unknown>;
  scope: 'tab' | 'page';
  tabId: string;
  snapshot: FrontstageSignalSnapshot;
}): FrontstageSignalCommitResult {
  const ports = block.ports?.outputs ?? [];
  for (const port of ports) {
    if (!Object.hasOwn(outputs, port.name)) {
      return failure(snapshot, `Declared output is missing: ${port.name}.`);
    }
    if (
      !isJsonValue(outputs[port.name]) ||
      !validateValue(outputs[port.name], port.schema)
    ) {
      return failure(
        snapshot,
        `Output does not match its schema: ${port.name}.`
      );
    }
  }
  for (const name of Object.keys(outputs)) {
    if (!ports.some((port) => port.name === name)) {
      return failure(snapshot, `Output is not declared: ${name}.`);
    }
  }

  const values = new Map(snapshot.values);
  for (const port of ports) {
    values.set(
      signalKey({
        scope,
        tab_id: tabId,
        block_id: block.id,
        output: port.name
      }),
      outputs[port.name]
    );
  }
  return { ok: true, snapshot: { revision: snapshot.revision + 1, values } };
}

export function readFrontstageSignal(
  snapshot: FrontstageSignalSnapshot,
  address: FrontstageSignalAddress
): unknown {
  return snapshot.values.get(signalKey(address));
}

export function clearFrontstagePageSignals(): FrontstageSignalSnapshot {
  return createFrontstageSignalSnapshot();
}

function signalKey(address: FrontstageSignalAddress): string {
  const owner = address.scope === 'page' ? 'page' : address.tab_id;
  return `${address.scope}:${owner}:${address.block_id}:${address.output}`;
}

function failure(
  snapshot: FrontstageSignalSnapshot,
  error: string
): FrontstageSignalCommitResult {
  return { ok: false, snapshot, error };
}

function validateValue(
  value: unknown,
  schema: Record<string, unknown>
): boolean {
  if (Array.isArray(schema.enum))
    return schema.enum.some((item) => Object.is(item, value));
  switch (schema.type) {
    case 'string':
      return typeof value === 'string';
    case 'integer':
      return typeof value === 'number' && Number.isInteger(value);
    case 'number':
      return typeof value === 'number' && Number.isFinite(value);
    case 'boolean':
      return typeof value === 'boolean';
    case 'array':
      return (
        Array.isArray(value) &&
        value.every((item) => validateValue(item, asSchema(schema.items)))
      );
    case 'object': {
      if (!isRecord(value)) return false;
      const properties = asSchema(schema.properties);
      for (const name of stringArray(schema.required)) {
        if (!Object.hasOwn(value, name)) return false;
      }
      return Object.entries(value).every(
        ([name, item]) =>
          !properties[name] || validateValue(item, asSchema(properties[name]))
      );
    }
    default:
      return isJsonValue(value);
  }
}

function isJsonValue(value: unknown): boolean {
  if (value === null || ['string', 'boolean'].includes(typeof value))
    return true;
  if (typeof value === 'number') return Number.isFinite(value);
  if (Array.isArray(value)) return value.every(isJsonValue);
  return isRecord(value) && Object.values(value).every(isJsonValue);
}

function asSchema(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function stringArray(value: unknown): string[] {
  return Array.isArray(value)
    ? value.filter((item): item is string => typeof item === 'string')
    : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}
