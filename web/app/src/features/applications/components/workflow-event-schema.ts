/** Discover declared fields without guessing object members or array indices. */
export function eventSchemaPointers(schema: unknown): string[] {
  const pointers = new Set<string>();
  const definitions =
    schema && typeof schema === 'object'
      ? (schema as Record<string, unknown>)
      : {};
  const resolve = (ref: string): unknown => {
    if (!ref.startsWith('#/')) return undefined;
    return ref
      .slice(2)
      .split('/')
      .reduce<unknown>(
        (value, key) =>
          value && typeof value === 'object'
            ? (value as Record<string, unknown>)[
                key.replace(/~1/g, '/').replace(/~0/g, '~')
              ]
            : undefined,
        definitions
      );
  };
  const visit = (node: unknown, pointer: string, ancestors: Set<object>) => {
    if (!node || typeof node !== 'object' || ancestors.has(node)) return;
    const next = new Set(ancestors).add(node);
    const value = node as Record<string, unknown>;
    if (typeof value.$ref === 'string')
      visit(resolve(value.$ref), pointer, next);
    for (const key of ['allOf', 'anyOf', 'oneOf']) {
      if (Array.isArray(value[key]))
        for (const child of value[key]) visit(child, pointer, next);
    }
    if (value.properties && typeof value.properties === 'object') {
      for (const [key, child] of Object.entries(value.properties)) {
        const path = `${pointer}/${key.replace(/~/g, '~0').replace(/\//g, '~1')}`;
        pointers.add(path);
        visit(child, path, next);
      }
    }
  };
  // The lifecycle signal wraps source-declared event data in payload.
  visit(schema, '/payload', new Set());
  return [...pointers];
}
