import type { BlockProtocolError } from '@1flowbase/page-protocol';

import {
  JS_BLOCK_ALLOWED_IMPORTS,
  validateJsBlockSource,
  type ValidateJsBlockSourceOptions
} from './js-block-source-policy';
import {
  isIdentifierPart,
  isIdentifierStart,
  isImportName,
  isLocalBindingName,
  isWhitespace
} from './js-block-source-transform/identifiers';

export type JsBlockInjectedModuleSource =
  (typeof JS_BLOCK_ALLOWED_IMPORTS)[number];

export type JsBlockImportBinding =
  | {
      kind: 'named';
      source: JsBlockInjectedModuleSource;
      imported: string;
      local: string;
    }
  | {
      kind: 'default';
      source: JsBlockInjectedModuleSource;
      local: string;
    }
  | {
      kind: 'namespace';
      source: JsBlockInjectedModuleSource;
      local: string;
    };

export interface JsBlockInjectedModule {
  source: JsBlockInjectedModuleSource;
  bindings: JsBlockImportBinding[];
}

export interface JsBlockSourceTransformSuccess {
  ok: true;
  source: string;
  normalizedSource: string;
  injectedModules: JsBlockInjectedModule[];
  importBindings: JsBlockImportBinding[];
  executableBody: string;
  executablePreambleLines: number;
  sourceMap?: unknown;
  moduleMapIdentifier: string;
  defaultExportIdentifier: string;
  errors: [];
}

export interface JsBlockSourceTransformFailure {
  ok: false;
  errors: BlockProtocolError[];
}

export type JsBlockSourceTransformResult =
  | JsBlockSourceTransformSuccess
  | JsBlockSourceTransformFailure;

interface SourceToken {
  value: string;
  start: number;
  end: number;
  depth: number;
}

interface ImportDeclaration {
  source: JsBlockInjectedModuleSource;
  bindings: JsBlockImportBinding[];
  start: number;
  end: number;
}

interface DefaultExportDeclaration {
  start: number;
  end: number;
  expression: string;
}

interface SourceEdit {
  start: number;
  end: number;
  replacement: string;
}

interface StringLiteralValue {
  value: string;
  end: number;
}

interface StatementEnd {
  expressionEnd: number;
  statementEnd: number;
}

interface ParseSuccess<T> {
  ok: true;
  value: T;
}

interface ParseFailure {
  ok: false;
  error: BlockProtocolError;
}

type ParseResult<T> = ParseSuccess<T> | ParseFailure;

const MODULES_IDENTIFIER = '__flowbaseJsBlockModules';
const DEFAULT_EXPORT_IDENTIFIER = '__flowbaseJsBlockDefaultExport';
const RESERVED_TRANSFORM_IDENTIFIERS = new Set([
  MODULES_IDENTIFIER,
  DEFAULT_EXPORT_IDENTIFIER
]);
const allowedImportSources = new Set<string>(JS_BLOCK_ALLOWED_IMPORTS);

export function transformJsBlockSource(
  source: unknown,
  options: ValidateJsBlockSourceOptions = {}
): JsBlockSourceTransformResult {
  const policyResult = validateJsBlockSource(source, options);
  if (!policyResult.ok) {
    return policyResult;
  }

  const tokens = tokenizeSource(policyResult.source);
  const reservedToken = tokens.find((token) =>
    RESERVED_TRANSFORM_IDENTIFIERS.has(token.value)
  );
  if (reservedToken) {
    return transformFailed(
      'source.identifiers',
      `Identifier '${reservedToken.value}' is reserved by the JS block transform.`
    );
  }

  const parsed = parseTopLevelModuleSyntax(policyResult.source, tokens);
  if (!parsed.ok) {
    return { ok: false, errors: [parsed.error] };
  }

  const { imports, defaultExport } = parsed.value;
  const bindingResult = collectInjectedModules(imports);
  if (!bindingResult.ok) {
    return { ok: false, errors: [bindingResult.error] };
  }
  const modulePreamble = createModuleBindingPreamble(
    bindingResult.value.injectedModules
  );
  const executableSource = applyEdits(policyResult.source, [
    ...imports.map((importDeclaration) => ({
      start: importDeclaration.start,
      end: importDeclaration.end,
      replacement: preserveLineBreaks(
        policyResult.source.slice(
          importDeclaration.start,
          importDeclaration.end
        )
      )
    })),
    {
      start: defaultExport.start,
      end: defaultExport.end,
      replacement: `const ${DEFAULT_EXPORT_IDENTIFIER} = ${defaultExport.expression};`
    }
  ]);
  const executableBody = [
    ...modulePreamble,
    executableSource.trimEnd(),
    `return ${DEFAULT_EXPORT_IDENTIFIER};`
  ]
    .filter((line) => line.length > 0)
    .join('\n');

  return {
    ok: true,
    source: policyResult.source,
    normalizedSource: policyResult.normalizedSource,
    injectedModules: bindingResult.value.injectedModules,
    importBindings: bindingResult.value.importBindings,
    executableBody,
    executablePreambleLines: modulePreamble.length,
    moduleMapIdentifier: MODULES_IDENTIFIER,
    defaultExportIdentifier: DEFAULT_EXPORT_IDENTIFIER,
    errors: []
  };
}

