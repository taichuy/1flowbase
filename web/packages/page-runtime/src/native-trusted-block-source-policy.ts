import type { BlockProtocolError } from '@1flowbase/page-protocol';

import {
  allowedImports,
  deniedAntdGlobalIdentifiers,
  deniedAntdStaticModalMethods,
  deniedCallForwarders,
  deniedCallIdentifiers,
  deniedConstructorIdentifiers,
  deniedEscapeIdentifiers,
  deniedGlobalIdentifiers,
  deniedPortalIdentifiers
} from './native-trusted-block/source-policy-constants';

export {
  NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME
} from './native-trusted-block/source-policy-constants';

export interface ValidateNativeTrustedBlockSourceSuccess {
  ok: true;
  source: string;
  normalizedSource: string;
  errors: [];
}

export interface ValidateNativeTrustedBlockSourceFailure {
  ok: false;
  errors: BlockProtocolError[];
}

export type ValidateNativeTrustedBlockSourceResult =
  | ValidateNativeTrustedBlockSourceSuccess
  | ValidateNativeTrustedBlockSourceFailure;

export interface ValidateNativeTrustedBlockSourceOptions {
  allowedImportSources?: ReadonlySet<string>;
}

interface SourceToken {
  value: string;
  start: number;
  end: number;
}

interface StringLiteralValue {
  value: string;
  end: number;
}

interface PropertyAccess {
  property: string;
  end: number;
}

interface ScanResult {
  tokens: SourceToken[];
  error?: BlockProtocolError;
}

export function validateNativeTrustedBlockSource(
  source: unknown,
  options: ValidateNativeTrustedBlockSourceOptions = {}
): ValidateNativeTrustedBlockSourceResult {
  try {
    if (typeof source !== 'string') {
      return failure(
        'transform_failed',
        'source',
        'Native trusted block source must be a string.'
      );
    }

    const scan = scanSource(source);
    if (scan.error) {
      return { ok: false, errors: [scan.error] };
    }

    const errors = [
      ...validateImports(
        source,
        scan.tokens,
        options.allowedImportSources ?? allowedImports
      ),
      ...validateDeniedCapabilities(source, scan.tokens)
    ];

    if (errors.length > 0) {
      return { ok: false, errors };
    }

    return {
      ok: true,
      source,
      normalizedSource: source.trim(),
      errors: []
    };
  } catch {
    return failure(
      'transform_failed',
      'source',
      'Native trusted block source validation failed.'
    );
  }
}

function scanSource(source: string): ScanResult {
  const tokens: SourceToken[] = [];
  const result = scanCode(source, 0, tokens);

  if (result.error) {
    return { tokens, error: result.error };
  }

  return { tokens };
}

interface ScanCodeResult {
  index: number;
  closedByStop: boolean;
  error?: BlockProtocolError;
}

