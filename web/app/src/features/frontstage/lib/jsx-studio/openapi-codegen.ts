import type { ConsoleFrontstageCallableInterface } from '@1flowbase/api-client';

export interface FrontstageOpenApiCodegenResult {
  source: string;
  function_name: string;
  schema_digest: string;
}

type JsonSchema = Record<string, unknown>;

export function generateFrontstageCallableSource(
  operation: ConsoleFrontstageCallableInterface,
  bindingAlias: string
): FrontstageOpenApiCodegenResult {
  if (!operation.bindable) {
    throw new Error(operation.disabled_reason ?? 'Operation is not bindable.');
  }
  const functionName = requireIdentifier(bindingAlias);
  const typeBase = toPascalCase(functionName) || 'BoundInterface';
  const declarations: string[] = [];
  const request = asSchema(operation.request_schema);
  const requestProperties = asRecord(request.properties);
  const requestRequired = stringSet(request.required);
  const locations = ['path', 'query', 'headers', 'body'].filter(
    (location) => requestProperties[location] !== undefined
  );

  const locationTypes = new Map<string, string>();
  for (const location of locations) {
    const name = `${typeBase}${toPascalCase(location)}`;
    locationTypes.set(
      location,
      declareSchema(name, requestProperties[location], declarations)
    );
  }
  const responseName = `${typeBase}Response`;
  const responseType = declareSchema(
    responseName,
    operation.response_schema,
    declarations
  );

  let parameterSource: string;
  let requestSource: string;
  if (locations.length === 0) {
    parameterSource = '';
    requestSource = '';
  } else if (locations.length === 1 && locations[0] === 'query') {
    const queryType = locationTypes.get('query') as string;
    const optional = !requestRequired.has('query');
    parameterSource = `,\n  query: ${queryType}${optional ? ' = {}' : ''}`;
    requestSource = ',\n    { query }';
  } else {
    const requestName = `${typeBase}Request`;
    declarations.push(
      [
        `interface ${requestName} {`,
        ...locations.map((location) => {
          const optional = requestRequired.has(location) ? '' : '?';
          return `  ${location}${optional}: ${locationTypes.get(location)};`;
        }),
        '}'
      ].join('\n')
    );
    const allOptional = locations.every(
      (location) => !requestRequired.has(location)
    );
    parameterSource = `,\n  request: ${requestName}${allOptional ? ' = {}' : ''}`;
    requestSource = ',\n    request';
  }

  const provenance = [
    '/**',
    ' * @1flowbase-openapi',
    ` * operationId=${operation.operation_id}`,
    ` * binding=${functionName}`,
    ` * ${operation.method.toUpperCase()} ${operation.path}`,
    ` * specDigest=${operation.schema_digest}`,
    ' */'
  ].join('\n');
  const callable = [
    `async function ${functionName}(`,
    `  ctx: BlockContext${parameterSource}`,
    `): Promise<${responseType}> {`,
    `  return ctx.interfaces.call<${responseType}>(`,
    `    '${escapeSingleQuote(functionName)}'${requestSource}`,
    '  );',
    '}'
  ].join('\n');

  return {
    source: [provenance, ...declarations, callable].join('\n\n'),
    function_name: functionName,
    schema_digest: operation.schema_digest
  };
}

function declareSchema(
  preferredName: string,
  value: unknown,
  declarations: string[]
): string {
  const schema = asSchema(value);
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return schema.enum.map(toLiteral).join(' | ');
  }
  if (Array.isArray(schema.oneOf)) {
    return schema.oneOf
      .map((item, index) =>
        declareSchema(`${preferredName}Variant${index + 1}`, item, declarations)
      )
      .join(' | ');
  }
  if (schema.type === 'array') {
    return `${declareSchema(singularize(preferredName), schema.items, declarations)}[]`;
  }
  if (schema.type === 'object' || isRecord(schema.properties)) {
    const name = schemaTitle(schema) ?? preferredName;
    const properties = asRecord(schema.properties);
    const required = stringSet(schema.required);
    const fields = Object.entries(properties).map(([key, child]) => {
      const childName = `${name}${toPascalCase(singularize(key))}`;
      return `  ${quoteProperty(key)}${required.has(key) ? '' : '?'}: ${declareSchema(childName, child, declarations)};`;
    });
    if (fields.length === 0 && schema.additionalProperties) {
      return 'Record<string, unknown>';
    }
    if (!declarations.some((item) => item.startsWith(`interface ${name} `))) {
      declarations.push([`interface ${name} {`, ...fields, '}'].join('\n'));
    }
    return name;
  }

  const base =
    schema.type === 'string'
      ? 'string'
      : schema.type === 'integer' || schema.type === 'number'
        ? 'number'
        : schema.type === 'boolean'
          ? 'boolean'
          : schema.type === 'null'
            ? 'null'
            : 'unknown';
  return schema.nullable === true && base !== 'null' ? `${base} | null` : base;
}

function schemaTitle(schema: JsonSchema): string | null {
  return typeof schema.title === 'string' && schema.title.trim()
    ? toPascalCase(schema.title)
    : null;
}

function requireIdentifier(value: string): string {
  const identifier = value.trim();
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(identifier)) {
    throw new Error('Binding alias must be a TypeScript identifier.');
  }
  return identifier;
}

function toPascalCase(value: string): string {
  return value
    .replace(/([a-z0-9])([A-Z])/g, '$1 $2')
    .split(/[^A-Za-z0-9]+/)
    .filter(Boolean)
    .map((part) => part[0].toUpperCase() + part.slice(1))
    .join('');
}

function singularize(value: string): string {
  return value.endsWith('s') && value.length > 1 ? value.slice(0, -1) : value;
}

function quoteProperty(value: string): string {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(value)
    ? value
    : JSON.stringify(value);
}

function toLiteral(value: unknown): string {
  return value === null || ['string', 'number', 'boolean'].includes(typeof value)
    ? (JSON.stringify(value) ?? 'unknown')
    : 'unknown';
}

function escapeSingleQuote(value: string): string {
  return value.replace(/\\/g, '\\\\').replace(/'/g, "\\'");
}

function stringSet(value: unknown): Set<string> {
  return new Set(
    Array.isArray(value)
      ? value.filter((item): item is string => typeof item === 'string')
      : []
  );
}

function asSchema(value: unknown): JsonSchema {
  return isRecord(value) ? value : {};
}

function asRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : {};
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
