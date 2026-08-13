import type {
  ConsoleFrontstageBlockCode,
  ConsoleFrontstageBlockRuntimeLayer
} from '@1flowbase/api-client';
import {
  canonicalizeNativeReactCatalogDependencyLock,
  sha256Text,
  type NativeReactCatalogDependencyLock,
  type NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';
import {
  compileTailwindExecutableArtifact,
  TAILWIND_4_3_3_COMPILER_IDENTITY,
  TAILWIND_4_3_3_TOOLCHAIN_LOCK
} from '@1flowbase/tailwindcss-catalog/executable-contract';

export const NATIVE_REACT_TAILWIND_COMPILER_IDENTITY = Object.freeze({
  ...TAILWIND_4_3_3_COMPILER_IDENTITY
});

export const NATIVE_REACT_TAILWIND_TOOLCHAIN_LOCK = Object.freeze({
  ...TAILWIND_4_3_3_TOOLCHAIN_LOCK
});

type ExecutableDto =
  | Pick<
      ConsoleFrontstageBlockCode,
      | 'source_code'
      | 'source_sha256'
      | 'dependency_lock'
      | 'tailwind_toolchain_lock'
      | 'generated_css'
      | 'generated_css_sha256'
      | 'compiler_identity'
      | 'executable_state'
    >
  | Pick<
      ConsoleFrontstageBlockRuntimeLayer,
      | 'source_code'
      | 'source_sha256'
      | 'dependency_lock'
      | 'tailwind_toolchain_lock'
      | 'generated_css'
      | 'generated_css_sha256'
      | 'compiler_identity'
      | 'executable_state'
    >;

export interface LockedNativeReactExecutableStyle {
  source_code: string;
  source_sha256: string;
  dependency_lock: NativeReactCatalogDependencyLock;
  generated_css: string;
  generated_css_sha256: string;
  tailwind_toolchain_lock: Record<string, string>;
  compiler_identity: Record<string, string>;
  executable_style_identity: string;
  shadow_style_asset: NativeReactResolvedModuleAsset;
}

export interface NativeReactExecutableStyleCompilation {
  generated_css: string;
  generated_css_sha256: string;
  tailwind_toolchain_lock: Record<string, string>;
  compiler_identity: Record<string, string>;
}

export function readLockedNativeReactExecutableStyle(
  value: ExecutableDto
): LockedNativeReactExecutableStyle {
  if (
    value.executable_state !== 'ready' ||
    !value.source_sha256 ||
    !value.dependency_lock ||
    !isNonEmptyStringRecord(value.tailwind_toolchain_lock) ||
    value.generated_css === null ||
    !value.generated_css_sha256 ||
    !isNonEmptyStringRecord(value.compiler_identity)
  ) {
    throw new Error(
      'Frontstage block executable state is legacy or incomplete.'
    );
  }
  const dependencyLock = canonicalizeNativeReactCatalogDependencyLock(
    value.dependency_lock
  );
  if (!dependencyLock) {
    throw new Error('Frontstage block dependency_lock is invalid.');
  }
  const sourceSha256 = value.source_sha256.toLowerCase();
  const generatedCssSha256 = value.generated_css_sha256.toLowerCase();
  if (sha256Text(value.source_code) !== sourceSha256) {
    throw new Error(
      'Frontstage block source_sha256 does not match source_code.'
    );
  }
  if (sha256Text(value.generated_css) !== generatedCssSha256) {
    throw new Error(
      'Frontstage block generated_css_sha256 does not match generated_css.'
    );
  }
  const executableStyleIdentity = sha256Text(
    JSON.stringify(
      canonicalValue({
        generated_css_sha256: generatedCssSha256,
        tailwind_toolchain_lock: value.tailwind_toolchain_lock,
        compiler_identity: value.compiler_identity
      })
    )
  );
  return {
    source_code: value.source_code,
    source_sha256: sourceSha256,
    dependency_lock: dependencyLock,
    generated_css: value.generated_css,
    generated_css_sha256: generatedCssSha256,
    tailwind_toolchain_lock: value.tailwind_toolchain_lock,
    compiler_identity: value.compiler_identity,
    executable_style_identity: executableStyleIdentity,
    shadow_style_asset: createNativeReactExecutableStyleAsset(
      value.generated_css,
      generatedCssSha256
    )
  };
}

function canonicalValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonicalValue);
  if (typeof value !== 'object' || value === null) return value;
  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => [key, canonicalValue(entry)])
  );
}

function isNonEmptyStringRecord(
  value: unknown
): value is Record<string, string> {
  return (
    typeof value === 'object' &&
    value !== null &&
    !Array.isArray(value) &&
    Object.keys(value).length > 0 &&
    Object.values(value).every(
      (entry) => typeof entry === 'string' && entry.trim().length > 0
    )
  );
}

export async function compileNativeReactExecutableStyle(
  sourceCode: string,
  dependencyLock: NativeReactCatalogDependencyLock = []
): Promise<NativeReactExecutableStyleCompilation> {
  return compileLockedNativeReactExecutableStyle({
    sourceCode,
    dependencyLock,
    tailwindToolchainLock: NATIVE_REACT_TAILWIND_TOOLCHAIN_LOCK,
    compilerIdentity: NATIVE_REACT_TAILWIND_COMPILER_IDENTITY
  });
}

export async function compileLockedNativeReactExecutableStyle({
  sourceCode,
  dependencyLock,
  tailwindToolchainLock,
  compilerIdentity
}: {
  sourceCode: string;
  dependencyLock: NativeReactCatalogDependencyLock;
  tailwindToolchainLock: Record<string, string>;
  compilerIdentity: Record<string, string>;
}): Promise<
  NativeReactExecutableStyleCompilation & {
    dependency_lock: NativeReactCatalogDependencyLock;
  }
> {
  const result = await compileTailwindExecutableArtifact({
    source_code: sourceCode,
    dependency_lock: dependencyLock,
    compiler_identity: compilerIdentity,
    toolchain_lock: tailwindToolchainLock
  });
  if (!result.ok) throw new Error(result.error.message);
  if (
    result.source_sha256 !== sha256Text(sourceCode) ||
    result.generated_css_sha256 !== sha256Text(result.generated_css) ||
    JSON.stringify(canonicalValue(result.dependency_lock)) !==
      JSON.stringify(canonicalValue(dependencyLock)) ||
    JSON.stringify(canonicalValue(result.toolchain_lock)) !==
      JSON.stringify(canonicalValue(tailwindToolchainLock)) ||
    JSON.stringify(canonicalValue(result.compiler_identity)) !==
      JSON.stringify(canonicalValue(compilerIdentity))
  ) {
    throw new Error(
      'Executable compiler result does not match its locked input.'
    );
  }
  return {
    dependency_lock: result.dependency_lock,
    generated_css: result.generated_css,
    generated_css_sha256: result.generated_css_sha256,
    tailwind_toolchain_lock: result.toolchain_lock,
    compiler_identity: result.compiler_identity
  };
}

export function createNativeReactExecutableStyleAsset(
  generatedCss: string,
  generatedCssSha256 = sha256Text(generatedCss)
): NativeReactResolvedModuleAsset {
  return {
    module_source: 'frontstage/executable-style',
    role: 'shadow_style',
    media_type: 'text/css',
    sha256: generatedCssSha256,
    url: `frontstage-executable-style:${generatedCssSha256}`,
    bytes: new TextEncoder().encode(generatedCss).buffer
  };
}