function scanCode(
  source: string,
  start: number,
  tokens: SourceToken[],
  stopChar?: string
): ScanCodeResult {
  const delimiterStack: Array<{ expected: string; index: number }> = [];
  let index = start;

  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];

    if (stopChar && char === stopChar && delimiterStack.length === 0) {
      return { index: index + 1, closedByStop: true };
    }

    if (isWhitespace(char)) {
      index += 1;
      continue;
    }

    if (char === '/' && next === '/') {
      index = consumeLineComment(source, index + 2);
      continue;
    }

    if (char === '/' && next === '*') {
      const commentEnd = source.indexOf('*/', index + 2);
      if (commentEnd === -1) {
        return syntaxError(index, 'Unterminated block comment.');
      }
      index = commentEnd + 2;
      continue;
    }

    if (char === '"' || char === "'") {
      const stringEnd = consumeQuotedString(source, index, char);
      if (stringEnd.error) {
        return stringEnd;
      }
      index = stringEnd.index;
      continue;
    }

    if (char === '`') {
      const templateEnd = consumeTemplate(source, index, tokens);
      if (templateEnd.error) {
        return templateEnd;
      }
      index = templateEnd.index;
      continue;
    }

    if (isIdentifierStart(char)) {
      const tokenStart = index;
      index += 1;
      while (index < source.length && isIdentifierPart(source[index])) {
        index += 1;
      }
      tokens.push({
        value: source.slice(tokenStart, index),
        start: tokenStart,
        end: index
      });
      continue;
    }

    if (char === '[') {
      const computedAccess = readComputedPropertyAccess(source, index);
      if (
        computedAccess &&
        isDeniedComputedProperty(computedAccess.property) &&
        isComputedPropertyAccessTarget(source, index)
      ) {
        tokens.push({
          value: computedAccess.property,
          start: index + 1,
          end: computedAccess.end
        });
      }
    }

    if (char === '(' || char === '[' || char === '{') {
      delimiterStack.push({
        expected: matchingDelimiter(char),
        index
      });
      index += 1;
      continue;
    }

    if (char === ')' || char === ']' || char === '}') {
      const current = delimiterStack.pop();
      if (!current || current.expected !== char) {
        return syntaxError(index, `Unexpected '${char}'.`);
      }
      index += 1;
      continue;
    }

    index += 1;
  }

  if (stopChar) {
    return syntaxError(start, `Unterminated '${stopChar}' expression.`);
  }

  const openDelimiter = delimiterStack.at(-1);
  if (openDelimiter) {
    return syntaxError(
      openDelimiter.index,
      `Unterminated '${openDelimiter.expected}' delimiter.`
    );
  }

  return { index, closedByStop: false };
}

function consumeLineComment(source: string, start: number): number {
  const lineEnd = source.indexOf('\n', start);
  return lineEnd === -1 ? source.length : lineEnd + 1;
}

function consumeQuotedString(
  source: string,
  start: number,
  quote: '"' | "'"
): ScanCodeResult {
  let index = start + 1;

  while (index < source.length) {
    const char = source[index];

    if (char === '\\') {
      index += 2;
      continue;
    }

    if (char === quote) {
      return { index: index + 1, closedByStop: true };
    }

    if (char === '\n' || char === '\r') {
      return syntaxError(start, 'Unterminated string literal.');
    }

    index += 1;
  }

  return syntaxError(start, 'Unterminated string literal.');
}

function consumeTemplate(
  source: string,
  start: number,
  tokens: SourceToken[]
): ScanCodeResult {
  let index = start + 1;

  while (index < source.length) {
    const char = source[index];
    const next = source[index + 1];

    if (char === '\\') {
      index += 2;
      continue;
    }

    if (char === '`') {
      return { index: index + 1, closedByStop: true };
    }

    if (char === '$' && next === '{') {
      const expression = scanCode(source, index + 2, tokens, '}');
      if (expression.error) {
        return expression;
      }
      index = expression.index;
      continue;
    }

    index += 1;
  }

  return syntaxError(start, 'Unterminated template literal.');
}

function validateImports(
  source: string,
  tokens: SourceToken[],
  acceptedImportSources: ReadonlySet<string>
): BlockProtocolError[] {
  const errors: BlockProtocolError[] = [];
  let importIndex = 0;

  tokens.forEach((token, tokenIndex) => {
    if (token.value === 'import') {
      const importError = validateImportToken(
        source,
        tokens,
        tokenIndex,
        importIndex,
        acceptedImportSources
      );
      if (importError) {
        errors.push(importError);
      }
      importIndex += 1;
      return;
    }

    if (token.value === 'export') {
      const exportedSource = readExportSource(source, tokens, tokenIndex);
      if (exportedSource && !acceptedImportSources.has(exportedSource.value)) {
        errors.push(
          failureError(
            'import_denied',
            `source.imports[${importIndex}]`,
            `Import source '${exportedSource.value}' is not allowed.`
          )
        );
      }
      if (exportedSource) {
        importIndex += 1;
      }
    }
  });

  return errors;
}

