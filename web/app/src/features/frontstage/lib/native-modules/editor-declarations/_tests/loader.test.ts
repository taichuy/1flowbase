import { describe, expect, test, vi } from 'vitest';
import { createNativeDeclarationLoader } from '../loader';
import { declarationImportSources } from '../imports';

const file = (source: string, content = source) => ({
  source,
  content,
  filePath: `file:///node_modules/${source}/index.d.ts`
});

describe('source declaration demand', () => {
  test('reads multiline/re-export/type imports and skips comments and unrelated strings', () => {
    expect(
      declarationImportSources(`
// import X from 'comment';
const text = "import X from 'string'";
import type {\n A\n} from 'alpha';
export { B } from /* bridge */ 'beta';
import 'side-effect';
type C = import('gamma').C;
import legacy = require('legacy');
/// <reference types="ambient" />
`)
    ).toEqual(['alpha', 'beta', 'side-effect', 'gamma', 'legacy', 'ambient']);
  });

  test('loads shared cross-package resources, synthetic facade dependencies and aliases, never unrelated roots', async () => {
    const fetch = vi.fn(async (url: string) => [file(url)]);
    const loader = createNativeDeclarationLoader(
      {
        version: 1,
        roots: {
          react: ['r'],
          'antd/es/button': ['button', 'shared'],
          antd: ['all', 'shared']
        },
        resources: {
          r: '/r',
          button: '/button',
          shared: '/shared',
          all: '/all'
        }
      },
      [file('facade', `export * from 'antd/es/button';`)],
      fetch
    );
    const libs = await loader.loadForSource(`import { Button } from 'facade'`);
    expect(fetch.mock.calls.map(([url]) => url).sort()).toEqual([
      '/button',
      '/r',
      '/shared'
    ]);
    expect(libs.some((lib) => lib.source === 'facade')).toBe(true);
    await loader.loadForSource(`import { Button } from 'antd/es/button'`);
    expect(fetch).toHaveBeenCalledTimes(3);
    await loader.loadAll();
    expect(fetch.mock.calls.map(([url]) => url).sort()).toEqual([
      '/all',
      '/button',
      '/r',
      '/shared'
    ]);
  });

  test('immediate source demand is independent of an unrelated full-completion flight', async () => {
    let finishBackground!: (files: ReturnType<typeof file>[]) => void;
    const fetch = vi.fn((url: string) =>
      url === '/background'
        ? new Promise<ReturnType<typeof file>[]>((resolve) => {
            finishBackground = resolve;
          })
        : Promise.resolve([file('current')])
    );
    const loader = createNativeDeclarationLoader(
      {
        version: 1,
        roots: { current: ['current'], background: ['background'] },
        resources: { current: '/current', background: '/background' }
      },
      [],
      fetch
    );
    const all = loader.loadAll();
    expect(await loader.loadForSource(`import 'current'`)).toEqual([
      file('current')
    ]);
    expect(fetch).toHaveBeenCalledTimes(2);
    finishBackground([file('background')]);
    expect(await all).toHaveLength(2);
  });

  test('rejects errors and retries only failed hash resources, retaining successful cache', async () => {
    const fetch = vi
      .fn()
      .mockResolvedValueOnce([file('react')])
      .mockRejectedValueOnce(new Error('offline'))
      .mockResolvedValue([file('alpha')]);
    const loader = createNativeDeclarationLoader(
      {
        version: 1,
        roots: { react: ['r'], alpha: ['a', 'r'] },
        resources: { r: '/r', a: '/a' }
      },
      [],
      fetch
    );
    await loader.loadForSource('');
    await expect(loader.loadForSource(`import 'alpha'`)).rejects.toThrow(
      'offline'
    );
    expect(await loader.loadForSource(`import 'alpha'`)).toHaveLength(2);
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(['/r', '/a', '/a']);
  });
});

test('unknown source names cannot accidentally address inherited index properties', async () => {
  const fetch = vi.fn(async () => []);
  const loader = createNativeDeclarationLoader(
    { version: 1, roots: {}, resources: {} },
    [],
    fetch
  );
  expect(
    await loader.loadForSource("import 'constructor'; import 'toString';")
  ).toEqual([]);
  expect(fetch).not.toHaveBeenCalled();
});