function preserveLineBreaks(value: string): string {
  return value.replace(/[^\r\n]/g, ' ');
}

function tokenizeSource(source: string): SourceToken[] {
  const tokens: SourceToken[] = [];
  let index = 0;
  let depth = 0;

  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];

    if (isWhitespace(char)) {
      index += 1;
      continue;
    }

    if (char === '/' && next === '/') {
      index = consumeLineComment(source, index + 2);
      continue;
    }

    if (char === '/' && next === '*') {
      index = consumeBlockComment(source, index);
      continue;
    }

    if (char === '"' || char === "'") {
      index = consumeQuotedString(source, index, char);
      continue;
    }

    if (char === '`') {
      index = consumeTemplate(source, index);
      continue;
    }

    if (isIdentifierStart(char)) {
      const start = index;
      index += 1;
      while (index < source.length && isIdentifierPart(source[index])) {
        index += 1;
      }
      tokens.push({
        value: source.slice(start, index),
        start,
        end: index,
        depth
      });
      continue;
    }

    if (char === '(' || char === '[' || char === '{') {
      depth += 1;
      index += 1;
      continue;
    }

    if (char === ')' || char === ']' || char === '}') {
      depth = Math.max(0, depth - 1);
      index += 1;
      continue;
    }

    index += 1;
  }

  return tokens;
}

function parseTopLevelModuleSyntax(
  source: string,
  tokens: SourceToken[]
): ParseResult<{
  imports: ImportDeclaration[];
  defaultExport: DefaultExportDeclaration;
}> {
  const imports: ImportDeclaration[] = [];
  const defaultExports: DefaultExportDeclaration[] = [];

  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (token.depth !== 0) {
      continue;
    }

    if (token.value === 'import') {
      const importResult = parseImportDeclaration(source, tokens, index);
      if (!importResult.ok) {
        return importResult;
      }
      imports.push(importResult.value);
      continue;
    }

    if (token.value === 'export') {
      const exportResult = parseExportDeclaration(source, tokens, index);
      if (!exportResult.ok) {
        return exportResult;
      }
      defaultExports.push(exportResult.value);
    }
  }

  if (defaultExports.length === 0) {
    return parseError(
      'source.defaultExport',
      'JS block source must include exactly one default export.'
    );
  }

  if (defaultExports.length > 1) {
    return parseError(
      'source.defaultExport',
      'JS block source must not include more than one default export.'
    );
  }

  return {
    ok: true,
    value: {
      imports,
      defaultExport: defaultExports[0]
    }
  };
}

