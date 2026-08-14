import { transform } from 'sucrase';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  type NativeReactCatalogDependencyLock
} from '@1flowbase/page-runtime/dependency-lock';

import { TAILWIND_STYLESHEET_SHA256 } from './stylesheet-contract.ts';

export const TAILWIND_BLOCK_PRESET_ASSET = Object.freeze({
  path: 'tailwindcss-catalog.css',
  role: 'shadow_style' as const,
  media_type: 'text/css; charset=utf-8',
  sha256: '77c009cb4826b765d416513e3d9c83093482ecb69de9e361e4c25f5441240b36'
});

export const TAILWIND_4_3_3_COMPILER_IDENTITY = Object.freeze({
  name: '@1flowbase/tailwindcss-catalog',
  contract: 'block-preset-v1',
  tailwind_version: '4.3.3'
});

export const TAILWIND_4_3_3_TOOLCHAIN_LOCK = Object.freeze({
  package: 'tailwindcss',
  version: '4.3.3',
  mode: 'block-preset'
});

export const TAILWIND_4_3_3_STYLESHEET_IDENTITY = Object.freeze({
  package: 'tailwindcss',
  version: '4.3.3',
  mode: 'block-preset',
  sha256: TAILWIND_STYLESHEET_SHA256
});

export const TAILWIND_4_3_3_ARTIFACT_IDENTITY = Object.freeze({
  name: '@1flowbase/tailwindcss-catalog/compiler',
  version: '4.3.3',
  contract: 'executable-compiler-v1',
  preset: 'default-utilities-standard-variants-v1',
  preset_asset_sha256: TAILWIND_BLOCK_PRESET_ASSET.sha256,
  stylesheet_sha256: TAILWIND_STYLESHEET_SHA256,
  tsx_validation: 'sucrase@3.35.1/dependency-lock-imports-v2'
});

// Digest of the canonical TAILWIND_4_3_3_ARTIFACT_IDENTITY JSON value.
export const TAILWIND_4_3_3_ARTIFACT_SHA256 =
  '2005c459882fcaeb283ff36706b327efebf8783414ecaa111f92f628c4ba0af8';

export interface TailwindExecutableCompilerRequest {
  source_code: string;
  dependency_lock: NativeReactCatalogDependencyLock;
  compiler_identity: Record<string, string>;
  toolchain_lock: Record<string, string>;
}

export interface TailwindValidationDiagnostic {
  phase: 'validation';
  code:
    | 'invalid_request'
    | 'invalid_dependency_lock'
    | 'unknown_compiler_identity'
    | 'unknown_toolchain_lock'
    | 'tsx_transform_failed'
    | 'illegal_import'
    | 'compiler_failed';
  path: string;
  message: string;
  source_location?: { line: number; column: number };
}

interface CanonicalExecutableIdentities {
  compiler_identity: typeof TAILWIND_4_3_3_COMPILER_IDENTITY;
  toolchain_lock: typeof TAILWIND_4_3_3_TOOLCHAIN_LOCK;
  stylesheet_identity: typeof TAILWIND_4_3_3_STYLESHEET_IDENTITY;
  artifact_identity: typeof TAILWIND_4_3_3_ARTIFACT_IDENTITY;
  artifact_sha256: string;
}

export type TailwindExecutableCompilerResult =
  | ({
      ok: true;
      validation_diagnostics: [];
      generated_css: string;
      generated_css_sha256: string;
      source_sha256: string;
      dependency_lock: NativeReactCatalogDependencyLock;
    } & CanonicalExecutableIdentities)
  | {
      ok: false;
      error: { code: TailwindValidationDiagnostic['code']; message: string };
      validation_diagnostics: TailwindValidationDiagnostic[];
    };

interface ExecutableCompilerVersion extends CanonicalExecutableIdentities {
  compile(
    sourceCode: string,
    dependencyLock: NativeReactCatalogDependencyLock
  ): Promise<{
    generatedCss: string;
    diagnostics: TailwindValidationDiagnostic[];
  }>;
}

const hostBaselineImports = new Set(['react', 'react/jsx-runtime', 'antd']);