function validateImportToken(
  source: string,
  tokens: SourceToken[],
  tokenIndex: number,
  importIndex: number,
  acceptedImportSources: ReadonlySet<string>
): BlockProtocolError | undefined {
  const token = tokens[tokenIndex];
  const path = `source.imports[${importIndex}]`;
  const nextCodeIndex = skipWhitespace(source, token.end);
  const nextChar = source[nextCodeIndex];

  if (nextChar === '(' || nextChar === '.') {
    return failureError(
      'import_denied',
      path,
      'Dynamic import and import host access are not allowed.'
    );
  }

  if (nextChar === '"' || nextChar === "'") {
    const sourceLiteral = readStringLiteral(source, nextCodeIndex);
    if (!sourceLiteral) {
      return failureError('syntax_invalid', 'source', 'Invalid import source.');
    }
    return acceptedImportSources.has(sourceLiteral.value)
      ? undefined
      : failureError(
          'import_denied',
          path,
          `Import source '${sourceLiteral.value}' is not allowed.`
        );
  }

  const fromToken = findTokenBeforeStatementEnd(
    source,
    tokens,
    tokenIndex + 1,
    'from'
  );
  if (!fromToken) {
    return failureError(
      'syntax_invalid',
      'source',
      'Invalid import statement.'
    );
  }

  const sourceLiteralIndex = skipWhitespace(source, fromToken.end);
  const sourceLiteral = readStringLiteral(source, sourceLiteralIndex);
  if (!sourceLiteral) {
    return failureError('syntax_invalid', 'source', 'Invalid import source.');
  }

  return acceptedImportSources.has(sourceLiteral.value)
    ? undefined
    : failureError(
        'import_denied',
        path,
        `Import source '${sourceLiteral.value}' is not allowed.`
      );
}

function readExportSource(
  source: string,
  tokens: SourceToken[],
  tokenIndex: number
): { value: string } | undefined {
  const fromToken = findTokenBeforeStatementEnd(
    source,
    tokens,
    tokenIndex + 1,
    'from'
  );
  if (!fromToken) {
    return undefined;
  }

  const sourceLiteralIndex = skipWhitespace(source, fromToken.end);
  return readStringLiteral(source, sourceLiteralIndex);
}

function findTokenBeforeStatementEnd(
  source: string,
  tokens: SourceToken[],
  startTokenIndex: number,
  tokenValue: string
): SourceToken | undefined {
  const tokenIndex = findTokenIndexBeforeStatementEnd(
    source,
    tokens,
    startTokenIndex,
    tokenValue
  );
  return tokenIndex === undefined ? undefined : tokens[tokenIndex];
}

function findTokenIndexBeforeStatementEnd(
  source: string,
  tokens: SourceToken[],
  startTokenIndex: number,
  tokenValue: string
): number | undefined {
  for (let index = startTokenIndex; index < tokens.length; index += 1) {
    const token = tokens[index];
    const segment = source.slice(tokens[startTokenIndex - 1].end, token.start);
    if (segment.includes(';')) {
      return undefined;
    }
    if (token.value === tokenValue) {
      return index;
    }
  }

  return undefined;
}

