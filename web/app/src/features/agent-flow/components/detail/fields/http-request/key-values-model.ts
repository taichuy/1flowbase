import type { FlowBinding, NamedBindingEntry } from '@1flowbase/flow-schema';

export type HttpRequestKeyValueEntry = Pick<
  NamedBindingEntry,
  'name' | 'value' | 'valueType'
>;

export function namedBindingEntriesFromValue(
  value: unknown
): HttpRequestKeyValueEntry[] {
  if (
    typeof value === 'object' &&
    value !== null &&
    (value as { kind?: unknown }).kind === 'named_bindings' &&
    Array.isArray((value as { value?: unknown }).value)
  ) {
    return (value as { value: unknown[] }).value as HttpRequestKeyValueEntry[];
  }

  return [];
}

export function toNamedBinding(
  entries: HttpRequestKeyValueEntry[]
): FlowBinding {
  return {
    kind: 'named_bindings',
    value: entries.map((entry) => ({
      name: entry.name,
      valueType: entry.valueType,
      value: entry.value ?? { kind: 'templated_text', value: '' }
    }))
  };
}
