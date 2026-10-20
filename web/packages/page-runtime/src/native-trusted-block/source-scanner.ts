import {
  deniedAntdStaticNotificationMethods,
  deniedAntdStaticModalMethods,
  deniedCallIdentifiers,
  deniedEscapeIdentifiers
} from './source-policy-constants';
import type { BlockProtocolError } from '@1flowbase/page-protocol';

export interface SourceToken {
  value: string;
  start: number;
  end: number;
}

interface StringLiteralValue {
  value: string;
  end: number;
}

export interface PropertyAccess {
  property: string;
  end: number;
}

interface ScanResult {
  tokens: SourceToken[];
  error?: BlockProtocolError;
}

interface ScanContext {
  deferSyntaxErrorToTsxCompiler: boolean;
}

export function scanSource(source: string): ScanResult {
  const tokens: SourceToken[] = [];
  const context: ScanContext = { deferSyntaxErrorToTsxCompiler: false };
  const result = scanCode(source, 0, tokens, undefined, context);

  if (result.error && !context.deferSyntaxErrorToTsxCompiler) {
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
  stopChar?: string,
  context: ScanContext = { deferSyntaxErrorToTsxCompiler: false }
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
      const templateEnd = consumeTemplate(source, index, tokens, context);
      if (templateEnd.error) {
        return templateEnd;
      }
      index = templateEnd.index;
      continue;
    }

    if (isJsxStart(source, index)) {
      const jsxEnd = scanJsxElement(source, index, tokens, context);
      if (jsxEnd.error) {
        return jsxEnd;
      }
      index = jsxEnd.index;
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
  tokens: SourceToken[],
  context: ScanContext
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
      const expression = scanCode(source, index + 2, tokens, '}', context);
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

function scanJsxElement(
  source: string,
  start: number,
  tokens: SourceToken[],
  context: ScanContext
): ScanCodeResult {
  let index = start + 1;

  while (index < source.length) {
    const char = source[index];

    if (char === '"' || char === "'") {
      const stringEnd = consumeQuotedString(source, index, char);
      if (stringEnd.error) return stringEnd;
      index = stringEnd.index;
      continue;
    }

    if (char === '{') {
      const expression = scanCode(source, index + 1, tokens, '}', context);
      if (expression.error) {
        context.deferSyntaxErrorToTsxCompiler = true;
        return expression;
      }
      index = expression.index;
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

    if (char === '/' && source[index + 1] === '>') {
      return { index: index + 2, closedByStop: true };
    }

    if (char === '>') {
      index += 1;
      break;
    }

    index += 1;
  }

  while (index < source.length) {
    if (source.startsWith('</', index)) {
      const close = source.indexOf('>', index + 2);
      if (close === -1) {
        context.deferSyntaxErrorToTsxCompiler = true;
        return { index: source.length, closedByStop: false };
      }
      return { index: close + 1, closedByStop: true };
    }

    if (isJsxStart(source, index)) {
      const child = scanJsxElement(source, index, tokens, context);
      if (child.error) return child;
      index = child.index;
      continue;
    }

    if (source[index] === '{') {
      const expression = scanCode(source, index + 1, tokens, '}', context);
      if (expression.error) {
        context.deferSyntaxErrorToTsxCompiler = true;
        return expression;
      }
      index = expression.index;
      continue;
    }

    index += 1;
  }

  context.deferSyntaxErrorToTsxCompiler = true;
  return { index: source.length, closedByStop: false };
}

export function skipWhitespace(source: string, start: number): number {
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

function isJsxStart(source: string, index: number): boolean {
  if (source[index] !== '<') return false;
  const next = source[index + 1];
  if (next !== '>' && !isIdentifierStart(next)) return false;

  const previousIndex = previousNonWhitespaceIndex(source, index);
  if (previousIndex < 0 || !isIdentifierPart(source[previousIndex])) {
    return true;
  }

  let wordStart = previousIndex;
  while (wordStart > 0 && isIdentifierPart(source[wordStart - 1])) {
    wordStart -= 1;
  }
  const previousWord = source.slice(wordStart, previousIndex + 1);
  return previousWord === 'return' || previousWord === 'yield';
}

export function isIdentifierStart(char: string): boolean {
  return /[A-Za-z_$]/.test(char);
}

export function isIdentifierPart(char: string): boolean {
  return /[A-Za-z0-9_$]/.test(char);
}

function syntaxError(index: number, message: string): ScanCodeResult {
  return {
    index,
    closedByStop: false,
    error: failureError('syntax_invalid', 'source', message)
  };
}

function isDeniedComputedProperty(property: string): boolean {
  return (
    deniedEscapeIdentifiers.has(property) ||
    deniedCallIdentifiers.has(property) ||
    deniedAntdStaticModalMethods.has(property) ||
    deniedAntdStaticNotificationMethods.has(property)
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

export function readComputedPropertyAccess(
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

export function readStringLiteral(
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

export function failureError(
  code: BlockProtocolError['code'],
  path: string,
  message: string,
  sourceLocation?: BlockProtocolError['sourceLocation']
): BlockProtocolError {
  return sourceLocation
    ? { code, path, message, sourceLocation }
    : { code, path, message };
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