const tailwind433: ExecutableCompilerVersion = Object.freeze({
  compiler_identity: TAILWIND_4_3_3_COMPILER_IDENTITY,
  toolchain_lock: TAILWIND_4_3_3_TOOLCHAIN_LOCK,
  stylesheet_identity: TAILWIND_4_3_3_STYLESHEET_IDENTITY,
  artifact_identity: TAILWIND_4_3_3_ARTIFACT_IDENTITY,
  artifact_sha256: TAILWIND_4_3_3_ARTIFACT_SHA256,
  async compile(
    sourceCode: string,
    dependencyLock: NativeReactCatalogDependencyLock
  ) {
    const diagnostics = validateTsxSource(sourceCode, dependencyLock);
    if (diagnostics.length > 0) return { generatedCss: '', diagnostics };
    return { generatedCss: '', diagnostics: [] };
  }
});

const compilerVersions: readonly ExecutableCompilerVersion[] = Object.freeze([
  tailwind433
]);

export async function compileTailwindExecutableArtifact(
  request: unknown
): Promise<TailwindExecutableCompilerResult> {
  const requestDiagnostic = validateRequest(request);
  if (requestDiagnostic) return failure([requestDiagnostic]);

  const typedRequest = request as TailwindExecutableCompilerRequest;
  const version = compilerVersions.find((candidate) =>
    exactRecord(candidate.toolchain_lock, typedRequest.toolchain_lock)
  );
  if (!version) {
    return failure([
      diagnostic(
        'unknown_toolchain_lock',
        'toolchain_lock',
        'No executable Tailwind compiler is registered for the exact toolchain lock.'
      )
    ]);
  }
  if (!exactRecord(version.compiler_identity, typedRequest.compiler_identity)) {
    return failure([
      diagnostic(
        'unknown_compiler_identity',
        'compiler_identity',
        'The compiler identity does not match the selected toolchain lock.'
      )
    ]);
  }

  try {
    const canonicalDependencyLock = canonicalizeNativeReactCatalogDependencyLock(
      typedRequest.dependency_lock
    );
    if (!canonicalDependencyLock) {
      return failure([
        diagnostic(
          'invalid_dependency_lock',
          'dependency_lock',
          'dependency_lock must satisfy the canonical Native React catalog lock contract.'
        )
      ]);
    }
    const dependencyLock = withTailwindBlockPresetAsset(
      canonicalDependencyLock
    );
    const compiled = await version.compile(
      typedRequest.source_code,
      dependencyLock
    );
    if (compiled.diagnostics.length > 0) return failure(compiled.diagnostics);
    return {
      ok: true,
      validation_diagnostics: [],
      generated_css: compiled.generatedCss,
      generated_css_sha256: await sha256Text(compiled.generatedCss),
      source_sha256: await sha256Text(typedRequest.source_code),
      dependency_lock: dependencyLock,
      compiler_identity: version.compiler_identity,
      toolchain_lock: version.toolchain_lock,
      stylesheet_identity: version.stylesheet_identity,
      artifact_identity: version.artifact_identity,
      artifact_sha256: version.artifact_sha256
    };
  } catch {
    return failure([
      diagnostic(
        'compiler_failed',
        'compiler',
        'Executable Tailwind compiler failed.'
      )
    ]);
  }
}

function withTailwindBlockPresetAsset(
  dependencyLock: NativeReactCatalogDependencyLock
): NativeReactCatalogDependencyLock {
  return dependencyLock.map((entry) => {
    if (entry.module_source !== 'tailwindcss') return entry;
    const existing = entry.assets.find(
      (asset) => asset.role === TAILWIND_BLOCK_PRESET_ASSET.role
    );
    if (existing) {
      if (
        existing.sha256 !== TAILWIND_BLOCK_PRESET_ASSET.sha256 ||
        existing.media_type !== TAILWIND_BLOCK_PRESET_ASSET.media_type
      ) {
        throw new Error('Tailwind block preset asset identity mismatch.');
      }
      return entry;
    }
    const browserAsset = entry.assets.find(
      (asset) => asset.role === 'browser_module'
    );
    const assetUrl = browserAsset?.url.replace(
      /[a-f0-9]{64}$/u,
      TAILWIND_BLOCK_PRESET_ASSET.sha256
    );
    if (!assetUrl || assetUrl === browserAsset?.url) {
      throw new Error('Tailwind browser asset URL cannot derive the preset URL.');
    }
    return {
      ...entry,
      assets: [
        ...entry.assets,
        {
          role: TAILWIND_BLOCK_PRESET_ASSET.role,
          media_type: TAILWIND_BLOCK_PRESET_ASSET.media_type,
          sha256: TAILWIND_BLOCK_PRESET_ASSET.sha256,
          url: assetUrl
        }
      ]
    };
  });
}

