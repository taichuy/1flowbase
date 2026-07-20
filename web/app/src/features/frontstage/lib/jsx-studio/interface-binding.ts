import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';

import type {
  FrontstageBlockInstance,
  FrontstageBlockInterfaceBinding
} from '../page-document';

export interface FrontstageResolvedInterfaceBinding {
  binding: FrontstageBlockInterfaceBinding;
  operation: ConsoleFrontstageCallableInterface | null;
  status: 'current' | 'stale' | 'missing';
}

export function bindFrontstageCallableInterface(
  block: FrontstageBlockInstance,
  alias: string,
  operation: ConsoleFrontstageCallableInterface
): FrontstageBlockInstance {
  if (!operation.bindable) {
    throw new Error(
      operation.disabled_reason ?? 'Callable interface is not bindable.'
    );
  }
  const binding: FrontstageBlockInterfaceBinding = {
    alias: requireIdentifier(alias),
    operation_id: operation.operation_id,
    schema_digest: operation.schema_digest,
    scope: operation.scope,
    risk_level: operation.risk_level
  };
  const existing = (block.interfaces ?? []).filter(
    (item) => item.alias !== binding.alias
  );
  return { ...block, interfaces: [...existing, binding] };
}

export function resolveFrontstageInterfaceBindings(
  block: FrontstageBlockInstance,
  catalog: readonly ConsoleFrontstageCallableInterface[]
): FrontstageResolvedInterfaceBinding[] {
  const byOperationId = new Map(
    catalog.map((operation) => [operation.operation_id, operation])
  );
  return (block.interfaces ?? []).map((binding) => {
    const operation = byOperationId.get(binding.operation_id) ?? null;
    return {
      binding,
      operation,
      status: !operation
        ? 'missing'
        : operation.schema_digest === binding.schema_digest
          ? 'current'
          : 'stale'
    };
  });
}

function requireIdentifier(value: string): string {
  const alias = value.trim();
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(alias)) {
    throw new Error('Interface binding alias must be a TypeScript identifier.');
  }
  return alias;
}