function parseImportDeclaration(
  source: string,
  tokens: SourceToken[],
  importTokenIndex: number
): ParseResult<ImportDeclaration> {
  const importToken = tokens[importTokenIndex];
  const nextIndex = skipWhitespaceAndComments(source, importToken.end);
  const nextChar = source[nextIndex];

  if (nextChar === '"' || nextChar === "'") {
    const literal = readStringLiteral(source, nextIndex);
    if (!literal || !isAllowedImportSource(literal.value)) {
      return parseError(
        'source.imports',
        'JS block import source could not be transformed.'
      );
    }

    const end = readImportDeclarationEnd(source, literal.end);
    if (end === undefined) {
      return parseError(
        'source.imports',
        'JS block import declaration could not be transformed.'
      );
    }

    return {
      ok: true,
      value: {
        source: literal.value,
        bindings: [],
        start: importToken.start,
        end
      }
    };
  }

  const fromToken = findTopLevelTokenBeforeTerminator(
    source,
    tokens,
    importTokenIndex + 1,
    'from'
  );
  if (!fromToken) {
    return parseError(
      'source.imports',
      'JS block import declaration could not be transformed.'
    );
  }

  const literalStart = skipWhitespaceAndComments(source, fromToken.end);
  const literal = readStringLiteral(source, literalStart);
  if (!literal || !isAllowedImportSource(literal.value)) {
    return parseError(
      'source.imports',
      'JS block import source could not be transformed.'
    );
  }

  const end = readImportDeclarationEnd(source, literal.end);
  if (end === undefined) {
    return parseError(
      'source.imports',
      'JS block import declaration could not be transformed.'
    );
  }

  const bindings = parseImportClause(
    source.slice(importToken.end, fromToken.start),
    literal.value
  );
  if (!bindings.ok) {
    return bindings;
  }

  return {
    ok: true,
    value: {
      source: literal.value,
      bindings: bindings.value,
      start: importToken.start,
      end
    }
  };
}

function parseExportDeclaration(
  source: string,
  tokens: SourceToken[],
  exportTokenIndex: number
): ParseResult<DefaultExportDeclaration> {
  const exportToken = tokens[exportTokenIndex];
  const nextToken = tokens
    .slice(exportTokenIndex + 1)
    .find((token) => token.depth === 0 && token.start >= exportToken.end);

  if (!nextToken || nextToken.value !== 'default') {
    return parseError(
      'source.exports',
      'Only a JS block default export can be transformed.'
    );
  }

  const expressionStart = skipWhitespaceAndComments(source, nextToken.end);
  const firstExpressionToken = tokens.find(
    (token) => token.start >= expressionStart
  );
  if (
    firstExpressionToken &&
    firstExpressionToken.depth === 0 &&
    (firstExpressionToken.value === 'function' ||
      firstExpressionToken.value === 'class')
  ) {
    return parseError(
      'source.defaultExport',
      'JS block default export must be an expression.'
    );
  }

  const statementEnd = readDefaultExportStatementEnd(source, expressionStart);
  if (!statementEnd) {
    return parseError(
      'source.defaultExport',
      'JS block default export could not be transformed.'
    );
  }

  const expression = source
    .slice(expressionStart, statementEnd.expressionEnd)
    .trim();
  if (expression.length === 0) {
    return parseError(
      'source.defaultExport',
      'JS block default export expression is required.'
    );
  }

  return {
    ok: true,
    value: {
      start: exportToken.start,
      end: statementEnd.statementEnd,
      expression
    }
  };
}

function parseImportClause(
  clause: string,
  source: JsBlockInjectedModuleSource
): ParseResult<JsBlockImportBinding[]> {
  const trimmed = clause.trim();
  if (trimmed.length === 0) {
    return parseError(
      'source.imports',
      'JS block import bindings are required for this declaration.'
    );
  }

  const commaIndex = findTopLevelComma(trimmed);
  if (commaIndex === -1) {
    return parseSingleImportClause(trimmed, source);
  }

  const defaultClause = trimmed.slice(0, commaIndex).trim();
  const secondaryClause = trimmed.slice(commaIndex + 1).trim();
  const defaultBinding = parseDefaultBinding(defaultClause, source);
  if (!defaultBinding.ok) {
    return defaultBinding;
  }

  const secondaryBindings =
    secondaryClause.startsWith('{') || secondaryClause.startsWith('*')
      ? parseSingleImportClause(secondaryClause, source)
      : parseError(
          'source.imports',
          'JS block import bindings could not be transformed.'
        );
  if (!secondaryBindings.ok) {
    return secondaryBindings;
  }

  return {
    ok: true,
    value: [defaultBinding.value, ...secondaryBindings.value]
  };
}

function parseSingleImportClause(
  clause: string,
  source: JsBlockInjectedModuleSource
): ParseResult<JsBlockImportBinding[]> {
  if (clause.startsWith('{')) {
    return parseNamedImportBindings(clause, source);
  }

  if (clause.startsWith('*')) {
    const namespaceBinding = parseNamespaceBinding(clause, source);
    if (!namespaceBinding.ok) {
      return namespaceBinding;
    }
    return { ok: true, value: [namespaceBinding.value] };
  }

  const defaultBinding = parseDefaultBinding(clause, source);
  if (!defaultBinding.ok) {
    return defaultBinding;
  }
  return { ok: true, value: [defaultBinding.value] };
}

