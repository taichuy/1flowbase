import fs from 'node:fs';
import path from 'node:path';

import ts from 'typescript';
import { describe, expect, test } from 'vitest';

type SourceEntry = {
  file: string;
  sourceFile: ts.SourceFile;
};

function collectSourceEntries() {
  const files: string[] = [];

  function walk(directory: string) {
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      if (
        entry.isDirectory() &&
        ['coverage', 'dist', 'node_modules'].includes(entry.name)
      ) {
        continue;
      }

      const target = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        walk(target);
      } else if (/\.(ts|tsx)$/u.test(entry.name)) {
        files.push(target);
      }
    }
  }

  for (const root of ['src', '../packages']) {
    if (fs.existsSync(root)) {
      walk(root);
    }
  }

  return files.map<SourceEntry>((file) => ({
    file,
    sourceFile: ts.createSourceFile(
      file,
      fs.readFileSync(file, 'utf8'),
      ts.ScriptTarget.Latest,
      true,
      file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS
    )
  }));
}

function antdNamedImports(sourceFile: ts.SourceFile) {
  const imports = new Map<string, string>();

  sourceFile.forEachChild((node) => {
    if (
      !ts.isImportDeclaration(node) ||
      !ts.isStringLiteral(node.moduleSpecifier) ||
      node.moduleSpecifier.text !== 'antd' ||
      !node.importClause?.namedBindings ||
      !ts.isNamedImports(node.importClause.namedBindings)
    ) {
      return;
    }

    for (const element of node.importClause.namedBindings.elements) {
      imports.set(
        element.name.text,
        element.propertyName?.text ?? element.name.text
      );
    }
  });

  return imports;
}

function location(entry: SourceEntry, node: ts.Node) {
  const { line } = entry.sourceFile.getLineAndCharacterOfPosition(
    node.getStart(entry.sourceFile)
  );
  return `${entry.file}:${line + 1}`;
}

function staticMessageCalls(entry: SourceEntry) {
  type ApiOrigin = 'antd' | 'message' | 'static-method' | 'hook';
  const imports = antdNamedImports(entry.sourceFile);
  const messageImport = entry.sourceFile.statements.some(
    (node) =>
      ts.isImportDeclaration(node) &&
      ts.isStringLiteral(node.moduleSpecifier) &&
      node.moduleSpecifier.text === 'antd/es/message'
  );
  const namespaceImport = entry.sourceFile.statements.some(
    (node) =>
      ts.isImportDeclaration(node) &&
      ts.isStringLiteral(node.moduleSpecifier) &&
      node.moduleSpecifier.text === 'antd' &&
      node.importClause?.namedBindings &&
      ts.isNamespaceImport(node.importClause.namedBindings)
  );
  if (
    ![...imports.values()].includes('message') &&
    !messageImport &&
    !namespaceImport
  )
    return [];

  // Use the existing compiler's lexical binding so a local parameter named
  // message remains distinct from the imported context-free API.
  const host = ts.createCompilerHost({ noLib: true, noResolve: true });
  host.getSourceFile = (file) =>
    path.resolve(file) === path.resolve(entry.file)
      ? entry.sourceFile
      : undefined;
  const program = ts.createProgram(
    [entry.file],
    { noLib: true, noResolve: true },
    host
  );
  const checker = program.getTypeChecker();
  const roots = new Map<ts.Symbol, ApiOrigin>();
  for (const node of entry.sourceFile.statements) {
    if (
      !ts.isImportDeclaration(node) ||
      !ts.isStringLiteral(node.moduleSpecifier) ||
      !node.importClause
    )
      continue;
    const clause = node.importClause;
    const module = node.moduleSpecifier.text;
    const register = (name: ts.Identifier, origin: ApiOrigin) => {
      const symbol = checker.getSymbolAtLocation(name);
      if (symbol) roots.set(symbol, origin);
    };
    if (module === 'antd/es/message' && clause.name)
      register(clause.name, 'message');
    if (module !== 'antd' || !clause.namedBindings) continue;
    if (ts.isNamespaceImport(clause.namedBindings))
      register(clause.namedBindings.name, 'antd');
    else
      for (const specifier of clause.namedBindings.elements) {
        if ((specifier.propertyName?.text ?? specifier.name.text) === 'message')
          register(specifier.name, 'message');
      }
  }
  function memberOrigin(
    owner: ApiOrigin | null,
    member: string | null
  ): ApiOrigin | null {
    if (owner === 'antd') return member === 'message' ? 'message' : null;
    if (owner === 'message')
      return member === 'useMessage' ? 'hook' : 'static-method';
    return owner === 'static-method' ? owner : null;
  }
  function origin(
    expression: ts.Expression,
    seen = new Set<ts.Symbol>()
  ): ApiOrigin | null {
    if (
      ts.isParenthesizedExpression(expression) ||
      ts.isAsExpression(expression) ||
      ts.isTypeAssertionExpression(expression) ||
      ts.isNonNullExpression(expression)
    ) {
      return origin(expression.expression, seen);
    }
    if (ts.isPropertyAccessExpression(expression)) {
      return memberOrigin(
        origin(expression.expression, seen),
        expression.name.text
      );
    }
    if (ts.isElementAccessExpression(expression)) {
      return memberOrigin(
        origin(expression.expression, seen),
        ts.isStringLiteral(expression.argumentExpression)
          ? expression.argumentExpression.text
          : null
      );
    }
    if (!ts.isIdentifier(expression)) return null;
    const symbol = checker.getSymbolAtLocation(expression);
    if (!symbol || seen.has(symbol)) return null;
    const imported = roots.get(symbol);
    if (imported) return imported;
    seen.add(symbol);
    for (const declaration of symbol.declarations ?? []) {
      if (ts.isVariableDeclaration(declaration) && declaration.initializer) {
        return origin(declaration.initializer, seen);
      }
      if (
        ts.isBindingElement(declaration) &&
        ts.isObjectBindingPattern(declaration.parent)
      ) {
        const variable = declaration.parent.parent;
        if (!ts.isVariableDeclaration(variable) || !variable.initializer)
          continue;
        const owner = origin(variable.initializer, seen);
        if (declaration.dotDotDotToken) return owner;
        const property = declaration.propertyName ?? declaration.name;
        return memberOrigin(
          owner,
          ts.isIdentifier(property) || ts.isStringLiteral(property)
            ? property.text
            : null
        );
      }
    }
    return null;
  }
  const usages: string[] = [];
  function visit(node: ts.Node) {
    if (
      ts.isCallExpression(node) &&
      origin(node.expression) === 'static-method'
    )
      usages.push(location(entry, node));
    ts.forEachChild(node, visit);
  }
  visit(entry.sourceFile);
  return usages;
}

