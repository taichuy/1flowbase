import { transform, type TransformResult } from 'sucrase';

import type { BlockProtocolError } from '@1flowbase/page-protocol';

export type NativeReactTsxTransformResult =
  | {
      ok: true;
      code: string;
      sourceMap: TransformResult['sourceMap'];
    }
  | { ok: false; errors: BlockProtocolError[] };

/** Compiles author TSX with React's standard automatic JSX runtime. */
export function transformNativeReactTsx(
  source: string
): NativeReactTsxTransformResult {
  try {
    const transformed = transform(source, {
      transforms: ['jsx', 'typescript'],
      jsxRuntime: 'automatic',
      jsxImportSource: 'react',
      production: true,
      disableESTransforms: true,
      keepUnusedImports: true,
      filePath: 'native-react-block.tsx',
      sourceMapOptions: { compiledFilename: 'native-react-block.js' }
    });
    return {
      ok: true,
      code: transformed.code,
      sourceMap: transformed.sourceMap
    };
  } catch (error) {
    return {
      ok: false,
      errors: [
        {
          code: 'transform_failed',
          path: 'source.tsx',
          message: `Native React component compilation failed: ${readErrorMessage(error)}`,
          ...readCompileSourceLocation(error)
        }
      ]
    };
  }
}

function readCompileSourceLocation(error: unknown): {
  sourceLocation?: { line: number; column: number };
} {
  if (typeof error !== 'object' || error === null) return {};
  const loc = (error as { loc?: unknown }).loc;
  if (typeof loc !== 'object' || loc === null) return {};
  const line = (loc as { line?: unknown }).line;
  const column = (loc as { column?: unknown }).column;
  return typeof line === 'number' && typeof column === 'number'
    ? { sourceLocation: { line, column: column + 1 } }
    : {};
}

function readErrorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : 'Unknown transform error.';
}
