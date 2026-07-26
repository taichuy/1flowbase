import type { NativeReactCompileDiagnostic } from './artifact';

export const LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC = {
  phase: 'compile',
  code: 'transform_failed',
  path: 'source.contract',
  message:
    'Legacy BlockModule.main(ctx) source is not supported by the Native React Host. Export a standard React component as default.'
} as const satisfies NativeReactCompileDiagnostic;

export class NativeReactSourceContractError extends Error {
  readonly diagnostic: NativeReactCompileDiagnostic;

  constructor(
    diagnostic: NativeReactCompileDiagnostic = LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC
  ) {
    super(diagnostic.message);
    this.name = 'NativeReactSourceContractError';
    this.diagnostic = { ...diagnostic };
  }
}

const LEGACY_MODULE_IMPORT_PATTERN =
  /(?:^|\n)\s*import\s+[\s\S]*?\sfrom\s*['"](?:@1flowbase\/block-renderer\/antd-facade|@1flowbase\/antd-facade)['"]/u;
const LEGACY_BLOCK_MODULE_TYPE_PATTERN = /\bsatisfies\s+BlockModule\b/u;
const LEGACY_DEFAULT_MAIN_OBJECT_PATTERN =
  /\bexport\s+default\s*\{[^}]*\bmain\b[^}]*\}/u;

export function diagnoseLegacyBlockModuleSource(
  source: unknown
): NativeReactCompileDiagnostic | null {
  if (typeof source !== 'string') return null;
  const contractSource = maskCommentsAndStrings(source);
  return LEGACY_MODULE_IMPORT_PATTERN.test(source) ||
    LEGACY_BLOCK_MODULE_TYPE_PATTERN.test(contractSource) ||
    LEGACY_DEFAULT_MAIN_OBJECT_PATTERN.test(contractSource)
    ? { ...LEGACY_BLOCK_MODULE_SOURCE_DIAGNOSTIC }
    : null;
}

function maskCommentsAndStrings(source: string): string {
  let masked = '';
  let index = 0;
  while (index < source.length) {
    const current = source[index];
    const next = source[index + 1];
    if (current === '/' && next === '/') {
      const end = source.indexOf('\n', index + 2);
      const stop = end < 0 ? source.length : end;
      masked += ' '.repeat(stop - index);
      index = stop;
      continue;
    }
    if (current === '/' && next === '*') {
      const end = source.indexOf('*/', index + 2);
      const stop = end < 0 ? source.length : end + 2;
      masked += source
        .slice(index, stop)
        .replace(/[^\n]/gu, ' ');
      index = stop;
      continue;
    }
    if (current === "'" || current === '"' || current === '`') {
      const quote = current;
      let stop = index + 1;
      while (stop < source.length) {
        if (source[stop] === '\\') {
          stop += 2;
          continue;
        }
        stop += 1;
        if (source[stop - 1] === quote) break;
      }
      masked += source
        .slice(index, stop)
        .replace(/[^\n]/gu, ' ');
      index = stop;
      continue;
    }
    masked += current;
    index += 1;
  }
  return masked;
}
