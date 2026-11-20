import { describe, expect, test } from 'vitest';
import { collectScenarioDemandManifest, staticChunkClosure } from '../scenario-demand/manifest';

function chunk(fileName: string, owner: string, imports: string[] = [], dynamicImports: string[] = []) {
  return { type: 'chunk' as const, fileName, name: fileName, modules: { [owner]: {} }, imports, dynamicImports };
}
const page = '/src/features/frontstage/pages/FrontStagePage.tsx';
const studio = '/src/features/frontstage/components/jsx-studio/FrontstageJsxStudioDrawer.tsx';

describe('compiler scenario demand graph', () => {
  test('records every chunk, CSS and asset without crossing lazy editing or runtime Worker edges', () => {
    const manifest = collectScenarioDemandManifest({
      'page.js': { ...chunk('page.js', page, ['runtime.js'], ['studio.js']),
        viteMetadata: { importedCss: new Set(['page.css']), importedAssets: new Set(['runtime-worker.js']) } },
      'runtime.js': chunk('runtime.js', '/runtime/compiler.ts'),
      'studio.js': chunk('studio.js', studio),
      'runtime-worker.js': { type: 'asset', fileName: 'runtime-worker.js' },
      'declarations-hash.json': { type: 'asset', fileName: 'declarations-hash.json' }
    });
    expect(manifest.scenes.browse.staticClosure).toEqual(['page.js', 'runtime.js']);
    expect(manifest.chunks).toHaveLength(3);
    expect(manifest.chunks.find((c) => c.file === 'page.js')?.css).toEqual(['page.css']);
    expect(manifest.assets).toContain('declarations-hash.json');
  });
  test('rejects reintroducing Studio through a shared vendor on the browse path', () => {
    expect(() => collectScenarioDemandManifest({
      'page.js': chunk('page.js', page, ['vendor.js']),
      'vendor.js': chunk('vendor.js', studio)
    })).toThrow(/browse eagerly includes authoring/);
  });
  test('rejects declarations merged into an otherwise ordinary shared chunk', () => {
    expect(() => collectScenarioDemandManifest({
      'page.js': chunk('page.js', page, ['shared.js']),
      'shared.js': chunk('shared.js', '\0virtual:1flowbase-native-module-declarations')
    })).toThrow(/authoring module/);
  });
  test('static closure terminates on cycles and ignores external modules', () => {
    expect(staticChunkClosure(new Map([
      ['a', { imports: ['b'] }], ['b', { imports: ['a', 'external'] }]
    ]), ['a'])).toEqual(['a', 'b']);
  });
});
