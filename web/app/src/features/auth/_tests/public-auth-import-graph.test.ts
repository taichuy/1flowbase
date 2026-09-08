import fs from 'node:fs';
import path from 'node:path';
import ts from 'typescript';
import { expect, test } from 'vitest';

// Walk emitted static imports, as Vite's unbundled browser graph does. Worker
// entries and dynamic imports are separate execution/loading boundaries.
test('I2012-AC-001 keeps TSX compilation and legacy JSX transformation out of the public auth main-thread graph', () => {
  const appRoot = process.cwd();
  const runtimeRoot = path.resolve(appRoot, '../packages/page-runtime/src');
  const visited = new Set<string>();
  function visit(file: string) {
    if (visited.has(file)) return;
    visited.add(file);
    const emitted = ts.transpileModule(fs.readFileSync(file, 'utf8'), {
      compilerOptions: {
        module: ts.ModuleKind.ESNext,
        jsx: ts.JsxEmit.Preserve,
        target: ts.ScriptTarget.ES2022
      }
    }).outputText;
    const syntax = ts.createSourceFile(
      file,
      emitted,
      ts.ScriptTarget.ES2022,
      true
    );
    for (const statement of syntax.statements) {
      if (
        !ts.isImportDeclaration(statement) &&
        !ts.isExportDeclaration(statement)
      )
        continue;
      const specifier = statement.moduleSpecifier;
      if (!specifier || !ts.isStringLiteral(specifier)) continue;
      const source = specifier.text;
      if (source.includes('?') || source.endsWith('.css')) continue;
      let base: string;
      if (source === '@1flowbase/page-runtime')
        base = path.join(runtimeRoot, 'index');
      else if (source === '@1flowbase/page-runtime/browser')
        base = path.join(runtimeRoot, 'entrypoints/browser');
      else if (source === '@1flowbase/page-runtime/source-contract')
        base = path.join(runtimeRoot, 'native-react-compiler/source-contract');
      else if (source.startsWith('.'))
        base = path.resolve(path.dirname(file), source);
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
      if (resolved) visit(resolved);
    }
  }
  visit(path.join(appRoot, 'src/features/auth/components/PublicAuthBlock.tsx'));
  const graph = [...visited].map((file) => path.relative(runtimeRoot, file));
  expect(graph).not.toContain('native-react-compiler/tsx-transform.ts');
  expect(graph).not.toContain('native-trusted-block/jsx-transform.ts');
  expect(graph).not.toContain('native-react-compiler/worker-protocol.ts');
});
