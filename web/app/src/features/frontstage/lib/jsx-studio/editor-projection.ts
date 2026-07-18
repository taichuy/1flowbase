import type {
  ConsoleFrontstageDataCapabilities,
  ConsoleFrontstageDataCapabilityDescriptor,
  ConsoleFrontstageDataCapabilityModel
} from '@1flowbase/api-client';
import type { FrontendBlockMonacoExtraLib } from '@1flowbase/page-protocol';

import type { NormalizedFrontstageBlockCatalogEntry } from '../block-catalog';
import type { FrontstageBlockInstance } from '../page-document';
import {
  readFrontstageBlockDataBindings,
  type FrontstageBlockDataBinding
} from './block-data-binding';

export interface FrontstageJsxEditorBinding
  extends FrontstageBlockDataBinding {
  descriptor: ConsoleFrontstageDataCapabilityDescriptor;
  model: ConsoleFrontstageDataCapabilityModel;
  paramsTypeName: string;
  resultTypeName: string;
}

export interface FrontstageJsxEditorProjection {
  bindings: FrontstageJsxEditorBinding[];
  components: string[];
  prelude: string;
  monacoExtraLibs: FrontendBlockMonacoExtraLib[];
}

export function createFrontstageJsxEditorProjection({
  block,
  catalogEntry,
  dataCapabilities
}: {
  block: FrontstageBlockInstance;
  catalogEntry: NormalizedFrontstageBlockCatalogEntry | null;
  dataCapabilities: ConsoleFrontstageDataCapabilities | null;
}): FrontstageJsxEditorProjection {
  const catalogExtraLibs = catalogEntry?.codeCapabilities?.monacoExtraLibs ?? [];
  const components = collectCatalogComponents(catalogExtraLibs);
  const bindings = resolveEditorBindings(block, dataCapabilities);
  const generatedTypes = createBindingTypeDeclarations(bindings);

  return {
    bindings,
    components,
    prelude: createVisiblePrelude(bindings, components),
    monacoExtraLibs: [
      ...catalogExtraLibs,
      ...(generatedTypes
        ? [
            {
              filePath: `file:///frontstage/generated/${block.id}-bindings.d.ts`,
              content: generatedTypes
            }
          ]
        : [])
    ]
  };
}

export function createFrontstageJsxBindingSnippet(
  binding: FrontstageJsxEditorBinding
): string {
  const method = binding.kind === 'query'
    ? 'ctx.data.query'
    : 'ctx.actions.invoke';
  const serializedParams = serializeParams(binding.params);

  return `const ${binding.key} = await ${method}('${escapeSingleQuote(binding.id)}', ${serializedParams});`;
}

function resolveEditorBindings(
  block: FrontstageBlockInstance,
  dataCapabilities: ConsoleFrontstageDataCapabilities | null
): FrontstageJsxEditorBinding[] {
  if (!dataCapabilities) {
    return [];
  }

  const descriptorsById = new Map(
    [...dataCapabilities.queries, ...dataCapabilities.actions].map(
      (descriptor) => [descriptor.id, descriptor]
    )
  );
  const modelsByCode = new Map(
    dataCapabilities.models.map((model) => [model.code, model])
  );

  return readFrontstageBlockDataBindings(block.props).flatMap((binding) => {
    const descriptor = descriptorsById.get(binding.id);
    const model = modelsByCode.get(binding.params.model);
    if (!descriptor || descriptor.kind !== binding.kind || !model) {
      return [];
    }

    const typeBaseName = toPascalCase(binding.key) || 'BoundCapability';
    return [
      {
        ...binding,
        descriptor,
        model,
        paramsTypeName: `${typeBaseName}Params`,
        resultTypeName: `${typeBaseName}Result`
      }
    ];
  });
}

function collectCatalogComponents(
  extraLibs: readonly FrontendBlockMonacoExtraLib[]
): string[] {
  const names = new Set<string>();
  const componentPattern = /export\s+(?:declare\s+)?const\s+([A-Z][A-Za-z0-9_$]*)\b/g;

  for (const extraLib of extraLibs) {
    for (const match of extraLib.content.matchAll(componentPattern)) {
      if (match[1]) {
        names.add(match[1]);
      }
    }
  }

  return [...names].sort((left, right) => left.localeCompare(right));
}

function createVisiblePrelude(
  bindings: readonly FrontstageJsxEditorBinding[],
  components: readonly string[]
): string {
  const lines = [
    '/**',
    ' * @1flowbase 自动注入上下文（只读，不写入区块源码）',
    ' * ctx: currentUser, workspace, application, page, params, props, state, data, actions, events, theme, ui',
    ` * 可用组件: ${components.length > 0 ? components.join(', ') : '无'}`
  ];

  if (bindings.length === 0) {
    lines.push(' * 已绑定接口: 无');
  } else {
    lines.push(' * 已绑定接口:');
    for (const binding of bindings) {
      lines.push(` * ${binding.key}: ${createFrontstageJsxBindingSnippet(binding)}`);
    }
  }

  lines.push(' */');
  return lines.join('\n');
}

