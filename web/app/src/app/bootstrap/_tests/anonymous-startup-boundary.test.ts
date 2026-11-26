import { readFileSync, existsSync } from 'node:fs';
import { dirname, resolve, relative } from 'node:path';
import { parseSync } from 'vite';
import { expect, test } from 'vitest';

test('the sign-in-preloaded bootstrap has no static private runtime/API dependency', () => {
  const root = resolve(process.cwd(), 'src');
  const visited = new Set<string>();
  const specifiers = new Set<string>();
  const walk = (path: string) => {
    if (visited.has(path)) return;
    visited.add(path);
    const code = readFileSync(path, 'utf8');
    const parsed = parseSync(path, code);
    expect(parsed.errors).toEqual([]);
    for (const statement of parsed.program.body) {
      let specifier: string | undefined;
      if (statement.type === 'ImportDeclaration') {
        if (statement.importKind === 'type') continue;
        if (
          statement.specifiers.length === 0 &&
          /\bfrom\s*['"]/u.test(code.slice(statement.start, statement.end))
        )
          continue;
        if (
          statement.specifiers.length > 0 &&
          statement.specifiers.every(
            (element) =>
              element.type === 'ImportSpecifier' &&
              element.importKind === 'type'
          )
        )
          continue;
        specifier = statement.source.value;
      } else if (statement.type === 'ExportNamedDeclaration') {
        if (!statement.source || statement.exportKind === 'type') continue;
        if (
          statement.specifiers.every((element) => element.exportKind === 'type')
        )
          continue;
        specifier = statement.source.value;
      } else if (statement.type === 'ExportAllDeclaration') {
        if (statement.exportKind === 'type') continue;
        specifier = statement.source.value;
      }
      if (!specifier) continue;
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
  const paths = [...visited].map((path) =>
    relative(root, path).replaceAll('\\', '/')
  );
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
