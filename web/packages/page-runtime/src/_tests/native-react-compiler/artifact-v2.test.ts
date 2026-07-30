import { describe, expect, test } from 'vitest';

import {
  NATIVE_REACT_COMPILER_ABI,
  NATIVE_REACT_RUNTIME_ABI,
  canonicalizeNativeReactComponentArtifact,
  compileNativeReactComponent,
  createNativeReactRuntimeFingerprint,
  type NativeReactCatalogDependencyLock,
  type NativeReactComponentArtifact
} from '../../index';

const source =
  "import { Surface } from '@1flowbase/native-components'; export default () => <Surface />;";

describe('Native React Artifact V2 identity', () => {
  test('D2-AC-005 binds source, compiler/runtime ABI and exact dependency lock', () => {
    const first = compile(
      lock(),
      createNativeReactRuntimeFingerprint('/worker-a.js')
    );
    const same = compile(
      lock(),
      createNativeReactRuntimeFingerprint('/worker-a.js')
    );
    const sourceChanged = compile(
      lock(),
      createNativeReactRuntimeFingerprint('/worker-a.js'),
      `${source}\n// changed`
    );
    const versionChanged = compile(
      lock({ module_version: '2.0.0' }),
      createNativeReactRuntimeFingerprint('/worker-a.js')
    );
    const digestChanged = compile(
      lock({
        assets: [browserAsset('b')]
      }),
      createNativeReactRuntimeFingerprint('/worker-a.js')
    );
    const styleDigestChanged = compile(
      lock({
        assets: [browserAsset('a'), styleAsset('c')]
      }),
      createNativeReactRuntimeFingerprint('/worker-a.js')
    );
    const hostVersionChanged = compile(
      [...lock(), hostModule('react', '20.0.0')],
      createNativeReactRuntimeFingerprint('/worker-a.js')
    );
    const runtimeChanged = compile(
      lock(),
      createNativeReactRuntimeFingerprint('/worker-b.js')
    );
    const hostChanged = compile(
      lock(),
      createNativeReactRuntimeFingerprint('/worker-a.js', 'react@20')
    );

    expect(first.version).toBe(2);
    expect(first.identity).toMatchObject({
      compiler_abi: NATIVE_REACT_COMPILER_ABI,
      runtime_abi: NATIVE_REACT_RUNTIME_ABI
    });
    expect(same.identity).toEqual(first.identity);
    expect(sourceChanged.identity.source_sha256).not.toBe(
      first.identity.source_sha256
    );
    expect(versionChanged.identity.dependency_lock_sha256).not.toBe(
      first.identity.dependency_lock_sha256
    );
    expect(digestChanged.identity.dependency_lock_sha256).not.toBe(
      first.identity.dependency_lock_sha256
    );
    expect(styleDigestChanged.identity.dependency_lock_sha256).not.toBe(
      first.identity.dependency_lock_sha256
    );
    expect(hostVersionChanged.identity.dependency_lock_sha256).not.toBe(
      first.identity.dependency_lock_sha256
    );
    expect(runtimeChanged.identity.runtime_fingerprint).not.toBe(
      first.identity.runtime_fingerprint
    );
    expect(hostChanged.identity.runtime_fingerprint).not.toBe(
      first.identity.runtime_fingerprint
    );
  });

  test('D2-AC-006 canonicalizer rejects V1, corrupt identity, old ABI and corrupt program bytes', () => {
    const artifact = compile(
      lock(),
      createNativeReactRuntimeFingerprint('/worker.js')
    );
    const transportedArtifact = structuredClone(artifact);
    expect(
      transportedArtifact.program.importBindings.some(
        (binding) =>
          binding.kind === 'named' &&
          binding.source === '@1flowbase/native-components' &&
          binding.imported === 'Surface'
      )
    ).toBe(true);
    expect(
      canonicalizeNativeReactComponentArtifact(transportedArtifact)
    ).toEqual(artifact);

    expect(
      canonicalizeNativeReactComponentArtifact({ ...artifact, version: 1 })
    ).toBeNull();
    expect(
      canonicalizeNativeReactComponentArtifact({
        ...artifact,
        identity: {
          ...artifact.identity,
          dependency_lock_sha256: '0'.repeat(64)
        }
      })
    ).toBeNull();
    expect(
      canonicalizeNativeReactComponentArtifact({
        ...artifact,
        identity: { ...artifact.identity, runtime_abi: 'old-runtime' }
      })
    ).toBeNull();
    expect(
      canonicalizeNativeReactComponentArtifact({
        ...artifact,
        program: { ...artifact.program, executableBody: 'corrupt' }
      })
    ).toBeNull();
  });
});

function compile(
  dependencyLock: NativeReactCatalogDependencyLock,
  runtimeFingerprint: string,
  currentSource = source
): NativeReactComponentArtifact {
  const result = compileNativeReactComponent(
    currentSource,
    dependencyLock,
    runtimeFingerprint
  );
  if (!result.ok)
    throw new Error('Expected V2 fixture compilation to succeed.');
  return result.artifact;
}

function lock(
  overrides: Partial<NativeReactCatalogDependencyLock[number]> = {}
): NativeReactCatalogDependencyLock {
  return [
    {
      module_source: '@1flowbase/native-components',
      module_version: '1.0.0',
      binding: 'fetched',
      assets: [browserAsset('a')],
      exports: ['Surface'],
      ...overrides
    }
  ];
}

function browserAsset(digestCharacter: string) {
  const sha256 = digestCharacter.repeat(64);
  return {
    role: 'browser_module' as const,
    media_type: 'text/javascript; charset=utf-8',
    sha256,
    url: `/api/console/frontstage/workspace-1/component-module-assets/${sha256}`
  };
}

function styleAsset(digestCharacter: string) {
  const sha256 = digestCharacter.repeat(64);
  return {
    role: 'shadow_style' as const,
    media_type: 'text/css; charset=utf-8',
    sha256,
    url: `/api/console/frontstage/workspace-1/component-module-assets/${sha256}`
  };
}

function hostModule(moduleSource: 'react' | 'antd', moduleVersion: string) {
  return {
    module_source: moduleSource,
    module_version: moduleVersion,
    binding: 'host' as const,
    assets: [],
    exports: ['default']
  };
}
