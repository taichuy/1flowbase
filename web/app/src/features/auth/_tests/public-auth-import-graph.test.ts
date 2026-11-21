import fs from 'node:fs';
import path from 'node:path';
import { parseSync, transformWithOxc } from 'vite';
import { expect, test } from 'vitest';

const toPosixPath = (value: string) => value.replaceAll('\\', '/');

// Walk emitted static imports, as Vite's unbundled browser graph does. Worker
// entries and dynamic imports are separate execution/loading boundaries.
test('I2012-AC-001 keeps TSX compilation and legacy JSX transformation out of the public auth main-thread graph', async () => {
  const appRoot = process.cwd();
  const runtimeRoot = path.resolve(appRoot, '../packages/page-runtime/src');
  const visited = new Set<string>();
  async function visit(file: string) {
    const resolvedFile = path.resolve(file);
    if (visited.has(resolvedFile)) return;
    visited.add(resolvedFile);
    const rawSource = fs.readFileSync(resolvedFile, 'utf8');
    if (resolvedFile.endsWith('.json')) {
      JSON.parse(rawSource);
      return;
    }
    const posixFile = toPosixPath(resolvedFile);
    const emitted = (
      await transformWithOxc(rawSource, posixFile, {
        jsx: 'preserve',
        target: 'es2022'
      })
    ).code;
    const parsed = parseSync(posixFile, emitted);
    expect(parsed.errors).toEqual([]);
    for (const statement of parsed.program.body) {
      if (
        statement.type !== 'ImportDeclaration' &&
        statement.type !== 'ExportNamedDeclaration' &&
        statement.type !== 'ExportAllDeclaration'
      )
        continue;
      const source = statement.source?.value;
      if (!source) continue;
      if (source.includes('?') || source.endsWith('.css')) continue;
      let base: string;
      if (source === '@1flowbase/page-runtime')
        base = path.join(runtimeRoot, 'index');
      else if (source === '@1flowbase/page-runtime/browser')
        base = path.join(runtimeRoot, 'entrypoints/browser');
      else if (source === '@1flowbase/page-runtime/source-contract')
        base = path.join(runtimeRoot, 'native-react-compiler/source-contract');
      else if (source.startsWith('.'))
        base = path.resolve(path.dirname(resolvedFile), source);
      else continue;
      const resolved = [
        base,
        ...['.ts', '.tsx', '/index.ts', '/index.tsx'].map(
          (extension) => base + extension
        )
      ].find(
        (candidate) =>
          fs.existsSync(candidate) && fs.statSync(candidate).isFile()
      );
      if (resolved) await visit(resolved);
    }
  }
  await visit(
    path.join(appRoot, 'src/features/auth/components/PublicAuthBlock.tsx')
  );
  const graph = [...visited].map((file) =>
    toPosixPath(path.relative(runtimeRoot, file))
  );
  expect(
    [...visited].some((file) =>
      toPosixPath(file).endsWith('/frontstage/lib/native-modules/registry.ts')
    )
  ).toBe(false);
  expect(graph).not.toContain('native-react-compiler/tsx-transform.ts');
  expect(graph).not.toContain('native-trusted-block/jsx-transform.ts');
  expect(graph).not.toContain('native-react-compiler/worker-protocol.ts');
});
