import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, test } from 'vitest';

import { collectDndKitModuleSources } from '../../../../../../build/native-dnd-kit-modules';

describe('@dnd-kit native module inventory', () => {
  test('I1929-AC-002/003 discovers every installed package root and resolvable JavaScript subpath', () => {
    const inventory = collectDndKitModuleSources({
      projectRoot: process.cwd()
    });
    const moduleSources = inventory.map(({ moduleSource }) => moduleSource);

    expect(moduleSources).toEqual(
      expect.arrayContaining([
        '@dnd-kit/core',
        '@dnd-kit/modifiers',
        '@dnd-kit/sortable',
        '@dnd-kit/utilities',
        '@dnd-kit/core/dist/index.js',
        '@dnd-kit/core/dist/index'
      ])
    );
    expect(
      inventory.every(
        ({ moduleSource, packageName, packageVersion }) =>
          moduleSource.startsWith(`${packageName}/`) ||
          (moduleSource === packageName &&
            packageName.startsWith('@dnd-kit/') &&
            packageVersion.length > 0)
      )
    ).toBe(true);
  });

  test('I1929-AC-003 automatically inventories a newly installed @dnd-kit package after rebuild', () => {
    const projectRoot = mkdtempSync(join(tmpdir(), 'flowbase-dnd-kit-'));
    const packageRoot = join(
      projectRoot,
      'node_modules/@dnd-kit/future-package'
    );
    try {
      mkdirSync(join(packageRoot, 'dist'), { recursive: true });
      writeFileSync(
        join(packageRoot, 'package.json'),
        JSON.stringify({ name: '@dnd-kit/future-package', version: '1.2.3' })
      );
      writeFileSync(
        join(packageRoot, 'dist/index.js'),
        'export const ready = true;'
      );

      expect(collectDndKitModuleSources({ projectRoot })).toEqual(
        expect.arrayContaining([
          {
            loaderSource: '@dnd-kit/future-package',
            moduleSource: '@dnd-kit/future-package',
            packageName: '@dnd-kit/future-package',
            packageVersion: '1.2.3'
          },
          {
            loaderSource: '@dnd-kit/future-package/dist/index.js',
            moduleSource: '@dnd-kit/future-package/dist/index.js',
            packageName: '@dnd-kit/future-package',
            packageVersion: '1.2.3'
          }
        ])
      );
    } finally {
      rmSync(projectRoot, { recursive: true, force: true });
    }
  });
});
