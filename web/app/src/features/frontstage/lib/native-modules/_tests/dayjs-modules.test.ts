import { describe, expect, test } from 'vitest';

import { collectDayjsModuleSources } from '../../../../../../build/native-dayjs-modules';

describe('dayjs native module inventory', () => {
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
});
