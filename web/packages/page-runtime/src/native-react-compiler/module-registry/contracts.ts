export interface NativeReactModuleDefinition {
  module_source: string;
  exports: string[];
}

export function canonicalizeNativeReactModuleDefinitions(
  value: unknown
): NativeReactModuleDefinition[] | null {
  if (!Array.isArray(value)) return null;
  const sources = new Set<string>();
  const definitions: NativeReactModuleDefinition[] = [];
  for (const item of value) {
    if (
      !isRecord(item) ||
      !isNonEmptyString(item.module_source) ||
      !Array.isArray(item.exports) ||
      !item.exports.every(isNonEmptyString) ||
      new Set(item.exports).size !== item.exports.length ||
      sources.has(item.module_source)
    ) {
      return null;
    }
    sources.add(item.module_source);
    definitions.push({
      module_source: item.module_source,
      exports: [...item.exports]
    });
  }
  return definitions;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.length > 0;
}
