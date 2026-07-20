import type { FrontstageBlockInstance } from '../page-document';
import type {
  FrontstageBlockInputPort,
  FrontstageBlockOutputPort
} from './types';

export interface FrontstageSignalGraphDiagnostic {
  code: 'source_missing' | 'output_missing' | 'schema_incompatible' | 'cycle';
  block_id: string;
  input?: string;
  message: string;
}

export interface FrontstageSignalGraph {
  order: string[];
  dependencies: Map<string, Set<string>>;
  diagnostics: FrontstageSignalGraphDiagnostic[];
}

export function createFrontstageSignalGraph(
  blocks: readonly FrontstageBlockInstance[]
): FrontstageSignalGraph {
  const byId = new Map(blocks.map((block) => [block.id, block]));
  const dependencies = new Map(
    blocks.map((block) => [block.id, new Set<string>()])
  );
  const diagnostics: FrontstageSignalGraphDiagnostic[] = [];

  for (const block of blocks) {
    for (const input of block.ports?.inputs ?? []) {
      if (!input.source) continue;
      const source = byId.get(input.source.block_id);
      if (!source) {
        diagnostics.push(
          issue(
            'source_missing',
            block.id,
            input,
            'Signal source block does not exist.'
          )
        );
        continue;
      }
      const output = source.ports?.outputs.find(
        (candidate) => candidate.name === input.source?.output
      );
      if (!output) {
        diagnostics.push(
          issue(
            'output_missing',
            block.id,
            input,
            'Signal source output does not exist.'
          )
        );
        continue;
      }
      if (!isSignalSchemaAssignable(output, input)) {
        diagnostics.push(
          issue(
            'schema_incompatible',
            block.id,
            input,
            'Signal output schema is not assignable to the input schema.'
          )
        );
        continue;
      }
      dependencies.get(block.id)?.add(source.id);
    }
  }

  const order = topologicalOrder(
    blocks.map((block) => block.id),
    dependencies
  );
  if (order.length !== blocks.length) {
    for (const blockId of blocks
      .map((block) => block.id)
      .filter((id) => !order.includes(id))) {
      diagnostics.push({
        code: 'cycle',
        block_id: blockId,
        message: 'Signal dependency cycle is not allowed.'
      });
    }
  }
  return { order, dependencies, diagnostics };
}

export function isSignalSchemaAssignable(
  output: FrontstageBlockOutputPort,
  input: FrontstageBlockInputPort
): boolean {
  return schemaAssignable(output.schema, input.schema);
}

function schemaAssignable(
  source: Record<string, unknown>,
  target: Record<string, unknown>
): boolean {
  if (typeof target.type !== 'string') return true;
  if (source.type !== target.type) return false;
  if (target.type === 'array') {
    return schemaAssignable(asSchema(source.items), asSchema(target.items));
  }
  if (target.type !== 'object') return true;
  const sourceProperties = asProperties(source.properties);
  const targetProperties = asProperties(target.properties);
  const sourceRequired = stringSet(source.required);
  for (const required of stringSet(target.required)) {
    if (!sourceRequired.has(required) || !sourceProperties[required])
      return false;
  }
  return Object.entries(targetProperties).every(
    ([name, schema]) =>
      !sourceProperties[name] ||
      schemaAssignable(asSchema(sourceProperties[name]), asSchema(schema))
  );
}

function topologicalOrder(
  ids: string[],
  dependencies: Map<string, Set<string>>
): string[] {
  const pending = new Map(
    [...dependencies].map(([id, deps]) => [id, new Set(deps)])
  );
  const order: string[] = [];
  while (pending.size > 0) {
    const ready = ids.filter(
      (id) => pending.has(id) && pending.get(id)?.size === 0
    );
    if (ready.length === 0) break;
    for (const id of ready) {
      order.push(id);
      pending.delete(id);
      for (const deps of pending.values()) deps.delete(id);
    }
  }
  return order;
}

function issue(
  code: FrontstageSignalGraphDiagnostic['code'],
  blockId: string,
  input: FrontstageBlockInputPort,
  message: string
): FrontstageSignalGraphDiagnostic {
  return { code, block_id: blockId, input: input.name, message };
}

function asSchema(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function asProperties(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function stringSet(value: unknown): Set<string> {
  return new Set(
    Array.isArray(value)
      ? value.filter((item): item is string => typeof item === 'string')
      : []
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
