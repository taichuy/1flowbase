import type { BlockProtocolError } from '@1flowbase/page-protocol';
import { parse, type Node, type Program, type Token } from 'acorn';

export interface JavaScriptPolicyToken {
  value: string;
  start: number;
  end: number;
}

export type JavaScriptPolicyParseResult =
  | {
      ok: true;
      tokens: JavaScriptPolicyToken[];
      importErrors: BlockProtocolError[];
    }
  | { ok: false; error: BlockProtocolError };

interface JavaScriptImportReference {
  kind: 'static' | 'dynamic';
  source?: string;
  start: number;
  sourceLocation?: { line: number; column: number };
}

export function parseNativeTrustedBlockJavaScriptPolicy(
  source: string,
  acceptedImportSources: ReadonlySet<string>,
  includeImportSourceLocations: boolean,
  compilerGeneratedImportSources: ReadonlySet<string>
): JavaScriptPolicyParseResult {
  const parserTokens: Token[] = [];
  let program: Program;
  try {
    program = parse(source, {
      ecmaVersion: 'latest',
      sourceType: 'module',
      locations: true,
      allowAwaitOutsideFunction: true,
      allowReturnOutsideFunction: true,
      onToken: parserTokens
    });
  } catch (error) {
    return {
      ok: false,
      error: policyError(
        'syntax_invalid',
        'source',
        readParserErrorMessage(error)
      )
    };
  }

  return {
    ok: true,
    tokens: collectPolicyTokens(program, parserTokens),
    importErrors: validateJavaScriptImports(
      program,
      acceptedImportSources,
      includeImportSourceLocations,
      compilerGeneratedImportSources
    )
  };
}

function validateJavaScriptImports(
  program: Program,
  acceptedImportSources: ReadonlySet<string>,
  includeSourceLocations: boolean,
  compilerGeneratedImportSources: ReadonlySet<string>
): BlockProtocolError[] {
  const references = collectJavaScriptImportReferences(program)
    .filter(
      (reference) =>
        reference.kind !== 'static' ||
        !reference.source ||
        !compilerGeneratedImportSources.has(reference.source)
    )
    .sort((left, right) => left.start - right.start);

  return references.flatMap((reference, index): BlockProtocolError[] => {
    const sourceLocation = includeSourceLocations
      ? reference.sourceLocation
      : undefined;
    if (reference.kind === 'dynamic') {
      return [
        policyError(
          'import_denied',
          `source.imports[${index}]`,
          'Dynamic import and import host access are not allowed.',
          sourceLocation
        )
      ];
    }
    if (
      reference.source !== undefined &&
      !acceptedImportSources.has(reference.source)
    ) {
      return [
        policyError(
          'import_denied',
          `source.imports[${index}]`,
          `Import source '${reference.source}' is not allowed.`,
          sourceLocation
        )
      ];
    }
    return [];
  });
}

function collectJavaScriptImportReferences(
  program: Program
): JavaScriptImportReference[] {
  const references: JavaScriptImportReference[] = [];
  walkPolicySyntaxTree(program, (node) => {
    const record = node as Node & Record<string, unknown>;
    if (
      node.type === 'ImportDeclaration' ||
      node.type === 'ExportNamedDeclaration' ||
      node.type === 'ExportAllDeclaration'
    ) {
      const source = readLiteralString(record.source);
      if (source !== undefined) {
        references.push({
          kind: 'static',
          source,
          start: node.start,
          sourceLocation: readNodeStartLocation(node)
        });
      }
      return;
    }
    if (
      node.type === 'ImportExpression' ||
      (node.type === 'MetaProperty' &&
        readIdentifierName(record.meta) === 'import')
    ) {
      references.push({
        kind: 'dynamic',
        start: node.start,
        sourceLocation: readNodeStartLocation(node)
      });
    }
  });
  return references;
}

function collectPolicyTokens(
  program: Program,
  parserTokens: readonly Token[]
): JavaScriptPolicyToken[] {
  const tokens = parserTokens.flatMap((token): JavaScriptPolicyToken[] => {
    const tokenValue = (token as Token & { value?: unknown }).value;
    const value =
      token.type.label === 'name'
        ? tokenValue
        : (token.type.keyword ?? undefined);
    return typeof value === 'string'
      ? [{ value, start: token.start, end: token.end }]
      : [];
  });

  walkPolicySyntaxTree(program, (node) => {
    if (node.type !== 'MemberExpression') return;
    const member = node as Node & {
      computed?: boolean;
      property?: Node & { type: string; value?: unknown };
    };
    if (
      member.computed &&
      member.property?.type === 'Literal' &&
      typeof member.property.value === 'string'
    ) {
      tokens.push({
        value: member.property.value,
        start: member.property.start + 1,
        end: member.property.end - 1
      });
    }
  });

  return tokens.sort((left, right) => left.start - right.start);
}

function walkPolicySyntaxTree(root: Node, visit: (node: Node) => void): void {
  const pending: Node[] = [root];
  while (pending.length > 0) {
    const node = pending.pop();
    if (!node) continue;
    visit(node);
    Object.values(node as unknown as Record<string, unknown>).forEach(
      (value) => {
        if (Array.isArray(value)) {
          value.forEach((item) => {
            if (isSyntaxTreeNode(item)) pending.push(item);
          });
        } else if (isSyntaxTreeNode(value)) {
          pending.push(value);
        }
      }
    );
  }
}

function readLiteralString(value: unknown): string | undefined {
  if (!isSyntaxTreeNode(value) || value.type !== 'Literal') return undefined;
  const literal = value as Node & { value?: unknown };
  return typeof literal.value === 'string' ? literal.value : undefined;
}

function readIdentifierName(value: unknown): string | undefined {
  if (!isSyntaxTreeNode(value) || value.type !== 'Identifier') return undefined;
  const identifier = value as Node & { name?: unknown };
  return typeof identifier.name === 'string' ? identifier.name : undefined;
}

function readNodeStartLocation(
  node: Node
): { line: number; column: number } | undefined {
  return node.loc
    ? { line: node.loc.start.line, column: node.loc.start.column + 1 }
    : undefined;
}

function isSyntaxTreeNode(value: unknown): value is Node {
  return (
    typeof value === 'object' &&
    value !== null &&
    typeof (value as { type?: unknown }).type === 'string' &&
    typeof (value as { start?: unknown }).start === 'number' &&
    typeof (value as { end?: unknown }).end === 'number'
  );
}

function policyError(
  code: BlockProtocolError['code'],
  path: string,
  message: string,
  sourceLocation?: { line: number; column: number }
): BlockProtocolError {
  return {
    code,
    path,
    message,
    ...(sourceLocation ? { sourceLocation } : {})
  };
}

function readParserErrorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : 'Native trusted block JavaScript parsing failed.';
}