const sourceEntries = collectSourceEntries();

describe('Ant Design v6 structural compatibility', () => {
  test('does not use the deprecated List component', () => {
    const usages: string[] = [];

    for (const entry of sourceEntries) {
      const imports = antdNamedImports(entry.sourceFile);

      function visit(node: ts.Node) {
        if (
          (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) &&
          ts.isIdentifier(node.tagName) &&
          imports.get(node.tagName.text) === 'List'
        ) {
          usages.push(location(entry, node));
        }
        ts.forEachChild(node, visit);
      }

      visit(entry.sourceFile);
    }

    expect(usages).toEqual([]);
  });

  test('does not use deprecated Input addon props', () => {
    const usages: string[] = [];

    for (const entry of sourceEntries) {
      const imports = antdNamedImports(entry.sourceFile);

      function visit(node: ts.Node) {
        if (
          (ts.isJsxOpeningElement(node) || ts.isJsxSelfClosingElement(node)) &&
          ts.isIdentifier(node.tagName) &&
          imports.get(node.tagName.text) === 'Input'
        ) {
          for (const attribute of node.attributes.properties) {
            if (
              ts.isJsxAttribute(attribute) &&
              ts.isIdentifier(attribute.name) &&
              ['addonBefore', 'addonAfter'].includes(attribute.name.text)
            ) {
              usages.push(
                `${location(entry, attribute)} ${attribute.name.text}`
              );
            }
          }
        }
        ts.forEachChild(node, visit);
      }

      visit(entry.sourceFile);
    }

    expect(usages).toEqual([]);
  });

  test('does not call the context-free static message API', () => {
    const usages = sourceEntries.flatMap(staticMessageCalls);
    expect(usages).toEqual([]);
  });

  test('distinguishes message hooks from context-free calls, including aliases', () => {
    function calls(source: string) {
      return staticMessageCalls({
        file: 'fixture.ts',
        sourceFile: ts.createSourceFile(
          'fixture.ts',
          source,
          ts.ScriptTarget.Latest,
          true
        )
      });
    }
    expect(
      calls("import { message as notices } from 'antd'; notices.useMessage();")
    ).toEqual([]);
    expect(
      calls(
        "import { message as notices } from 'antd'; notices.error('failed');"
      )
    ).toHaveLength(1);
    expect(
      calls("import { message } from 'antd'; message['success']('saved');")
    ).toHaveLength(1);
    expect(
      calls(
        "import { message } from 'antd'; const notices = message; notices.error('failed');"
      )
    ).toHaveLength(1);
    expect(
      calls(
        "import { message } from 'antd'; const { error: showError } = message; showError('failed');"
      )
    ).toHaveLength(1);
    expect(
      calls(
        "import { message } from 'antd'; const showError = message.error; showError('failed');"
      )
    ).toHaveLength(1);
    expect(
      calls("import * as antd from 'antd'; antd.message.error('failed');")
    ).toHaveLength(1);
    expect(
      calls("import message from 'antd/es/message'; message.error('failed');")
    ).toHaveLength(1);
    expect(
      calls(
        "import { message } from 'antd'; const { useMessage } = message; useMessage();"
      )
    ).toEqual([]);
    expect(
      calls(
        "import { message } from 'antd'; function notify(message: { error(): void }) { message.error(); }"
      )
    ).toEqual([]);
    expect(
      calls(
        "import { message } from 'antd'; const [api] = message.useMessage(); api.error('failed');"
      )
    ).toEqual([]);
  });

  test('does not render legacy action-array separators', () => {
    const usages: string[] = [];

    for (const entry of sourceEntries) {
      function visit(node: ts.Node) {
        if (ts.isJsxElement(node)) {
          const className = node.openingElement.attributes.properties.find(
            (attribute) =>
              ts.isJsxAttribute(attribute) &&
              ts.isIdentifier(attribute.name) &&
              attribute.name.text === 'className'
          );
          const isStructuredListActions =
            className &&
            ts.isJsxAttribute(className) &&
            className.initializer &&
            ts.isStringLiteral(className.initializer) &&
            className.initializer.text === 'structured-list__actions';

          if (
            isStructuredListActions &&
            node.children.some(
              (child) => ts.isJsxText(child) && child.text.trim() === ','
            )
          ) {
            usages.push(location(entry, node));
          }
        }
        ts.forEachChild(node, visit);
      }

      visit(entry.sourceFile);
    }

    expect(usages).toEqual([]);
  });
});
