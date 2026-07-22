import type { ConsoleFrontstageInterfaceCapability } from '@1flowbase/api-client';

import type { FrontstageJsxRequiredImport } from './source-insertion';

export interface FrontstageOpenApiCodegenResult {
  source: string;
  requiredImports: FrontstageJsxRequiredImport[];
}

type JsonSchema = Record<string, unknown>;

const sourcePolicyDeniedPropertyNames = new Set([
  'window',
  'document',
  'globalThis',
  'self',
  'localStorage',
  'sessionStorage',
  'cookie',
  'constructor',
  'prototype',
  '__proto__'
]);
const reservedIdentifierNames = new Set([
  'break',
  'case',
  'catch',
  'class',
  'const',
  'continue',
  'debugger',
  'default',
  'delete',
  'do',
  'else',
  'enum',
  'export',
  'extends',
  'false',
  'finally',
  'for',
  'function',
  'if',
  'import',
  'in',
  'instanceof',
  'new',
  'null',
  'return',
  'super',
  'switch',
  'this',
  'throw',
  'true',
  'try',
  'typeof',
  'var',
  'void',
  'while',
  'with',
  'yield'
]);

export function generateFrontstageInterfaceSource(
  operation: ConsoleFrontstageInterfaceCapability,
  existingSource = ''
): FrontstageOpenApiCodegenResult {
  if (!operation.bindable) {
    throw new Error(operation.disabled_reason ?? 'Operation is not bindable.');
  }
  const functionName = createCallableName(operation, existingSource);
  const request = asSchema(operation.parameter_schema);
  const requestProperties = asRecord(request.properties);
  const requestRequired = stringSet(request.required);
  const locations = ['path', 'query', 'headers', 'body'].filter(
    (location) => requestProperties[location] !== undefined
  );

  const responseType = isNoContentResponse(operation)
    ? 'void'
    : isBinaryMediaType(operation.response_media_type)
      ? renderBinaryResource()
      : renderSchemaType(operation.result_schema);

  const parameters: Array<{ source: string; required: boolean }> = [];
  const requestFields: string[] = [];
  const parameterNames = new Set(['ctx']);
  const pathSchema = asSchema(requestProperties.path);
  const pathProperties = asRecord(pathSchema.properties);
  const requiredPathProperties = stringSet(pathSchema.required);
  const pathFields: string[] = [];
  for (const [propertyName, propertySchema] of Object.entries(pathProperties)) {
    const parameterName = uniqueParameterName(
      toCamelCase(propertyName),
      parameterNames
    );
    parameters.push({
      source: `${parameterName}: ${renderSchemaType(propertySchema, 1)}`,
      required:
        requestRequired.has('path') && requiredPathProperties.has(propertyName)
    });
    pathFields.push(`${quoteProperty(propertyName)}: ${parameterName}`);
  }
  if (pathFields.length > 0) {
    requestFields.push(`path: { ${pathFields.join(', ')} }`);
  }
  for (const location of locations.filter((value) => value !== 'path')) {
    const required = requestRequired.has(location);
    const parameterName = uniqueParameterName(location, parameterNames);
    const schemaType = renderSchemaType(requestProperties[location], 1);
    const defaultValue =
      !required && isObjectSchema(requestProperties[location]) ? ' = {}' : '';
    parameters.push({
      source: `${parameterName}${!required && !defaultValue ? '?' : ''}: ${schemaType}${defaultValue}`,
      required
    });
    requestFields.push(
      parameterName === location ? location : `${location}: ${parameterName}`
    );
  }
  parameters.sort(
    (left, right) => Number(right.required) - Number(left.required)
  );

  let parameterSource = '';
  let requestSource: string | null;
  if (parameters.length === 0) {
    requestSource = null;
  } else {
    parameterSource = `,\n${parameters.map((parameter) => `  ${parameter.source}`).join(',\n')}`;
    requestSource = `{ ${requestFields.join(', ')} }`;
  }

  const isStream = operation.response_media_type === 'text/event-stream';
  const method = operation.method.toUpperCase();
  if (
    !['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'].includes(
      method
    )
  ) {
    throw new Error(`Unsupported HTTP method: ${method}.`);
  }
  const methodName = method.toLowerCase();
  const escapedPath = escapeSingleQuote(operation.path);
  const callSource = isStream
    ? requestSource
      ? `ctx.api.stream(\n    '${escapeSingleQuote(method)}',\n    '${escapedPath}',\n    ${requestSource}\n  )`
      : `ctx.api.stream('${escapeSingleQuote(method)}', '${escapedPath}')`
    : requestSource
      ? `ctx.api.${methodName}(\n    '${escapedPath}',\n    ${requestSource}\n  )`
      : `ctx.api.${methodName}('${escapedPath}')`;
  const callable = [
    `const ${functionName} = (`,
    `  ctx: BlockContext${parameterSource}`,
    `): ${isStream ? `AsyncIterable<${responseType}>` : `Promise<${responseType}>`} =>`,
    `  ${callSource};`
  ].join('\n');

  return {
    source: callable,
    requiredImports: [
      {
        kind: 'type',
        name: 'BlockContext',
        moduleSource: '@1flowbase/block-sdk'
      }
    ]
  };
}

