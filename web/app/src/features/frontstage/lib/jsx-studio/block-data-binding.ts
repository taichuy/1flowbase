import type { FrontstageBlockInstance } from '../page-document';

export type FrontstageBlockDataBindingKind = 'query' | 'action';

export interface FrontstageBlockDataBinding {
  key: string;
  id: string;
  kind: FrontstageBlockDataBindingKind;
  params: Record<string, unknown> & { model: string };
}

export function readFrontstageBlockDataBindings(
  props: Record<string, unknown>
): FrontstageBlockDataBinding[] {
  if (!Array.isArray(props.dataBinding)) {
    return [];
  }

  const bindings: FrontstageBlockDataBinding[] = [];
  const seenKeys = new Set<string>();

  for (const value of props.dataBinding) {
    if (!isRecord(value) || !isRecord(value.params)) {
      continue;
    }

    const key = readRequiredString(value.key);
    const id = readRequiredString(value.id);
    const model = readRequiredString(value.params.model);
    const kind =
      value.kind === 'query' || value.kind === 'action' ? value.kind : null;

    if (!key || !id || !model || !kind || seenKeys.has(key)) {
      continue;
    }

    seenKeys.add(key);
    bindings.push({
      key,
      id,
      kind,
      params: {
        ...value.params,
        model
      }
    });
  }

  return bindings;
}

export function writeFrontstageBlockDataBindings(
  block: FrontstageBlockInstance,
  bindings: readonly FrontstageBlockDataBinding[]
): FrontstageBlockInstance {
  return {
    ...block,
    props: {
      ...block.props,
      dataBinding: bindings.map((binding) => ({
        key: binding.key,
        id: binding.id,
        kind: binding.kind,
        params: { ...binding.params }
      }))
    }
  };
}

function readRequiredString(value: unknown): string | null {
  return typeof value === 'string' && value.trim().length > 0
    ? value.trim()
    : null;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