function validateDeniedCapabilities(
  source: string,
  tokens: SourceToken[]
): BlockProtocolError[] {
  const errors: BlockProtocolError[] = [];
  const antdModalAliases = collectAntdModalAliases(source, tokens);
  const addError = (error: BlockProtocolError): void => {
    const alreadyAdded = errors.some(
      (current) => current.code === error.code && current.path === error.path
    );
    if (!alreadyAdded) {
      errors.push(error);
    }
  };

  tokens.forEach((token) => {
    if (deniedGlobalIdentifiers.has(token.value)) {
      addError(
        capabilityError(
          token.value,
          `Identifier '${token.value}' is not allowed in native trusted block source.`
        )
      );
      return;
    }

    if (deniedPortalIdentifiers.has(token.value)) {
      addError(
        capabilityError(
          token.value,
          `Portal or root owner '${token.value}' is not allowed in native trusted block source.`
        )
      );
      return;
    }

    if (
      deniedAntdGlobalIdentifiers.has(token.value) &&
      isIdentifierReference(source, token)
    ) {
      addError(
        capabilityError(
          token.value,
          `AntD global API '${token.value}' is not allowed in native trusted block source.`
        )
      );
      return;
    }

    if (token.value === 'Upload') {
      addError(
        capabilityError(
          token.value,
          'AntD Upload is not allowed in native trusted block source.'
        )
      );
      return;
    }

    if (deniedEscapeIdentifiers.has(token.value)) {
      addError(
        capabilityError(
          token.value,
          `Identifier '${token.value}' is not allowed in native trusted block source.`
        )
      );
      return;
    }

    const deniedPropertyAccess = readDeniedPropertyAccess(
      source,
      token,
      antdModalAliases
    );
    if (deniedPropertyAccess) {
      addError(
        failureError(
          deniedPropertyAccess.code,
          `source.identifiers.${deniedPropertyAccess.identifier}`,
          deniedPropertyAccess.message
        )
      );
      return;
    }

    if (
      deniedCallIdentifiers.has(token.value) &&
      isDeniedInvocation(source, token)
    ) {
      addError(
        failureError(
          token.value === 'require' ? 'import_denied' : 'transform_failed',
          `source.identifiers.${token.value}`,
          `Call '${token.value}' is not allowed in native trusted block source.`
        )
      );
      return;
    }

    if (deniedConstructorIdentifiers.has(token.value)) {
      addError(
        failureError(
          'transform_failed',
          `source.identifiers.${token.value}`,
          `Constructor '${token.value}' is not allowed in native trusted block source.`
        )
      );
    }
  });

  return errors;
}

function isIdentifierReference(source: string, token: SourceToken): boolean {
  for (let index = token.start - 1; index >= 0; index -= 1) {
    if (isWhitespace(source[index])) continue;
    if (source[index] === '.') return false;
    break;
  }
  for (let index = token.end; index < source.length; index += 1) {
    if (isWhitespace(source[index])) continue;
    if (source[index] === ':' || source[index] === '=') return false;
    if (source[index] === '?') {
      for (index += 1; index < source.length; index += 1) {
        if (isWhitespace(source[index])) continue;
        return source[index] !== ':';
      }
    }
    break;
  }
  return true;
}

function collectAntdModalAliases(
  source: string,
  tokens: SourceToken[]
): Set<string> {
  const antdModuleAliases = collectAntdModuleAliases(source, tokens);
  const aliases = new Set<string>(['Modal']);

  collectAntdModalImportAliases(source, tokens, aliases);

  let changed = true;
  while (changed) {
    changed =
      collectAntdModalAssignmentAliases(
        source,
        tokens,
        aliases,
        antdModuleAliases
      ) ||
      collectAntdModalDestructuringAliases(
        source,
        tokens,
        aliases,
        antdModuleAliases
      );
  }

  return aliases;
}

function collectAntdModuleAliases(
  source: string,
  tokens: SourceToken[]
): Set<string> {
  const aliases = new Set<string>(['antd']);

  tokens.forEach((token, tokenIndex) => {
    if (token.value !== 'import') {
      return;
    }

    const importSource = readStaticImportSource(source, tokens, tokenIndex);
    if (!importSource || importSource.value !== 'antd') {
      return;
    }

    const importSegment = source.slice(token.end, importSource.fromToken.start);
    const namespaceMatch = importSegment.match(
      /\*\s+as\s+([A-Za-z_$][A-Za-z0-9_$]*)/
    );
    if (namespaceMatch) {
      aliases.add(namespaceMatch[1]);
    }
  });

  return aliases;
}

