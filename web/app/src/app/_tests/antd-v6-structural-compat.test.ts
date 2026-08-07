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

  test('does not import the static message API', () => {
    const usages = sourceEntries.flatMap((entry) =>
      [...antdNamedImports(entry.sourceFile).entries()]
        .filter(([, imported]) => imported === 'message')
        .map(([local]) => `${entry.file} imports ${local}`)
    );

    expect(usages).toEqual([]);
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