function parseNamedImportBindings(
  clause: string,
  source: JsBlockInjectedModuleSource
): ParseResult<JsBlockImportBinding[]> {
  const trimmed = clause.trim();
  if (!trimmed.endsWith('}')) {
    return parseError(
      'source.imports',
      'JS block named import bindings could not be transformed.'
    );
  }

  const content = trimmed.slice(1, -1).trim();
  if (content.length === 0) {
    return { ok: true, value: [] };
  }

  const bindings: JsBlockImportBinding[] = [];
  const segments = content.split(',');

  for (const segment of segments) {
    const binding = parseNamedImportBinding(segment, source);
    if (!binding.ok) {
      return binding;
    }
    if (binding.value) {
      bindings.push(binding.value);
    }
  }

  return { ok: true, value: bindings };
}

function parseNamedImportBinding(
  segment: string,
  source: JsBlockInjectedModuleSource
): ParseResult<JsBlockImportBinding | undefined> {
  const trimmed = segment.trim();
  if (trimmed.length === 0) {
    return { ok: true, value: undefined };
  }

  const parts = trimmed.split(/\s+/);
  if (parts.length !== 1 && !(parts.length === 3 && parts[1] === 'as')) {
    return parseError(
      'source.imports',
      'JS block named import binding could not be transformed.'
    );
  }

  const imported = parts[0];
  const local = parts.length === 1 ? imported : parts[2];
  if (
    !isImportName(imported) ||
    !isLocalBindingName(local, RESERVED_TRANSFORM_IDENTIFIERS)
  ) {
    return parseError(
      'source.imports',
      'JS block named import binding could not be transformed.'
    );
  }

  return {
    ok: true,
    value: {
      kind: 'named',
      imported,
      local,
      source
    }
  };
}

function parseDefaultBinding(
  clause: string,
  source: JsBlockInjectedModuleSource
): ParseResult<JsBlockImportBinding> {
  const local = clause.trim();
  if (!isLocalBindingName(local, RESERVED_TRANSFORM_IDENTIFIERS)) {
    return parseError(
      'source.imports',
      'JS block default import binding could not be transformed.'
    );
  }

  return {
    ok: true,
    value: {
      kind: 'default',
      local,
      source
    }
  };
}

function parseNamespaceBinding(
  clause: string,
  source: JsBlockInjectedModuleSource
): ParseResult<JsBlockImportBinding> {
  const parts = clause.trim().split(/\s+/);
  if (parts.length !== 3 || parts[0] !== '*' || parts[1] !== 'as') {
    return parseError(
      'source.imports',
      'JS block namespace import binding could not be transformed.'
    );
  }

  const local = parts[2];
  if (!isLocalBindingName(local, RESERVED_TRANSFORM_IDENTIFIERS)) {
    return parseError(
      'source.imports',
      'JS block namespace import binding could not be transformed.'
    );
  }

  return {
    ok: true,
    value: {
      kind: 'namespace',
      local,
      source
    }
  };
}

function collectInjectedModules(imports: ImportDeclaration[]): ParseResult<{
  injectedModules: JsBlockInjectedModule[];
  importBindings: JsBlockImportBinding[];
}> {
  const modules = new Map<JsBlockInjectedModuleSource, JsBlockInjectedModule>();
  const localBindings = new Set<string>();
  const importBindings: JsBlockImportBinding[] = [];

  for (const importDeclaration of imports) {
    let module = modules.get(importDeclaration.source);
    if (!module) {
      module = {
        source: importDeclaration.source,
        bindings: []
      };
      modules.set(importDeclaration.source, module);
    }

    for (const binding of importDeclaration.bindings) {
      if (localBindings.has(binding.local)) {
        return parseError(
          'source.imports',
          `JS block import binding '${binding.local}' is declared more than once.`
        );
      }
      localBindings.add(binding.local);
      module.bindings.push(binding);
      importBindings.push(binding);
    }
  }

  return {
    ok: true,
    value: {
      injectedModules: [...modules.values()],
      importBindings
    }
  };
}