function collectAntdModalImportAliases(
  source: string,
  tokens: SourceToken[],
  aliases: Set<string>
): void {
  tokens.forEach((token, tokenIndex) => {
    if (token.value !== 'import') {
      return;
    }

    const importSource = readStaticImportSource(source, tokens, tokenIndex);
    if (!importSource || importSource.value !== 'antd') {
      return;
    }

    for (
      let index = tokenIndex + 1;
      index < importSource.fromTokenIndex;
      index += 1
    ) {
      if (tokens[index].value !== 'Modal') {
        continue;
      }

      const maybeAsToken = tokens[index + 1];
      const maybeAliasToken = tokens[index + 2];
      if (
        maybeAsToken?.value === 'as' &&
        maybeAliasToken &&
        index + 2 < importSource.fromTokenIndex
      ) {
        aliases.add(maybeAliasToken.value);
        continue;
      }

      aliases.add('Modal');
    }
  });
}

function collectAntdModalAssignmentAliases(
  source: string,
  tokens: SourceToken[],
  modalAliases: Set<string>,
  antdModuleAliases: Set<string>
): boolean {
  let changed = false;

  tokens.forEach((aliasToken, tokenIndex) => {
    if (!isVariableDeclarationName(source, tokens, tokenIndex)) {
      return;
    }

    const targetToken = tokens[tokenIndex + 1];
    if (!targetToken) {
      return;
    }

    const assignmentSegment = source.slice(aliasToken.end, targetToken.start);
    if (!isSimpleAssignmentSegment(assignmentSegment)) {
      return;
    }

    if (
      modalAliases.has(targetToken.value) ||
      isAntdModalPropertyReference(source, targetToken, antdModuleAliases)
    ) {
      changed = addAlias(modalAliases, aliasToken.value) || changed;
    }
  });

  return changed;
}

function collectAntdModalDestructuringAliases(
  source: string,
  tokens: SourceToken[],
  modalAliases: Set<string>,
  antdModuleAliases: Set<string>
): boolean {
  let changed = false;

  tokens.forEach((modalToken, tokenIndex) => {
    if (modalToken.value !== 'Modal') {
      return;
    }

    const aliasToken = tokens[tokenIndex + 1];
    const moduleToken = tokens[tokenIndex + 2];
    if (
      !aliasToken ||
      !moduleToken ||
      !antdModuleAliases.has(moduleToken.value)
    ) {
      return;
    }

    const aliasSegment = source.slice(modalToken.end, aliasToken.start);
    const moduleSegment = source.slice(aliasToken.end, moduleToken.start);
    if (!aliasSegment.includes(':') || !/}\s*=/.test(moduleSegment)) {
      return;
    }

    changed = addAlias(modalAliases, aliasToken.value) || changed;
  });

  return changed;
}

function readStaticImportSource(
  source: string,
  tokens: SourceToken[],
  importTokenIndex: number
):
  | {
      value: string;
      fromToken: SourceToken;
      fromTokenIndex: number;
    }
  | undefined {
  const importToken = tokens[importTokenIndex];
  const nextCodeIndex = skipWhitespace(source, importToken.end);
  const nextChar = source[nextCodeIndex];
  if (
    nextChar === '(' ||
    nextChar === '.' ||
    nextChar === '"' ||
    nextChar === "'"
  ) {
    return undefined;
  }

  const fromTokenIndex = findTokenIndexBeforeStatementEnd(
    source,
    tokens,
    importTokenIndex + 1,
    'from'
  );
  if (fromTokenIndex === undefined) {
    return undefined;
  }

  const sourceLiteralIndex = skipWhitespace(source, tokens[fromTokenIndex].end);
  const sourceLiteral = readStringLiteral(source, sourceLiteralIndex);
  if (!sourceLiteral) {
    return undefined;
  }

  return {
    value: sourceLiteral.value,
    fromToken: tokens[fromTokenIndex],
    fromTokenIndex
  };
}

