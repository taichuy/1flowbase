import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve, relative } from 'node:path';
import ts from 'typescript';
import { expect, test } from 'vitest';

test('the sign-in-preloaded bootstrap has no static private runtime/API dependency', () => {
  const root = fileURLToPath(new URL('../../../', import.meta.url));
  const visited = new Set<string>();
  const specifiers = new Set<string>();
  const walk = (path: string) => {
    if (visited.has(path)) return;
    visited.add(path);
    const source = ts.createSourceFile(
      path,
      readFileSync(path, 'utf8'),
      ts.ScriptTarget.Latest,
      true
    );
    for (const statement of source.statements) {
      if (
        !ts.isImportDeclaration(statement) &&
        !ts.isExportDeclaration(statement)
      )
        continue;
      if (
        !statement.moduleSpecifier ||
        !ts.isStringLiteral(statement.moduleSpecifier)
      )
        continue;
      if (ts.isImportDeclaration(statement)) {
        const clause = statement.importClause;
        if (clause?.isTypeOnly) continue;
        if (
          clause &&
          !clause.name &&
          clause.namedBindings &&
          ts.isNamedImports(clause.namedBindings) &&
          clause.namedBindings.elements.every((element) => element.isTypeOnly)
        )
          continue;
      } else {
        if (statement.isTypeOnly) continue;
        if (
          statement.exportClause &&
          ts.isNamedExports(statement.exportClause) &&
          statement.exportClause.elements.every((element) => element.isTypeOnly)
        )
          continue;
      }
      const specifier = statement.moduleSpecifier.text;
      specifiers.add(specifier);
      if (!specifier.startsWith('.')) continue;
      const local = resolve(dirname(path), specifier);
      const target = [
        local,
        `${local}.ts`,
        `${local}.tsx`,
        `${local}/index.ts`,
        `${local}/index.tsx`
      ].find((candidate) => /\.tsx?$/.test(candidate) && existsSync(candidate));
      if (target) walk(target);
    }
  };
  for (const entry of [
    'App',
    'ApplicationRuntimeBootstrap',
    'AnonymousAppRuntime',
    'public-router'
  ]) {
    walk(resolve(root, `app/${entry}.tsx`));
  }
  const paths = [...visited].map((path) => relative(root, path));
  expect(paths).not.toContain('app/bootstrap/authenticated-navigation.ts');
  expect(paths).not.toContain('app/bootstrap/AuthenticatedStartup.tsx');
  expect(
    paths.some(
      (path) =>
        path.startsWith('features/frontstage/') ||
        path.startsWith('features/settings/')
    )
  ).toBe(false);
  expect(specifiers.has('@tanstack/react-query')).toBe(false);
  expect(specifiers.has('@1flowbase/api-client')).toBe(false);
  expect(
    [...specifiers].filter((name) =>
      /^(antd(?:\/|$)|antd-style(?:\/|$)|@ant-design\/(?:icons|cssinjs)|@tanstack\/(?:react-router|react-query)|i18next(?:\/|$)|react-i18next(?:\/|$)|@1flowbase\/ui(?:\/|$))/.test(
        name
      )
    )
  ).toEqual([]);
  for (const heavy of [
    'features/auth/components/PublicAuthBlock.tsx',
    'features/auth/components/PublicAuthProviders.tsx',
    'features/auth/pages/mcp-authorization/McpAuthorizePage.tsx',
    'shared/i18n/app-i18n.ts'
  ])
    expect(paths).not.toContain(heavy);
  // Dynamic imports are deliberately excluded; the production chunk graph is a
  // separate Root check against accidental bundler coalescing of this boundary.
});
