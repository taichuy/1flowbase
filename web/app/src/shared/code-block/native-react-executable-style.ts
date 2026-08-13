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
  compileTailwindUtilities,
  extractStaticTailwindCandidates
} from '@1flowbase/tailwindcss-catalog/compiler';

export const NATIVE_REACT_TAILWIND_COMPILER_IDENTITY = Object.freeze({
  name: '@1flowbase/tailwindcss-catalog',
  contract: 'source-driven-utilities-v1',
  tailwind_version: '4.3.3'
});

export const NATIVE_REACT_TAILWIND_TOOLCHAIN_LOCK = Object.freeze({
  package: 'tailwindcss',
  version: '4.3.3',
  mode: 'theme-and-utilities'
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
  sourceCode: string
): Promise<NativeReactExecutableStyleCompilation> {
  const importsTailwind =
    /(?:import|export)\s+(?:[^'";]+?\s+from\s+)?['"]tailwindcss['"]/u.test(
      sourceCode
    );
  const generatedCss = importsTailwind
    ? (
        await compileTailwindUtilities(
          extractStaticTailwindCandidates(sourceCode)
        )
      ).css
    : '';
  return {
    generated_css: generatedCss,
    generated_css_sha256: sha256Text(generatedCss),
    tailwind_toolchain_lock: NATIVE_REACT_TAILWIND_TOOLCHAIN_LOCK,
    compiler_identity: NATIVE_REACT_TAILWIND_COMPILER_IDENTITY
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
