import { describe, expect, test, vi } from 'vitest';

import {
  NativeReactModuleRegistryError,
  compileNativeReactComponent,
  createNativeReactRuntimeFingerprint,
  type NativeReactCompileDiagnostic,
  type NativeReactModuleRegistry,
  type NativeReactResolvedModuleAsset
} from '@1flowbase/page-runtime';

import { prepareNativeReactSource } from '../native-react-source-preparation';
import type { NativeReactBrowserCompileResult } from '../native-react-compiler-browser';

const frozenSource = 'export default function Block() { return null; }';

function compiledArtifact() {
  const compiled = compileNativeReactComponent(
    frozenSource,
    [],
    createNativeReactRuntimeFingerprint('source-preparation-test')
  );
  if (!compiled.ok) throw new Error('Native React test artifact failed.');
  return compiled.artifact;
}

function compilerReturning(
  result: NativeReactBrowserCompileResult
): typeof import(
  '../native-react-compiler-browser'
).compileNativeReactComponentInBrowser {
  return vi.fn(async () => result);
}

function registry(
  overrides: Partial<NativeReactModuleRegistry> = {}
): NativeReactModuleRegistry {
  return {
    load: vi.fn(async () => ({})),
    resolveModuleMap: vi.fn(async () => ({})),
    resolveModuleAssets: vi.fn(async () => []),
    ...overrides
  };
}

describe('Native React source preparation', () => {
  test('R7-AC-001 passes Host evaluation bindings without changing the artifact', async () => {
    const runtimeLog = vi.fn();
    const source =
      "export default function Block() { console.log('prepared'); return null; }";
    const compiled = compileNativeReactComponent(source);
    if (!compiled.ok) throw new Error('Native React test artifact failed.');
    const result = await prepareNativeReactSource({
      frozenSource: source,
      requestId: 'runtime-console',
      dependencyLock: [],
      compiler: compilerReturning({
        ok: true,
        artifact: compiled.artifact,
        diagnostics: []
      }),
      registryFactory: () => registry(),
      evaluationBindings: {
        console: {
          debug: vi.fn(),
          error: vi.fn(),
          info: vi.fn(),
          log: runtimeLog,
          warn: vi.fn()
        }
      }
    });

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    result.component();
    expect(runtimeLog).toHaveBeenCalledWith('prepared');
  });

  test('R6-P1 preserves compile diagnostics without creating a registry', async () => {
    const diagnostic: NativeReactCompileDiagnostic = {
      phase: 'compile',
      code: 'transform_failed',
      path: 'source',
      message: 'fixture compile failure'
    };
    const registryFactory = vi.fn();

    const result = await prepareNativeReactSource({
      frozenSource,
      requestId: 'compile-failure',
      dependencyLock: [],
      compiler: compilerReturning({ ok: false, diagnostics: [diagnostic] }),
      registryFactory
    });

    expect(result).toEqual({ ok: false, diagnostics: [diagnostic] });
    expect(registryFactory).not.toHaveBeenCalled();
  });

  test('R6-P1 returns typed runtime diagnostics from registry evaluation', async () => {
    const moduleRegistry = registry({
      resolveModuleMap: vi.fn(async () => {
        throw new NativeReactModuleRegistryError(
          'module_not_registered',
          'modules.catalog/widget',
          'fixture registry failure'
        );
      })
    });

    const result = await prepareNativeReactSource({
      frozenSource,
      requestId: 'registry-failure',
      dependencyLock: [],
      compiler: compilerReturning({
        ok: true,
        artifact: compiledArtifact(),
        diagnostics: []
      }),
      registryFactory: () => moduleRegistry
    });

    expect(result).toEqual({
      ok: false,
      diagnostics: [
        {
          phase: 'runtime',
          code: 'runtime_error',
          path: 'modules.catalog/widget',
          message: 'fixture registry failure'
        }
      ]
    });
  });

  test('R6-P1 evaluates the component and resolves its module assets', async () => {
    const artifact = compiledArtifact();
    const moduleAsset: NativeReactResolvedModuleAsset = {
      module_source: 'catalog/widget',
      role: 'shadow_style',
      media_type: 'text/css',
      sha256: 'a'.repeat(64),
      url: 'https://assets.example/widget.css',
      bytes: new ArrayBuffer(4)
    };
    const moduleRegistry = registry({
      resolveModuleAssets: vi.fn(async () => [moduleAsset])
    });
    const registryFactory = vi.fn(() => moduleRegistry);

    const result = await prepareNativeReactSource({
      frozenSource,
      requestId: 'success',
      dependencyLock: [],
      compiler: compilerReturning({ ok: true, artifact, diagnostics: [] }),
      registryFactory
    });

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.component).toBeTypeOf('function');
    expect(result.moduleAssets).toEqual([moduleAsset]);
    expect(registryFactory).toHaveBeenCalledWith(artifact.dependencyLock);
    expect(moduleRegistry.resolveModuleAssets).toHaveBeenCalledWith(
      artifact.program.injectedModules.map(({ source }) => source)
    );
  });

  test('R6-P1 converts module asset failures to typed runtime diagnostics', async () => {
    const moduleRegistry = registry({
      resolveModuleAssets: vi.fn(async () => {
        throw new NativeReactModuleRegistryError(
          'module_fetch_failed',
          'modules.catalog/widget.assets.shadow_style',
          'fixture asset failure'
        );
      })
    });

    const result = await prepareNativeReactSource({
      frozenSource,
      requestId: 'asset-failure',
      dependencyLock: [],
      compiler: compilerReturning({
        ok: true,
        artifact: compiledArtifact(),
        diagnostics: []
      }),
      registryFactory: () => moduleRegistry
    });

    expect(result).toEqual({
      ok: false,
      diagnostics: [
        {
          phase: 'runtime',
          code: 'runtime_error',
          path: 'modules.catalog/widget.assets.shadow_style',
          message: 'fixture asset failure'
        }
      ]
    });
  });
});
