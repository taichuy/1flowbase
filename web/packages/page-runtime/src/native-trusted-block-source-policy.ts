import {
  readComputedPropertyAccess,
  failureError,
  readStringLiteral
} from './native-trusted-block/source-scanner';
import {
  type SourceToken,
  type PropertyAccess,
  scanSource,
  skipWhitespace,
  isIdentifierStart,
  isIdentifierPart
} from './native-trusted-block/source-scanner';
import type { BlockProtocolError } from '@1flowbase/page-protocol';

import {
  allowedImports,
  deniedAntdStaticNotificationMethods,
  deniedAntdStaticModalMethods,
  deniedCallForwarders,
  deniedCallIdentifiers,
  deniedEscapeIdentifiers,
  deniedGlobalIdentifiers,
  deniedPortalIdentifiers
} from './native-trusted-block/source-policy-constants';
import { parseNativeTrustedBlockJavaScriptPolicy } from './native-trusted-block/javascript-policy-parser';

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
  scopedModal?: boolean;
  scopedNotification?: boolean;
  compilerGeneratedImportSources?: ReadonlySet<string>;
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

    const acceptedImportSources =
      options.allowedImportSources ?? allowedImports;
    const includeImportSourceLocations =
      options.allowedImportSources !== undefined;
    const errors = [
      ...validateImports(
        source,
        scan.tokens,
        acceptedImportSources,
        includeImportSourceLocations
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

export function validateNativeTrustedBlockJavaScript(
  source: string,
  options: ValidateNativeTrustedBlockSourceOptions = {}
): ValidateNativeTrustedBlockSourceResult {
  try {
    const acceptedImportSources =
      options.allowedImportSources ?? allowedImports;
    const includeImportSourceLocations =
      options.allowedImportSources !== undefined;
    const parsed = parseNativeTrustedBlockJavaScriptPolicy(
      source,
      acceptedImportSources,
      includeImportSourceLocations,
      options.compilerGeneratedImportSources ?? new Set()
    );
    if (!parsed.ok) {
      return { ok: false, errors: [parsed.error] };
    }

    const errors = [
      ...parsed.importErrors,
      ...validateDeniedCapabilities(
        source,
        parsed.tokens,
        options.scopedModal,
        options.scopedNotification
      )
    ];

    return errors.length > 0
      ? { ok: false, errors }
      : {
          ok: true,
          source,
          normalizedSource: source.trim(),
          errors: []
        };
  } catch {
    return failure(
      'transform_failed',
      'source',
      'Native trusted block JavaScript validation failed.'
    );
  }
}

function validateImports(
  source: string,
  tokens: SourceToken[],
  acceptedImportSources: ReadonlySet<string>,
  includeSourceLocations: boolean
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
        acceptedImportSources,
        includeSourceLocations
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
            `Import source '${exportedSource.value}' is not allowed.`,
            includeSourceLocations
              ? sourceLocationFromOffset(source, token.start)
              : undefined
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
  acceptedImportSources: ReadonlySet<string>,
  includeSourceLocations: boolean
): BlockProtocolError | undefined {
  const token = tokens[tokenIndex];
  const path = `source.imports[${importIndex}]`;
  const sourceLocation = includeSourceLocations
    ? sourceLocationFromOffset(source, token.start)
    : undefined;
  const nextCodeIndex = skipWhitespace(source, token.end);
  const nextChar = source[nextCodeIndex];

  if (nextChar === '(' || nextChar === '.') {
    return failureError(
      'import_denied',
      path,
      'Dynamic import and import host access are not allowed.',
      sourceLocation
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
          `Import source '${sourceLiteral.value}' is not allowed.`,
          sourceLocation
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
        `Import source '${sourceLiteral.value}' is not allowed.`,
        sourceLocation
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
  tokens: SourceToken[],
  scopedModal = false,
  scopedNotification = false
): BlockProtocolError[] {
  const errors: BlockProtocolError[] = [];
  const antdModalAliases = scopedModal
    ? new Set<string>()
    : collectAntdModalAliases(source, tokens);
  const antdNotificationAliases = scopedNotification
    ? new Set<string>()
    : collectAntdNotificationAliases(source, tokens);
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
      antdModalAliases,
      antdNotificationAliases
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
  });

  return errors;
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

function collectAntdNotificationAliases(
  source: string,
  tokens: SourceToken[]
): Set<string> {
  const aliases = new Set<string>();

  tokens.forEach((token, tokenIndex) => {
    if (token.value !== 'import') return;
    const importSource = readStaticImportSource(source, tokens, tokenIndex);
    if (!importSource || importSource.value !== 'antd') return;

    for (
      let index = tokenIndex + 1;
      index < importSource.fromTokenIndex;
      index += 1
    ) {
      if (tokens[index].value !== 'notification') continue;
      const maybeAsToken = tokens[index + 1];
      const maybeAliasToken = tokens[index + 2];
      aliases.add(
        maybeAsToken?.value === 'as' &&
          maybeAliasToken &&
          index + 2 < importSource.fromTokenIndex
          ? maybeAliasToken.value
          : 'notification'
      );
    }
  });

  let changed = true;
  while (changed) {
    changed = false;
    tokens.forEach((aliasToken, tokenIndex) => {
      if (!isVariableDeclarationName(source, tokens, tokenIndex)) return;
      const targetToken = tokens[tokenIndex + 1];
      if (!targetToken) return;
      const assignmentSegment = source.slice(aliasToken.end, targetToken.start);
      if (
        isSimpleAssignmentSegment(assignmentSegment) &&
        aliases.has(targetToken.value)
      ) {
        changed = addAlias(aliases, aliasToken.value) || changed;
      }
    });
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
  antdModalAliases: Set<string>,
  antdNotificationAliases: Set<string>
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

  if (
    antdNotificationAliases.has(token.value) &&
    deniedAntdStaticNotificationMethods.has(access.property)
  ) {
    return {
      identifier: access.property,
      code: 'transform_failed',
      message: `AntD Notification static method '${access.property}' is not allowed in native trusted block source.`
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

  return undefined;
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

function sourceLocationFromOffset(
  source: string,
  offset: number
): NonNullable<BlockProtocolError['sourceLocation']> {
  let line = 1;
  let column = 1;

  for (let index = 0; index < offset; index += 1) {
    if (source[index] === '\r') {
      if (source[index + 1] === '\n' && index + 1 < offset) {
        index += 1;
      }
      line += 1;
      column = 1;
      continue;
    }

    if (source[index] === '\n') {
      line += 1;
      column = 1;
      continue;
    }

    column += 1;
  }

  return { line, column };
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
