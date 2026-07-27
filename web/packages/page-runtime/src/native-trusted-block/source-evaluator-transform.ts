import { NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS } from '../native-trusted-block-source-policy';
import {
  NATIVE_REACT_JSX_RUNTIME_IMPORT_SOURCE,
  type DefaultExportDeclaration,
  type ImportDeclaration,
  type NativeTrustedBlockImportBinding,
  type NativeTrustedBlockInjectedModule,
  type NativeTrustedBlockInjectedModuleSource,
  type ParseFailure,
  type ParseResult,
  type SourceEdit,
  type SourceToken,
  type StatementEnd,
  type StringLiteralValue
} from './source-evaluator-types';

export const MODULES_IDENTIFIER = '__flowbaseNativeTrustedBlockModules';
export const DEFAULT_EXPORT_IDENTIFIER =
  '__flowbaseNativeTrustedBlockDefaultExport';
export const RESERVED_TRANSFORM_IDENTIFIERS = new Set([
  MODULES_IDENTIFIER,
  DEFAULT_EXPORT_IDENTIFIER
]);

const allowedImportSources = new Set<string>([
  ...NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
  NATIVE_REACT_JSX_RUNTIME_IMPORT_SOURCE
]);
const localBindingIdentifiers = new Set<string>([
  'as',
  'async',
  'await',
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
  'export',
  'extends',
  'false',
  'finally',
  'for',
  'from',
  'function',
  'if',
  'import',
  'in',
  'instanceof',
  'let',
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
  'undefined',
  'var',
  'void',
  'while',
  'with',
  'yield'
]);