function isVariableDeclarationName(
  source: string,
  tokens: SourceToken[],
  tokenIndex: number
): boolean {
  const previousToken = tokens[tokenIndex - 1];
  if (
    !previousToken ||
    (previousToken.value !== 'const' &&
      previousToken.value !== 'let' &&
      previousToken.value !== 'var')
  ) {
    return false;
  }

  return skipWhitespace(source, previousToken.end) === tokens[tokenIndex].start;
}

function isSimpleAssignmentSegment(segment: string): boolean {
  return /^[\s=]+$/.test(segment) && segment.includes('=');
}

function isAntdModalPropertyReference(
  source: string,
  token: SourceToken,
  antdModuleAliases: Set<string>
): boolean {
  if (!antdModuleAliases.has(token.value)) {
    return false;
  }

  const access = readPropertyAccess(source, token.end);
  return access?.property === 'Modal';
}

function addAlias(aliases: Set<string>, alias: string): boolean {
  if (aliases.has(alias)) {
    return false;
  }

  aliases.add(alias);
  return true;
}

function capabilityError(
  identifier: string,
  message: string
): BlockProtocolError {
  return failureError(
    'transform_failed',
    `source.identifiers.${identifier}`,
    message
  );
}

function readDeniedPropertyAccess(
  source: string,
  token: SourceToken,
  antdModalAliases: Set<string>
):
  | {
      identifier: string;
      code: BlockProtocolError['code'];
      message: string;
    }
  | undefined {
  const access = readPropertyAccess(source, token.end);
  if (!access) {
    return undefined;
  }

  if (deniedEscapeIdentifiers.has(access.property)) {
    return {
      identifier: access.property,
      code: 'transform_failed',
      message: `Property '${access.property}' is not allowed in native trusted block source.`
    };
  }

  if (access.property === 'cookie') {
    return {
      identifier: access.property,
      code: 'transform_failed',
      message: 'Cookie access is not allowed in native trusted block source.'
    };
  }

  if (
    token.value === 'ReactDOM' &&
    (access.property === 'createPortal' ||
      access.property === 'createRoot' ||
      access.property === 'hydrateRoot')
  ) {
    return {
      identifier: access.property,
      code: 'transform_failed',
      message: `ReactDOM '${access.property}' is not allowed in native trusted block source.`
    };
  }

  if (
    antdModalAliases.has(token.value) &&
    deniedAntdStaticModalMethods.has(access.property)
  ) {
    return {
      identifier: access.property,
      code: 'transform_failed',
      message: `AntD Modal static method '${access.property}' is not allowed in native trusted block source.`
    };
  }

  if (!isDeniedPropertyInvocation(source, access.end)) {
    return undefined;
  }

  if (deniedCallIdentifiers.has(access.property)) {
    return {
      identifier: access.property,
      code:
        access.property === 'require' ? 'import_denied' : 'transform_failed',
      message: `Call '${access.property}' is not allowed in native trusted block source.`
    };
  }

  if (deniedConstructorIdentifiers.has(access.property)) {
    return {
      identifier: access.property,
      code: 'transform_failed',
      message: `Constructor '${access.property}' is not allowed in native trusted block source.`
    };
  }

  return undefined;
}

function isDeniedComputedProperty(property: string): boolean {
  return (
    deniedEscapeIdentifiers.has(property) ||
    deniedCallIdentifiers.has(property) ||
    deniedConstructorIdentifiers.has(property) ||
    deniedAntdStaticModalMethods.has(property) ||
    property === 'cookie'
  );
}

function isComputedPropertyAccessTarget(
  source: string,
  start: number
): boolean {
  const targetIndex = previousNonWhitespaceIndex(source, start);
  if (targetIndex < 0) {
    return false;
  }

  const targetChar = source[targetIndex];
  if (targetChar === '.' && source[targetIndex - 1] === '?') {
    const optionalTargetIndex = previousNonWhitespaceIndex(
      source,
      targetIndex - 1
    );
    return (
      optionalTargetIndex >= 0 &&
      isPropertyTargetChar(source[optionalTargetIndex])
    );
  }

  return isPropertyTargetChar(targetChar);
}