function renderSchemaType(value: unknown, indent = 0): string {
  const schema = asSchema(value);
  if (schema.format === 'binary') {
    return renderBinaryInput(indent);
  }
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return schema.enum.map(toLiteral).join(' | ');
  }
  if ('const' in schema) {
    return toLiteral(schema.const);
  }
  if (Array.isArray(schema.allOf)) {
    return schema.allOf
      .map((item) => renderSchemaType(item, indent))
      .join(' & ');
  }
  if (Array.isArray(schema.anyOf)) {
    return schema.anyOf
      .map((item) => renderSchemaType(item, indent))
      .join(' | ');
  }
  if (Array.isArray(schema.oneOf)) {
    return schema.oneOf
      .map((item) => renderSchemaType(item, indent))
      .join(' | ');
  }
  if (schema.type === 'array') {
    return `${renderSchemaType(schema.items, indent)}[]`;
  }
  if (Array.isArray(schema.type)) {
    return schema.type
      .map((type) => renderSchemaType({ ...schema, type }, indent))
      .join(' | ');
  }
  if (schema.type === 'object' || isRecord(schema.properties)) {
    const properties = asRecord(schema.properties);
    const required = stringSet(schema.required);
    const fields = Object.entries(properties).map(([key, child]) => {
      return `${'  '.repeat(indent + 1)}${quoteProperty(key)}${required.has(key) ? '' : '?'}: ${renderSchemaType(child, indent + 1)};`;
    });
    if (fields.length === 0 && schema.additionalProperties) {
      return schema.additionalProperties === true
        ? 'Record<string, unknown>'
        : `Record<string, ${renderSchemaType(schema.additionalProperties, indent)}>`;
    }
    return ['{', ...fields, `${'  '.repeat(indent)}}`].join('\n');
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

function renderBinaryInput(indent: number): string {
  const fieldIndent = '  '.repeat(indent + 1);
  return [
    '{',
    `${fieldIndent}base64: string;`,
    `${fieldIndent}file_name?: string;`,
    `${fieldIndent}content_type?: string;`,
    `${'  '.repeat(indent)}}`
  ].join('\n');
}

function renderBinaryResource(): string {
  return [
    '{',
    '  bytes: Uint8Array;',
    '  file_name: string | null;',
    '  content_type: string;',
    '}'
  ].join('\n');
}

function isNoContentResponse(
  operation: ConsoleFrontstageInterfaceCapability
): boolean {
  const schema = asSchema(operation.result_schema);
  return (
    operation.response_media_type === null && Object.keys(schema).length === 0
  );
}

function isBinaryMediaType(mediaType: string | null): boolean {
  if (mediaType === null || mediaType === 'text/event-stream') return false;
  const normalized = mediaType.split(';', 1)[0]?.trim().toLocaleLowerCase();
  return normalized !== 'application/json' && !normalized?.endsWith('+json');
}

function requireIdentifier(value: string): string {
  const identifier = value.trim();
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(identifier)) {
    throw new Error('Function name must be a TypeScript identifier.');
  }
  return identifier;
}