function createBindingTypeDeclarations(
  bindings: readonly FrontstageJsxEditorBinding[]
): string {
  if (bindings.length === 0) {
    return '';
  }

  const models = new Map<string, ConsoleFrontstageDataCapabilityModel>();
  for (const binding of bindings) {
    models.set(binding.model.code, binding.model);
  }

  const lines: string[] = [];
  for (const model of models.values()) {
    lines.push(createModelRecordDeclaration(model), '');
  }

  for (const binding of bindings) {
    const recordTypeName = `${toPascalCase(binding.model.code)}Record`;
    lines.push(
      `declare type ${binding.paramsTypeName} = ${schemaToTypeScript(binding.descriptor.params_schema, {
        modelCode: binding.model.code,
        recordTypeName,
        rootKind: 'params'
      })};`,
      `declare type ${binding.resultTypeName} = ${schemaToTypeScript(binding.descriptor.result_schema, {
        modelCode: binding.model.code,
        recordTypeName,
        rootKind: 'result'
      })};`,
      ''
    );
  }

  const queryBindings = bindings.filter((binding) => binding.kind === 'query');
  const actionBindings = bindings.filter((binding) => binding.kind === 'action');
  lines.push("declare module '@1flowbase/block-sdk' {");
  if (queryBindings.length > 0) {
    lines.push('  interface BlockContextDataAccess {');
    for (const binding of queryBindings) {
      lines.push(
        `    query(queryId: '${escapeSingleQuote(binding.id)}', params: ${binding.paramsTypeName}): Promise<${binding.resultTypeName}>;`
      );
    }
    lines.push('  }');
  }
  if (actionBindings.length > 0) {
    lines.push('  interface BlockContextActions {');
    for (const binding of actionBindings) {
      lines.push(
        `    invoke(actionId: '${escapeSingleQuote(binding.id)}', params: ${binding.paramsTypeName}): Promise<${binding.resultTypeName}>;`
      );
    }
    lines.push('  }');
  }
  lines.push('}');

  return lines.join('\n');
}

function createModelRecordDeclaration(
  model: ConsoleFrontstageDataCapabilityModel
): string {
  const recordTypeName = `${toPascalCase(model.code)}Record`;
  const lines = [`declare interface ${recordTypeName} {`];

  for (const field of model.fields) {
    const optional = field.is_required ? '' : '?';
    lines.push(
      `  ${quotePropertyIfNeeded(field.code)}${optional}: ${fieldKindToTypeScript(field.field_kind)};`
    );
  }

  lines.push('}');
  return lines.join('\n');
}

function schemaToTypeScript(
  schema: unknown,
  context: {
    modelCode: string;
    recordTypeName: string;
    rootKind: 'params' | 'result';
    propertyName?: string;
  }
): string {
  if (!isRecord(schema)) {
    return 'unknown';
  }

  if (context.rootKind === 'params' && context.propertyName === 'model') {
    return `'${escapeSingleQuote(context.modelCode)}'`;
  }
  if (context.rootKind === 'result' && context.propertyName === 'record') {
    return context.recordTypeName;
  }
  if (
    context.rootKind === 'result' &&
    context.propertyName === 'items' &&
    schema.type === 'array'
  ) {
    return `${context.recordTypeName}[]`;
  }

  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return schema.enum.map(toTypeScriptLiteral).join(' | ');
  }

  switch (schema.type) {
    case 'string':
      return 'string';
    case 'integer':
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'array':
      return `${schemaToTypeScript(schema.items, context)}[]`;
    case 'object': {
      if (!isRecord(schema.properties)) {
        return 'Record<string, unknown>';
      }
      const required = new Set(
        Array.isArray(schema.required)
          ? schema.required.filter((value): value is string => typeof value === 'string')
          : []
      );
      const fields = Object.entries(schema.properties).map(([key, value]) => {
        const optional = required.has(key) ? '' : '?';
        return `${quotePropertyIfNeeded(key)}${optional}: ${schemaToTypeScript(value, {
          ...context,
          propertyName: key
        })}`;
      });
      return fields.length > 0 ? `{ ${fields.join('; ')} }` : 'Record<string, unknown>';
    }
    default:
      return 'unknown';
  }
}

function fieldKindToTypeScript(fieldKind: string): string {
  switch (fieldKind) {
    case 'number':
      return 'number';
    case 'boolean':
      return 'boolean';
    case 'one_to_many':
    case 'many_to_many':
      return 'Record<string, unknown>[]';
    case 'json':
    case 'many_to_one':
      return 'Record<string, unknown>';
    default:
      return 'string';
  }
}

function serializeParams(params: Record<string, unknown>): string {
  const entries = Object.entries(params).map(
    ([key, value]) => `${quotePropertyIfNeeded(key)}: ${toJavaScriptLiteral(value)}`
  );
  return `{ ${entries.join(', ')} }`;
}

function toJavaScriptLiteral(value: unknown): string {
  if (typeof value === 'string') {
    return `'${escapeSingleQuote(value)}'`;
  }
  if (typeof value === 'number' && Number.isFinite(value)) {
    return String(value);
  }
  if (typeof value === 'boolean') {
    return String(value);
  }
  if (value === null) {
    return 'null';
  }
  return JSON.stringify(value) ?? 'undefined';
}

function toTypeScriptLiteral(value: unknown): string {
  return typeof value === 'string'
    ? `'${escapeSingleQuote(value)}'`
    : JSON.stringify(value) ?? 'unknown';
}

function quotePropertyIfNeeded(value: string): string {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(value)
    ? value
    : `'${escapeSingleQuote(value)}'`;
}

function toPascalCase(value: string): string {
  return value
    .split(/[^A-Za-z0-9_$]+/)
    .filter(Boolean)
    .map((part) => `${part.charAt(0).toUpperCase()}${part.slice(1)}`)
    .join('');
}

function escapeSingleQuote(value: string): string {
  return value.replaceAll('\\', '\\\\').replaceAll("'", "\\'");
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}
