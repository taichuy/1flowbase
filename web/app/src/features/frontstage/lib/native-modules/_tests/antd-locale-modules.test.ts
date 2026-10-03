import { readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { describe, expect, test } from 'vitest';
import { build } from 'vite';
import { loadAntDesignEsModule } from 'virtual:1flowbase-native-antd-es-modules';

import {
  collectAntDesignEsModuleSources,
  nativeAntDesignEsModulesPlugin
} from '../../../../../../build/native-antd-es-modules';

const require = createRequire(import.meta.url);
const packageRoot = path.dirname(require.resolve('antd/package.json'));

describe('Ant Design public locale modules', () => {
  test('AC-002 builds public locale imports without CommonJS interop failures', async () => {
    const result = await build({
      configFile: false,
      logLevel: 'silent',
      plugins: [
        nativeAntDesignEsModulesPlugin('build'),
        {
          name: 'locale-test-entry',
          resolveId(id) {
            if (id === 'locale-test-entry' || id.endsWith('/locale-test-entry'))
              return '\0locale-test-entry';
          },
          load(id) {
            if (id === '\0locale-test-entry')
              return `
            export { default as en } from 'antd/locale/en_US';
            export { default as zh } from 'antd/locale/zh_CN.js';
          `;
          }
        }
      ],
      build: {
        write: false,
        minify: false,
        lib: {
          entry: 'locale-test-entry',
          formats: ['cjs'],
          fileName: 'locales'
        }
      }
    });
    const output = Array.isArray(result) ? result[0] : result;
    if (!('output' in output)) throw new Error('Expected a completed build');
    const chunk = output.output.find((entry) => entry.type === 'chunk');
    if (!chunk || chunk.type !== 'chunk')
      throw new Error('Missing locale bundle');
    const exports: Record<string, { locale: string }> = {};
    new Function('exports', chunk.code)(exports);
    expect(exports.en.locale).toBe('en');
    expect(exports.zh.locale).toBe('zh-cn');
  });

  test('AC-001 registers installed public locales against canonical ESM modules', () => {
    const inventory = new Map(
      collectAntDesignEsModuleSources().map(
        ({ moduleSource, loaderSource }) => [moduleSource, loaderSource]
      )
    );
    const localeFiles = readdirSync(path.join(packageRoot, 'locale')).filter(
      (file) => /^[A-Za-z0-9_]+\.js$/u.test(file)
    );
    expect(localeFiles).toContain('en_US.js');
    expect(localeFiles).toContain('zh_CN.js');
    for (const file of localeFiles) {
      expect(inventory.get(`antd/locale/${file}`)).toBe(
        `antd/es/locale/${file}`
      );
      expect(inventory.get(`antd/locale/${file.slice(0, -3)}`)).toBe(
        `antd/es/locale/${file}`
      );
    }
    expect(inventory.has('antd/locale/not_installed')).toBe(false);
    expect(inventory.has('antd/not_installed')).toBe(false);
  });

  test('AC-001/002 loads real default exports and shares the ESM locale object', async () => {
    for (const name of ['en_US', 'zh_CN', 'fr_FR']) {
      const [publicModule, explicitModule, esmModule] = await Promise.all([
        loadAntDesignEsModule(`antd/locale/${name}`),
        loadAntDesignEsModule(`antd/locale/${name}.js`),
        loadAntDesignEsModule(`antd/es/locale/${name}`)
      ]);
      expect(publicModule.default).toBe(esmModule.default);
      expect(explicitModule.default).toBe(esmModule.default);
      expect(publicModule.default).toHaveProperty('Pagination');
    }
    await expect(
      loadAntDesignEsModule('antd/locale/not_installed')
    ).rejects.toThrow();
  });
});