function createModuleBindingPreamble(
  modules: JsBlockInjectedModule[]
): string[] {
  const lines: string[] = [];

  for (const module of modules) {
    const moduleExpression = `${MODULES_IDENTIFIER}[${JSON.stringify(
      module.source
    )}]`;
    const namespaceBindings = module.bindings.filter(
      (
        binding
      ): binding is Extract<JsBlockImportBinding, { kind: 'namespace' }> =>
        binding.kind === 'namespace'
    );
    const defaultBindings = module.bindings.filter(
      (
        binding
      ): binding is Extract<JsBlockImportBinding, { kind: 'default' }> =>
        binding.kind === 'default'
    );
    const namedBindings = module.bindings.filter(
      (binding): binding is Extract<JsBlockImportBinding, { kind: 'named' }> =>
        binding.kind === 'named'
    );

    namespaceBindings.forEach((binding) => {
      lines.push(`const ${binding.local} = ${moduleExpression};`);
    });
    defaultBindings.forEach((binding) => {
      lines.push(`const ${binding.local} = ${moduleExpression}.default;`);
    });
    if (namedBindings.length > 0) {
      lines.push(
        `const { ${namedBindings
          .map(formatNamedBinding)
          .join(', ')} } = ${moduleExpression};`
      );
    }
  }

  return lines;
}

function formatNamedBinding(
  binding: Extract<JsBlockImportBinding, { kind: 'named' }>
): string {
  return binding.imported === binding.local
    ? binding.imported
    : `${binding.imported}: ${binding.local}`;
}

function applyEdits(source: string, edits: SourceEdit[]): string {
  const orderedEdits = [...edits].sort(
    (left, right) => left.start - right.start
  );
  let result = '';
  let cursor = 0;

  orderedEdits.forEach((edit) => {
    result += source.slice(cursor, edit.start);
    result += edit.replacement;
    cursor = edit.end;
  });

  result += source.slice(cursor);
  return result;
}

function findTopLevelTokenBeforeTerminator(
  source: string,
  tokens: SourceToken[],
  startTokenIndex: number,
  tokenValue: string
): SourceToken | undefined {
  const previousToken = tokens[startTokenIndex - 1];
  for (let index = startTokenIndex; index < tokens.length; index += 1) {
    const token = tokens[index];
    const segment = source.slice(previousToken.end, token.start);
    if (segment.includes(';')) {
      return undefined;
    }
    if (token.depth === previousToken.depth && token.value === tokenValue) {
      return token;
    }
  }

  return undefined;
}

function readImportDeclarationEnd(
  source: string,
  start: number
): number | undefined {
  let index = skipHorizontalWhitespace(source, start);

  while (source[index] === '/' && source[index + 1] === '*') {
    index = skipHorizontalWhitespace(
      source,
      consumeBlockComment(source, index)
    );
  }

  if (source[index] === ';') {
    return index + 1;
  }

  if (source[index] === '/' && source[index + 1] === '/') {
    return consumeLineComment(source, index + 2);
  }

  if (
    index >= source.length ||
    source[index] === '\n' ||
    source[index] === '\r'
  ) {
    return index;
  }

  return undefined;
}

function readDefaultExportStatementEnd(
  source: string,
  start: number
): StatementEnd | undefined {
  let index = start;
  let depth = 0;

  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];

    if (char === '/' && next === '/') {
      index = consumeLineComment(source, index + 2);
      continue;
    }

    if (char === '/' && next === '*') {
      index = consumeBlockComment(source, index);
      continue;
    }

    if (char === '"' || char === "'") {
      index = consumeQuotedString(source, index, char);
      continue;
    }

    if (char === '`') {
      index = consumeTemplate(source, index);
      continue;
    }

    if (char === '(' || char === '[' || char === '{') {
      depth += 1;
      index += 1;
      continue;
    }

    if (char === ')' || char === ']' || char === '}') {
      depth = Math.max(0, depth - 1);
      index += 1;
      continue;
    }

    if (char === ';' && depth === 0) {
      return {
        expressionEnd: index,
        statementEnd: index + 1
      };
    }

    if ((char === '\n' || char === '\r') && depth === 0) {
      const trailingIndex = skipWhitespaceAndComments(source, index);
      if (trailingIndex >= source.length) {
        return {
          expressionEnd: index,
          statementEnd: source.length
        };
      }

      return undefined;
    }

    index += 1;
  }

  return {
    expressionEnd: source.length,
    statementEnd: source.length
  };
}

