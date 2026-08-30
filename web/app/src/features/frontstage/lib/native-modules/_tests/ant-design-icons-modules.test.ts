import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { describe, expect, test } from 'vitest';

import {
  NATIVE_ANT_DESIGN_ICONS_LOADERS_VIRTUAL_ID,
  collectAntDesignIconModuleSources,
  generateNativeAntDesignIconsLoadersModule,
  generateNativeAntDesignIconsModule
} from '../../../../../../build/native-ant-design-icons-modules';

describe('@ant-design/icons native module inventory', () => {
  test('I1945-AC-004 keeps root namespace icon leaves lazy', () => {
    const inventory = collectAntDesignIconModuleSources({
      projectRoot: process.cwd()
    });
    const generatedModule = generateNativeAntDesignIconsLoadersModule(inventory);

    expect(generatedModule).toContain('Object.keys(leafLoaders)');
    expect(generatedModule).toContain(
      'lazy(() => loadLeafModule(moduleSource))'
    );
    expect(generatedModule).not.toContain('(await load()).default');
  });

  test('I1949-AC-001 keeps the icon leaf loader table outside the registry entry module', () => {
    const inventory = collectAntDesignIconModuleSources({
      projectRoot: process.cwd()
    });
    const generatedModule = generateNativeAntDesignIconsModule(inventory);

    expect(generatedModule).not.toContain('const leafLoaders');
    expect(generatedModule).not.toContain(
      "import { lazy } from 'react'"
    );
    expect(generatedModule).toContain(
      `import(${JSON.stringify(NATIVE_ANT_DESIGN_ICONS_LOADERS_VIRTUAL_ID)})`
    );
  });

  test('I1949-AC-003 shares the loader-domain flight and clears rejected initialization', () => {
    const inventory = collectAntDesignIconModuleSources({
      projectRoot: process.cwd()
    });
    const generatedModule = generateNativeAntDesignIconsModule(inventory);

    expect(generatedModule).toContain('loaderDomainPromise ??=');
    expect(generatedModule).toContain('loaderDomainPromise = undefined');
  });

  test('I1950-AC-003 coalesces icon leaf module flights and evicts rejected flights', () => {
    const inventory = collectAntDesignIconModuleSources({
      projectRoot: process.cwd()
    });
    const generatedModule =
      generateNativeAntDesignIconsLoadersModule(inventory);

    expect(generatedModule).toContain('const leafModuleFlights = new Map()');
    expect(generatedModule).toContain('leafModuleFlights.get(moduleSource)');
    expect(generatedModule).toContain(
      'leafModuleFlights.set(moduleSource, flight)'
    );
    expect(generatedModule).toContain('leafModuleFlights.delete(moduleSource)');
    expect(generatedModule).toContain(
      'lazy(() => loadLeafModule(moduleSource))'
    );
  });

  test('I1945-AC-002/004 inventories every installed public icon leaf and excludes internal aliases', () => {
    const inventory = collectAntDesignIconModuleSources({
      projectRoot: process.cwd()
    });
    const moduleSources = inventory.modules.map(
      ({ moduleSource }) => moduleSource
    );

    expect(moduleSources).toEqual(
      expect.arrayContaining([
        '@ant-design/icons/ClockCircleOutlined',
        '@ant-design/icons/HomeOutlined'
      ])
    );
    expect(moduleSources).not.toContain('@ant-design/icons/es/icons');
    expect(moduleSources).not.toContain(
      '@ant-design/icons/lib/icons/ClockCircleOutlined'
    );
    expect(inventory.rootExports).toEqual(
      expect.arrayContaining([
        'ClockCircleOutlined',
        'HomeOutlined',
        'createFromIconfontCN',
        'default'
      ])
    );
  });

  test('I1945-AC-002 discovers newly installed public icon leaves after rebuild', () => {
    const projectRoot = mkdtempSync(join(tmpdir(), 'flowbase-icons-'));
    const packageRoot = join(
      projectRoot,
      'node_modules/@ant-design/icons'
    );
    try {
      mkdirSync(join(packageRoot, 'es/icons'), { recursive: true });
      mkdirSync(join(packageRoot, 'lib/icons'), { recursive: true });
      writeFileSync(
        join(packageRoot, 'package.json'),
        JSON.stringify({
          name: '@ant-design/icons',
          version: '9.9.9',
          exports: {
            './*': {
              types: './lib/icons/*.d.ts',
              import: './es/icons/*.js'
            }
          }
        })
      );
      writeFileSync(
        join(packageRoot, 'es/icons/FutureOutlined.js'),
        'export default function FutureOutlined() {}'
      );
      writeFileSync(
        join(packageRoot, 'lib/icons/FutureOutlined.d.ts'),
        'declare const FutureOutlined: unknown; export default FutureOutlined;'
      );

      expect(collectAntDesignIconModuleSources({ projectRoot })).toEqual({
        modules: [
          {
            loaderSource: '@ant-design/icons/FutureOutlined',
            moduleSource: '@ant-design/icons/FutureOutlined'
          }
        ],
        packageName: '@ant-design/icons',
        packageVersion: '9.9.9',
        rootExports: expect.arrayContaining([
          'FutureOutlined',
          'createFromIconfontCN',
          'default'
        ])
      });
    } finally {
      rmSync(projectRoot, { recursive: true, force: true });
    }
  });
});
