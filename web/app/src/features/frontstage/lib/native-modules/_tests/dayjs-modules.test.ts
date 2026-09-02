import { describe, expect, test } from 'vitest';

import {
  collectDayjsModuleSources,
  generateNativeDayjsDevModules,
  resolveDayjsDevModuleSource
} from '../../../../../../build/native-dayjs-modules';

describe('dayjs native module inventory', () => {
  test('I1953-AC-001/003 keeps dev imports independent from the dayjs catalog size', () => {
    const inventory = collectDayjsModuleSources({
      projectRoot: process.cwd()
    });
    const generatedModule = generateNativeDayjsDevModules(inventory);

    expect(generatedModule).toContain('const moduleSourceSet = new Set(');
    expect(generatedModule).toContain('/* @vite-ignore */');
    expect(generatedModule.match(/import\(/gu)).toHaveLength(1);
    expect(generatedModule).not.toContain('const loaders');
    expect(
      inventory.find(({ moduleSource }) => moduleSource === 'dayjs/plugin/utc')
        ?.devLoaderSource
    ).toBe('dayjs/esm/plugin/utc/index.js');
  });

  test('I1933-AC-004a/004b inventories the package root and every resolvable JavaScript subpath', () => {
    const inventory = collectDayjsModuleSources({
      projectRoot: process.cwd()
    });
    const moduleSources = inventory.map(({ moduleSource }) => moduleSource);

    expect(moduleSources).toEqual(
      expect.arrayContaining([
        'dayjs',
        'dayjs/plugin/utc',
        'dayjs/plugin/utc.js',
        'dayjs/locale/zh-cn',
        'dayjs/locale/zh-cn.js',
        'dayjs/esm/plugin/utc',
        'dayjs/esm/plugin/utc/index.js'
      ])
    );
    expect(moduleSources).not.toContain('dayjs/plugin/not-installed');
    expect(
      inventory.every(
        ({ moduleSource, packageName, packageVersion }) =>
          (moduleSource === 'dayjs' || moduleSource.startsWith('dayjs/')) &&
          packageName === 'dayjs' &&
          packageVersion === '1.11.20'
      )
    ).toBe(true);
  });

  test('I1933-AC-004d distinguishes real declaration entrypoints from runtime-only modules', () => {
    const inventory = collectDayjsModuleSources({
      projectRoot: process.cwd()
    });
    const declaredSources = inventory
      .filter(({ hasDeclaration }) => hasDeclaration)
      .map(({ moduleSource }) => moduleSource);

    expect(declaredSources).toEqual(
      expect.arrayContaining([
        'dayjs',
        'dayjs/plugin/utc',
        'dayjs/plugin/utc.js'
      ])
    );
    expect(declaredSources).not.toContain('dayjs/locale/zh-cn');
  });

  test('DV-F06 maps third-party CommonJS plugin imports to the ESM development entry', () => {
    const inventory = collectDayjsModuleSources({
      projectRoot: process.cwd()
    });

    expect(
      resolveDayjsDevModuleSource(inventory, 'dayjs/plugin/advancedFormat.js')
    ).toBe('dayjs/esm/plugin/advancedFormat/index.js');
    expect(
      resolveDayjsDevModuleSource(inventory, 'dayjs/plugin/duration')
    ).toBe('dayjs/esm/plugin/duration/index.js');
    expect(resolveDayjsDevModuleSource(inventory, 'dayjs')).toBe(
      'dayjs/esm/index.js'
    );
    expect(
      resolveDayjsDevModuleSource(inventory, 'dayjs/esm/plugin/utc/index.js')
    ).toBeNull();
  });
});
