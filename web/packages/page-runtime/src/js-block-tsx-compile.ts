import { transform } from 'sucrase';

import type { BlockProtocolError } from '@1flowbase/page-protocol';

export const JSX_PRAGMA = 'h';
export const JSX_FRAGMENT_PRAGMA = 'Fragment';
export const JSX_RUNTIME_IMPORT_SOURCE =
  '@1flowbase/block-renderer/antd-facade';

export type JsBlockTsxCompileResult =
  | { ok: true; code: string; changed: boolean }
  | { ok: false; errors: BlockProtocolError[] };

export function compileJsBlockTsxSource(
  source: string
): JsBlockTsxCompileResult {
  let compiled: string;
  try {
    compiled = transform(source, {
      transforms: ['jsx', 'typescript'],
      jsxPragma: JSX_PRAGMA,
      jsxFragmentPragma: JSX_FRAGMENT_PRAGMA,
      production: true,
      disableESTransforms: true,
      keepUnusedImports: true
    }).code;
  } catch (error) {
    return {
      ok: false,
      errors: [
        {
          code: 'transform_failed',
          path: 'source.tsx',
          message: `Code block TSX compile failed: ${
            error instanceof Error ? error.message : String(error)
          }`
        }
      ]
    };
  }

  if (compiled === source) {
    return { ok: true, code: compiled, changed: false };
  }

  return {
    ok: true,
    code: ensureJsxRuntimeImport(source, compiled),
    changed: true
  };
}

function ensureJsxRuntimeImport(original: string, compiled: string): string {
  const usesPragma =
    new RegExp(`\\b${JSX_PRAGMA}\\s*\\(`).test(compiled) ||
    new RegExp(`\\b${JSX_FRAGMENT_PRAGMA}\\b`).test(compiled);
  if (!usesPragma) {
    return compiled;
  }

  const importedNames = collectFacadeImportNames(original);
  const missing = [JSX_PRAGMA, JSX_FRAGMENT_PRAGMA].filter(
    (name) => !importedNames.has(name)
  );
  if (missing.length === 0) {
    return compiled;
  }

  return `import { ${missing.join(', ')} } from '${JSX_RUNTIME_IMPORT_SOURCE}';\n${compiled}`;
}

function collectFacadeImportNames(source: string): Set<string> {
  const names = new Set<string>();
  const importPattern =
    /import\s*\{([^}]*)\}\s*from\s*['"]@1flowbase\/block-renderer\/antd-facade['"]/g;
  for (const match of source.matchAll(importPattern)) {
    for (const binding of match[1].split(',')) {
      const name = binding.split(' as ')[0]?.trim();
      if (name) {
        names.add(name);
      }
    }
  }
  return names;
}