function validateRequest(
  request: unknown
): TailwindValidationDiagnostic | undefined {
  if (
    typeof request !== 'object' ||
    request === null ||
    Array.isArray(request)
  ) {
    return diagnostic(
      'invalid_request',
      'request',
      'Compiler request must be a JSON object.'
    );
  }
  const value = request as Record<string, unknown>;
  if (typeof value.source_code !== 'string') {
    return diagnostic(
      'invalid_request',
      'source_code',
      'source_code must be a string.'
    );
  }
  if (!Array.isArray(value.dependency_lock)) {
    return diagnostic(
      'invalid_dependency_lock',
      'dependency_lock',
      'dependency_lock must be an array.'
    );
  }
  for (const field of ['compiler_identity', 'toolchain_lock'] as const) {
    if (!isStringRecord(value[field])) {
      return diagnostic(
        'invalid_request',
        field,
        `${field} must be a non-empty string record.`
      );
    }
  }
  return undefined;
}

function validateTsxSource(
  source: string,
  dependencyLock: NativeReactCatalogDependencyLock
): TailwindValidationDiagnostic[] {
  const allowedImports = new Set(hostBaselineImports);
  for (const entry of dependencyLock) allowedImports.add(entry.module_source);
  const imports = readStaticImports(source);
  const illegalImport = imports.find(
    (moduleSource) => !allowedImports.has(moduleSource)
  );
  if (illegalImport) {
    return [
      diagnostic(
        'illegal_import',
        'source_code.imports',
        `Import '${illegalImport}' is not allowed by the executable compiler contract.`
      )
    ];
  }
  if (/\bimport\s*\(/u.test(source)) {
    return [
      diagnostic(
        'illegal_import',
        'source_code.imports',
        'Dynamic import is not allowed by the executable compiler contract.'
      )
    ];
  }
  try {
    transform(source, {
      transforms: ['typescript', 'jsx'],
      jsxRuntime: 'automatic',
      jsxImportSource: 'react',
      production: true,
      filePath: 'native-react-block.tsx'
    });
    return [];
  } catch (error) {
    const location = readSourceLocation(error);
    return [
      {
        ...diagnostic(
          'tsx_transform_failed',
          'source_code',
          `TSX validation failed: ${readErrorMessage(error)}`
        ),
        ...(location ? { source_location: location } : {})
      }
    ];
  }
}

function readStaticImports(source: string): string[] {
  const imports: string[] = [];
  const pattern =
    /(?:\bimport\s+(?:[^'";]+?\s+from\s+)?|\bexport\s+[^'";]+?\s+from\s+)['"]([^'"]+)['"]/gu;
  for (const match of source.matchAll(pattern)) imports.push(match[1]);
  return imports;
}

function readSourceLocation(
  error: unknown
): { line: number; column: number } | undefined {
  if (typeof error !== 'object' || error === null) return undefined;
  const loc = (error as { loc?: unknown }).loc;
  if (typeof loc !== 'object' || loc === null) return undefined;
  const line = (loc as { line?: unknown }).line;
  const column = (loc as { column?: unknown }).column;
  return typeof line === 'number' && typeof column === 'number'
    ? { line, column: column + 1 }
    : undefined;
}

function readErrorMessage(error: unknown): string {
  return error instanceof Error && error.message
    ? error.message
    : 'Unknown transform error.';
}

function isStringRecord(value: unknown): value is Record<string, string> {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length > 0 &&
    Object.values(value).every((entry) => typeof entry === 'string')
  );
}

function exactRecord(
  expected: Record<string, string>,
  actual: Record<string, string>
): boolean {
  const expectedKeys = Object.keys(expected).sort();
  const actualKeys = Object.keys(actual).sort();
  return (
    expectedKeys.length === actualKeys.length &&
    expectedKeys.every(
      (key, index) => key === actualKeys[index] && expected[key] === actual[key]
    )
  );
}

function diagnostic(
  code: TailwindValidationDiagnostic['code'],
  path: string,
  message: string
): TailwindValidationDiagnostic {
  return { phase: 'validation', code, path, message };
}

function failure(
  diagnostics: TailwindValidationDiagnostic[]
): Extract<TailwindExecutableCompilerResult, { ok: false }> {
  const first = diagnostics[0];
  return {
    ok: false,
    error: { code: first.code, message: first.message },
    validation_diagnostics: diagnostics
  };
}

async function sha256Text(value: string): Promise<string> {
  const digest = await globalThis.crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(value)
  );
  return Array.from(new Uint8Array(digest), (byte) =>
    byte.toString(16).padStart(2, '0')
  ).join('');
}