function createCallableName(
  operation: ConsoleFrontstageInterfaceCapability,
  existingSource: string
): string {
  const baseName =
    createRuntimeDataModelCallableName(operation.path) ??
    (operation.name === operation.interface_id
      ? null
      : createIdentifierFromWords(operation.name)) ??
    createRouteCallableName(operation.method, operation.path);
  let candidate = baseName;
  let suffix = 2;
  while (sourceContainsIdentifier(existingSource, candidate)) {
    candidate = `${baseName}${suffix}`;
    suffix += 1;
  }
  return candidate;
}

function createRuntimeDataModelCallableName(path: string): string | null {
  const match = path.match(
    /^\/api\/runtime\/models\/([^/]+)\/(list|create|get|update|delete)(?:\/\{[^/{}]+\})?$/
  );
  if (!match) return null;
  const resourceName = createPascalIdentifier(match[1] ?? '');
  const action = match[2];
  if (!resourceName || !action) return null;
  return action === 'list'
    ? `list${resourceName}`
    : `${action}${resourceName}Record`;
}

function createRouteCallableName(method: string, path: string): string {
  const routeWords = path
    .split('/')
    .filter(
      (part) =>
        part.length > 0 &&
        part !== 'api' &&
        part !== 'console' &&
        part !== 'runtime'
    )
    .flatMap((part) =>
      part.startsWith('{') && part.endsWith('}')
        ? ['by', part.slice(1, -1)]
        : [part]
    );
  return (
    createIdentifierFromWords(
      `${method.toLowerCase()} ${routeWords.join(' ')}`
    ) ?? 'callApi'
  );
}

function createIdentifierFromWords(value: string): string | null {
  const words = value.split(/[^A-Za-z0-9]+/).filter(Boolean);
  if (words.length === 0) return null;
  const candidate = words
    .map((word, index) =>
      index === 0
        ? word.charAt(0).toLowerCase() + word.slice(1)
        : word.charAt(0).toUpperCase() + word.slice(1)
    )
    .join('');
  if (!/^[A-Za-z_$][A-Za-z0-9_$]*$/.test(candidate)) return null;
  return reservedIdentifierNames.has(candidate)
    ? `${candidate}Operation`
    : candidate;
}

function createPascalIdentifier(value: string): string | null {
  const candidate = createIdentifierFromWords(value);
  return candidate
    ? candidate.charAt(0).toUpperCase() + candidate.slice(1)
    : null;
}

function sourceContainsIdentifier(source: string, identifier: string): boolean {
  const escaped = identifier.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`(^|[^A-Za-z0-9_$])${escaped}(?![A-Za-z0-9_$])`).test(
    source
  );
}

function toCamelCase(value: string): string {
  const words = value.split(/[^A-Za-z0-9]+/).filter(Boolean);
  const candidate = words
    .map((word, index) =>
      index === 0
        ? word.charAt(0).toLowerCase() + word.slice(1)
        : word.charAt(0).toUpperCase() + word.slice(1)
    )
    .join('');
  const identifier = requireIdentifier(candidate || 'value');
  return reservedIdentifierNames.has(identifier)
    ? `${identifier}Value`
    : identifier;
}

function uniqueParameterName(value: string, used: Set<string>): string {
  let candidate = value;
  let suffix = 2;
  while (used.has(candidate)) {
    candidate = `${value}${suffix}`;
    suffix += 1;
  }
  used.add(candidate);
  return candidate;
}

function isObjectSchema(value: unknown): boolean {
  const schema = asSchema(value);
  return schema.type === 'object' || isRecord(schema.properties);
}

function quoteProperty(value: string): string {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(value) &&
    !sourcePolicyDeniedPropertyNames.has(value)
    ? value
    : JSON.stringify(value);
}

function toLiteral(value: unknown): string {
  return value === null ||
    ['string', 'number', 'boolean'].includes(typeof value)
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
