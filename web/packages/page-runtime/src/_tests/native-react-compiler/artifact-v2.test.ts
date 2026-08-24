import { describe, expect, test } from 'vitest';

import {
  NATIVE_REACT_COMPILER_ABI,
  NATIVE_REACT_RUNTIME_ABI,
  canonicalizeNativeReactComponentArtifact,
  compileNativeReactComponent,
  type NativeReactComponentArtifact,
  type NativeReactModuleDefinition
} from '../../index';

const source =
  "import { Surface } from '@1flowbase/native-components'; export default () => <Surface />;";

describe('Native React content-addressed artifact identity', () => {
  test('AC-004/006/007 binds only source and compiler/runtime ABI', () => {
    const first = compile(definitions());
    const same = compile(definitions());
    const sourceChanged = compile(definitions(), `${source}\n// changed`);
    const registryExpanded = compile(
      definitions(['Surface', 'ScrollableSurface'])
    );

    expect(first.identity).toEqual({
      source_sha256: expect.stringMatching(/^[a-f0-9]{64}$/),
      compiler_abi: NATIVE_REACT_COMPILER_ABI,
      runtime_abi: NATIVE_REACT_RUNTIME_ABI
    });
    expect(same.identity).toEqual(first.identity);
    expect(registryExpanded.identity).toEqual(first.identity);
    expect(sourceChanged.identity.source_sha256).not.toBe(
      first.identity.source_sha256
    );
    expect(first).not.toHaveProperty('dependencyLock');
    expect(first.identity).not.toHaveProperty('dependency_lock_sha256');
    expect(first.identity).not.toHaveProperty('runtime_fingerprint');
  });

  test('AC-004 canonicalizer rejects old format, old ABI and corrupt program bytes', () => {
    const artifact = compile(definitions());
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
      canonicalizeNativeReactComponentArtifact({ ...artifact, version: 2 })
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
  moduleDefinitions: NativeReactModuleDefinition[],
  currentSource = source
): NativeReactComponentArtifact {
  const result = compileNativeReactComponent(currentSource, moduleDefinitions);
  if (!result.ok) throw new Error('Expected artifact compilation to succeed.');
  return result.artifact;
}

function definitions(
  exports: string[] = ['Surface']
): NativeReactModuleDefinition[] {
  return [
    {
      module_source: 'react/jsx-runtime',
      exports: ['Fragment', 'jsx', 'jsxs']
    },
    {
      module_source: '@1flowbase/native-components',
      exports
    }
  ];
}