function previousNonWhitespaceIndex(source: string, start: number): number {
  let index = start - 1;
  while (index >= 0 && isWhitespace(source[index])) {
    index -= 1;
  }
  return index;
}

function isPropertyTargetChar(char: string): boolean {
  return (
    isIdentifierPart(char) ||
    char === ')' ||
    char === ']' ||
    char === '}' ||
    char === '"' ||
    char === "'" ||
    char === '`'
  );
}

function isDeniedInvocation(source: string, token: SourceToken): boolean {
  return (
    isCallExpressionAt(source, token.end) ||
    isForwardedCallExpression(source, token.end)
  );
}

function isDeniedPropertyInvocation(source: string, start: number): boolean {
  return (
    isCallExpressionAt(source, start) ||
    isForwardedCallExpression(source, start)
  );
}

function isForwardedCallExpression(source: string, start: number): boolean {
  const access = readPropertyAccess(source, start);
  return (
    !!access &&
    deniedCallForwarders.has(access.property) &&
    isCallExpressionAt(source, access.end)
  );
}

function isCallExpressionAt(source: string, start: number): boolean {
  const index = skipWhitespace(source, start);
  if (source[index] === '(') {
    return true;
  }

  if (source[index] === '?' && source[index + 1] === '.') {
    return source[skipWhitespace(source, index + 2)] === '(';
  }

  return false;
}

function readPropertyAccess(
  source: string,
  start: number
): PropertyAccess | undefined {
  const index = skipWhitespace(source, start);
  if (source[index] === '.') {
    return readDotPropertyAccess(source, index + 1);
  }

  if (source[index] === '?' && source[index + 1] === '.') {
    const optionalAccessIndex = skipWhitespace(source, index + 2);
    if (source[optionalAccessIndex] === '[') {
      return readComputedPropertyAccess(source, optionalAccessIndex);
    }
    return readDotPropertyAccess(source, optionalAccessIndex);
  }

  if (source[index] === '[') {
    return readComputedPropertyAccess(source, index);
  }

  return undefined;
}

function readDotPropertyAccess(
  source: string,
  start: number
): PropertyAccess | undefined {
  const propertyStart = skipWhitespace(source, start);
  if (!isIdentifierStart(source[propertyStart])) {
    return undefined;
  }

  let propertyEnd = propertyStart + 1;
  while (propertyEnd < source.length && isIdentifierPart(source[propertyEnd])) {
    propertyEnd += 1;
  }

  return {
    property: source.slice(propertyStart, propertyEnd),
    end: propertyEnd
  };
}

function readComputedPropertyAccess(
  source: string,
  start: number
): PropertyAccess | undefined {
  const literalStart = skipWhitespace(source, start + 1);
  const literal = readStringLiteral(source, literalStart);
  if (!literal) {
    return undefined;
  }

  const closeBracketIndex = skipWhitespace(source, literal.end);
  if (source[closeBracketIndex] !== ']') {
    return undefined;
  }

  return {
    property: literal.value,
    end: closeBracketIndex + 1
  };
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

function skipWhitespace(source: string, start: number): number {
  let index = start;

  while (index < source.length && isWhitespace(source[index])) {
    index += 1;
  }

  return index;
}

function matchingDelimiter(char: string): string {
  if (char === '(') {
    return ')';
  }
  if (char === '[') {
    return ']';
  }
  return '}';
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

function syntaxError(index: number, message: string): ScanCodeResult {
  return {
    index,
    closedByStop: false,
    error: failureError('syntax_invalid', 'source', message)
  };
}

function failure(
  code: BlockProtocolError['code'],
  path: string,
  message: string
): ValidateNativeTrustedBlockSourceFailure {
  return {
    ok: false,
    errors: [failureError(code, path, message)]
  };
}

function failureError(
  code: BlockProtocolError['code'],
  path: string,
  message: string
): BlockProtocolError {
  return { code, path, message };
}