function findTopLevelComma(source: string): number {
  let index = 0;
  let depth = 0;

  while (index < source.length) {
    const char = source[index];

    if (char === '{' || char === '[' || char === '(') {
      depth += 1;
      index += 1;
      continue;
    }

    if (char === '}' || char === ']' || char === ')') {
      depth = Math.max(0, depth - 1);
      index += 1;
      continue;
    }

    if (char === ',' && depth === 0) {
      return index;
    }

    index += 1;
  }

  return -1;
}

function readStringLiteral(
  source: string,
  start: number
): StringLiteralValue | undefined {
  const quote = source[start];
  if (quote !== '"' && quote !== "'") {
    return undefined;
  }

  let index = start + 1;
  let value = '';

  while (index < source.length) {
    const char = source[index];

    if (char === '\\') {
      value += source[index + 1] ?? '';
      index += 2;
      continue;
    }

    if (char === quote) {
      return { value, end: index + 1 };
    }

    value += char;
    index += 1;
  }

  return undefined;
}

function consumeQuotedString(
  source: string,
  start: number,
  quote: '"' | "'"
): number {
  let index = start + 1;

  while (index < source.length) {
    const char = source[index];

    if (char === '\\') {
      index += 2;
      continue;
    }

    if (char === quote) {
      return index + 1;
    }

    index += 1;
  }

  return source.length;
}

function consumeTemplate(source: string, start: number): number {
  let index = start + 1;

  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];

    if (char === '\\') {
      index += 2;
      continue;
    }

    if (char === '`') {
      return index + 1;
    }

    if (char === '$' && next === '{') {
      index = consumeTemplateExpression(source, index + 2);
      continue;
    }

    index += 1;
  }

  return source.length;
}

function consumeTemplateExpression(source: string, start: number): number {
  let index = start;
  let depth = 0;

  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];

    if (char === '/' && next === '/') {
      index = consumeLineComment(source, index + 2);
      continue;
    }

    if (char === '/' && next === '*') {
      index = consumeBlockComment(source, index);
      continue;
    }

    if (char === '"' || char === "'") {
      index = consumeQuotedString(source, index, char);
      continue;
    }

    if (char === '`') {
      index = consumeTemplate(source, index);
      continue;
    }

    if (char === '{' || char === '[' || char === '(') {
      depth += 1;
      index += 1;
      continue;
    }

    if (char === '}' && depth === 0) {
      return index + 1;
    }

    if (char === '}' || char === ']' || char === ')') {
      depth = Math.max(0, depth - 1);
      index += 1;
      continue;
    }

    index += 1;
  }

  return source.length;
}

function consumeLineComment(source: string, start: number): number {
  const lineEnd = source.indexOf('\n', start);
  return lineEnd === -1 ? source.length : lineEnd + 1;
}

function consumeBlockComment(source: string, start: number): number {
  const commentEnd = source.indexOf('*/', start + 2);
  return commentEnd === -1 ? source.length : commentEnd + 2;
}

function skipWhitespaceAndComments(source: string, start: number): number {
  let index = start;

  while (index < source.length) {
    const next = source[index + 1];
    if (isWhitespace(source[index])) {
      index += 1;
      continue;
    }
    if (source[index] === '/' && next === '/') {
      index = consumeLineComment(source, index + 2);
      continue;
    }
    if (source[index] === '/' && next === '*') {
      index = consumeBlockComment(source, index);
      continue;
    }
    break;
  }

  return index;
}

function skipHorizontalWhitespace(source: string, start: number): number {
  let index = start;

  while (source[index] === ' ' || source[index] === '\t') {
    index += 1;
  }

  return index;
}

function isAllowedImportSource(
  source: string
): source is JsBlockInjectedModuleSource {
  return allowedImportSources.has(source);
}

function parseError(path: string, message: string): ParseFailure {
  return {
    ok: false,
    error: createTransformError(path, message)
  };
}

function transformFailed(
  path: string,
  message: string
): JsBlockSourceTransformFailure {
  return {
    ok: false,
    errors: [createTransformError(path, message)]
  };
}

function createTransformError(
  path: string,
  message: string
): BlockProtocolError {
  return { code: 'transform_failed', path, message };
}