export function tokenizeSource(source: string): SourceToken[] {
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

export function parseTopLevelModuleSyntax(
  source: string,
  tokens: SourceToken[],
  acceptedImportSources: ReadonlySet<string> = allowedImportSources
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
      const importResult = parseImportDeclaration(
        source,
        tokens,
        index,
        acceptedImportSources
      );
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
      'Native trusted block source must include exactly one default export.'
    );
  }

  if (defaultExports.length > 1) {
    return parseError(
      'source.defaultExport',
      'Native trusted block source must not include more than one default export.'
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

export function collectInjectedModules(
  imports: ImportDeclaration[]
): ParseResult<{
  injectedModules: NativeTrustedBlockInjectedModule[];
  importBindings: NativeTrustedBlockImportBinding[];
}> {
  const modules = new Map<
    NativeTrustedBlockInjectedModuleSource,
    NativeTrustedBlockInjectedModule
  >();
  const localBindings = new Set<string>();
  const importBindings: NativeTrustedBlockImportBinding[] = [];

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
          `Native trusted block import binding '${binding.local}' is declared more than once.`
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

export function createModuleBindingPreamble(
  modules: NativeTrustedBlockInjectedModule[]
): string[] {
  const lines: string[] = [];

  for (const module of modules) {
    const moduleExpression = `${MODULES_IDENTIFIER}[${JSON.stringify(
      module.source
    )}]`;
    const namespaceBindings = module.bindings.filter(
      (
        binding
      ): binding is Extract<
        NativeTrustedBlockImportBinding,
        { kind: 'namespace' }
      > => binding.kind === 'namespace'
    );
    const defaultBindings = module.bindings.filter(
      (
        binding
      ): binding is Extract<
        NativeTrustedBlockImportBinding,
        { kind: 'default' }
      > => binding.kind === 'default'
    );
    const namedBindings = module.bindings.filter(
      (
        binding
      ): binding is Extract<
        NativeTrustedBlockImportBinding,
        { kind: 'named' }
      > => binding.kind === 'named'
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

export function findReactJsxRuntimeIdentifier(
  bindings: NativeTrustedBlockImportBinding[]
): string | undefined {
  return bindings.find(
    (binding) =>
      binding.source === 'react' &&
      (binding.kind === 'default' || binding.kind === 'namespace')
  )?.local;
}

export function applyEdits(source: string, edits: SourceEdit[]): string {
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

function parseImportDeclaration(
  source: string,
  tokens: SourceToken[],
  importTokenIndex: number,
  acceptedImportSources: ReadonlySet<string>
): ParseResult<ImportDeclaration> {
  const importToken = tokens[importTokenIndex];
  const nextIndex = skipWhitespaceAndComments(source, importToken.end);
  const nextChar = source[nextIndex];

  if (nextChar === '"' || nextChar === "'") {
    const literal = readStringLiteral(source, nextIndex);
    if (!literal || !acceptedImportSources.has(literal.value)) {
      return parseError(
        'source.imports',
        'Native trusted block import source could not be transformed.'
      );
    }

    const end = readImportDeclarationEnd(source, literal.end);
    if (end === undefined) {
      return parseError(
        'source.imports',
        'Native trusted block import declaration could not be transformed.'
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
      'Native trusted block import declaration could not be transformed.'
    );
  }

  const literalStart = skipWhitespaceAndComments(source, fromToken.end);
  const literal = readStringLiteral(source, literalStart);
  if (!literal || !acceptedImportSources.has(literal.value)) {
    return parseError(
      'source.imports',
      'Native trusted block import source could not be transformed.'
    );
  }

  const end = readImportDeclarationEnd(source, literal.end);
  if (end === undefined) {
    return parseError(
      'source.imports',
      'Native trusted block import declaration could not be transformed.'
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
      'Only a native trusted block default export can be transformed.'
    );
  }

  const expressionStart = skipWhitespaceAndComments(source, nextToken.end);
  const firstExpressionToken = tokens.find(
    (token) => token.start >= expressionStart
  );
  const secondExpressionToken = firstExpressionToken
    ? tokens.find((token) => token.start >= firstExpressionToken.end)
    : undefined;
  const isDeclarationExport =
    firstExpressionToken?.value === 'function' ||
    firstExpressionToken?.value === 'class' ||
    (firstExpressionToken?.value === 'async' &&
      secondExpressionToken?.value === 'function');
  const statementEnd = isDeclarationExport
    ? readDefaultExportDeclarationEnd(source, expressionStart)
    : readDefaultExportStatementEnd(source, expressionStart);

  if (!statementEnd) {
    return parseError(
      'source.defaultExport',
      'Native trusted block default export could not be transformed.'
    );
  }

  const expression = source
    .slice(expressionStart, statementEnd.expressionEnd)
    .trim();
  if (expression.length === 0) {
    return parseError(
      'source.defaultExport',
      'Native trusted block default export expression is required.'
    );
  }

  return {
    ok: true,
    value: {
      start: exportToken.start,
      end: statementEnd.statementEnd,
      replacement: `const ${DEFAULT_EXPORT_IDENTIFIER} = ${expression};`
    }
  };
}

function parseImportClause(
  clause: string,
  source: NativeTrustedBlockInjectedModuleSource
): ParseResult<NativeTrustedBlockImportBinding[]> {
  const trimmed = clause.trim();
  if (trimmed.length === 0) {
    return parseError(
      'source.imports',
      'Native trusted block import bindings are required for this declaration.'
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
          'Native trusted block import bindings could not be transformed.'
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
  source: NativeTrustedBlockInjectedModuleSource
): ParseResult<NativeTrustedBlockImportBinding[]> {
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
  source: NativeTrustedBlockInjectedModuleSource
): ParseResult<NativeTrustedBlockImportBinding[]> {
  const trimmed = clause.trim();
  if (!trimmed.endsWith('}')) {
    return parseError(
      'source.imports',
      'Native trusted block named import bindings could not be transformed.'
    );
  }

  const content = trimmed.slice(1, -1).trim();
  if (content.length === 0) {
    return { ok: true, value: [] };
  }

  const bindings: NativeTrustedBlockImportBinding[] = [];
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
  source: NativeTrustedBlockInjectedModuleSource
): ParseResult<NativeTrustedBlockImportBinding | undefined> {
  const trimmed = segment.trim();
  if (trimmed.length === 0) {
    return { ok: true, value: undefined };
  }

  const parts = trimmed.split(/\s+/);
  if (parts.length !== 1 && !(parts.length === 3 && parts[1] === 'as')) {
    return parseError(
      'source.imports',
      'Native trusted block named import binding could not be transformed.'
    );
  }

  const imported = parts[0];
  const local = parts.length === 1 ? imported : parts[2];
  if (!isImportName(imported) || !isLocalBindingName(local)) {
    return parseError(
      'source.imports',
      'Native trusted block named import binding could not be transformed.'
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
  source: NativeTrustedBlockInjectedModuleSource
): ParseResult<NativeTrustedBlockImportBinding> {
  const local = clause.trim();
  if (!isLocalBindingName(local)) {
    return parseError(
      'source.imports',
      'Native trusted block default import binding could not be transformed.'
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
  source: NativeTrustedBlockInjectedModuleSource
): ParseResult<NativeTrustedBlockImportBinding> {
  const parts = clause.trim().split(/\s+/);
  if (parts.length !== 3 || parts[0] !== '*' || parts[1] !== 'as') {
    return parseError(
      'source.imports',
      'Native trusted block namespace import binding could not be transformed.'
    );
  }

  const local = parts[2];
  if (!isLocalBindingName(local)) {
    return parseError(
      'source.imports',
      'Native trusted block namespace import binding could not be transformed.'
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

function formatNamedBinding(
  binding: Extract<NativeTrustedBlockImportBinding, { kind: 'named' }>
): string {
  return binding.imported === binding.local
    ? binding.imported
    : `${binding.imported}: ${binding.local}`;
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

function readDefaultExportDeclarationEnd(
  source: string,
  start: number
): StatementEnd | undefined {
  const bodyStart = findDeclarationBodyStart(source, start);
  if (bodyStart === undefined) {
    return undefined;
  }

  const bodyEnd = consumeBalancedBlock(source, bodyStart);
  const statementEnd = readOptionalSemicolonEnd(source, bodyEnd);

  return {
    expressionEnd: bodyEnd,
    statementEnd
  };
}

function findDeclarationBodyStart(
  source: string,
  start: number
): number | undefined {
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

    if (char === '(' || char === '[') {
      depth += 1;
      index += 1;
      continue;
    }

    if (char === ')' || char === ']') {
      depth = Math.max(0, depth - 1);
      index += 1;
      continue;
    }

    if (char === '{' && depth === 0) {
      return index;
    }

    index += 1;
  }

  return undefined;
}

function consumeBalancedBlock(source: string, start: number): number {
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

    if (char === '{') {
      depth += 1;
      index += 1;
      continue;
    }

    if (char === '}') {
      depth -= 1;
      index += 1;
      if (depth === 0) {
        return index;
      }
      continue;
    }

    index += 1;
  }

  return source.length;
}

function readOptionalSemicolonEnd(source: string, start: number): number {
  const index = skipHorizontalWhitespace(source, start);
  return source[index] === ';' ? index + 1 : start;
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

function isImportName(value: string): boolean {
  return value === 'default' || isIdentifierName(value);
}

function isLocalBindingName(value: string): boolean {
  return (
    isIdentifierName(value) &&
    !localBindingIdentifiers.has(value) &&
    !RESERVED_TRANSFORM_IDENTIFIERS.has(value)
  );
}

function isIdentifierName(value: string): boolean {
  return /^[A-Za-z_$][A-Za-z0-9_$]*$/.test(value);
}

function isWhitespace(char: string): boolean {
  return char === ' ' || char === '\t' || char === '\n' || char === '\r';
}

function isIdentifierStart(char: string): boolean {
  return /[A-Za-z_$]/.test(char);
}

function isIdentifierPart(char: string): boolean {
  return /[A-Za-z0-9_$]/.test(char);
}

function parseError(path: string, message: string): ParseFailure {
  return {
    ok: false,
    error: { code: 'runtime_error', path, message }
  };
}
