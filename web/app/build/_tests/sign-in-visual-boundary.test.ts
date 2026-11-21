import { expect, test } from 'vitest';
import { assertSignInVisualBoundary } from '../sign-in-visual-boundary';

const page = '/src/features/auth/pages/SignInPage.tsx';
const hero = '/src/features/auth/components/HeroAnimation.tsx';
const form = '/src/features/auth/components/PublicAuthBlock.tsx';
const mcp = '/src/features/auth/pages/mcp-authorization/McpAuthorizePage.tsx';
const chunk = (
  modules: string[],
  imports: string[] = [],
  dynamicImports: string[] = []
) => ({
  type: 'chunk' as const,
  modules: Object.fromEntries(modules.map((id) => [id, {}])),
  imports,
  dynamicImports
});

test('traces only static edges, even across a shared dependency cycle', () => {
  expect(() =>
    assertSignInVisualBoundary({
      a: chunk([page], ['b'], ['c']),
      b: chunk([hero], ['a']),
      c: chunk([form, mcp])
    })
  ).not.toThrow();
});
test.each([form, mcp])(
  'rejects an indirect eager dependency on %s',
  (forbidden) => {
    expect(() =>
      assertSignInVisualBoundary({
        a: chunk([page, hero], ['b']),
        b: chunk([], ['c']),
        c: chunk([forbidden])
      })
    ).toThrow(forbidden.split('/').pop());
  }
);
test('a lazy hero or empty layout is not an interactive sign-in screen', () => {
  expect(() =>
    assertSignInVisualBoundary({
      a: chunk([page], [], ['b']),
      b: chunk([hero])
    })
  ).toThrow('HeroAnimation');
});

test('rejects a form pulled in by another HTML preload root, not just the sign-in page', () => {
  expect(() =>
    assertSignInVisualBoundary({
      page: chunk([page, hero]),
      boot: chunk(['/src/main.tsx'], ['shared']),
      shared: chunk([form])
    })
  ).toThrow('PublicAuthBlock');
});

test('the actual ShadowRoot style provider cannot drag a table onto the login form path', () => {
  expect(() =>
    assertSignInVisualBoundary({
      app: chunk([page, hero]),
      style: chunk(['/node_modules/antd-style/es/index.js'], ['helpers']),
      helpers: chunk([], ['data']),
      data: chunk(['/node_modules/antd/es/table/index.js'])
    })
  ).toThrow('table');
});

test('document entry cannot leak a heavy query framework into the anonymous closure', () => {
  expect(() =>
    assertSignInVisualBoundary({
      app: chunk([page, hero]),
      entry: chunk(['/src/bootstrap.ts'], ['ui']),
      ui: chunk(['/node_modules/@tanstack/react-query/index.js'])
    })
  ).toThrow('react-query');
});

function budgetFixture({
  code = 'export default 1;',
  extra = 0,
  preload = true,
  htmlPadding = ''
} = {}) {
  const dependencies = Array.from(
    { length: extra },
    (_, i) => `assets/part-${i}.js`
  );
  const files = ['assets/app.js', ...dependencies];
  const html = `<html><head>${preload ? files.map((file) => `<link rel="modulepreload" href="/${file}">`).join('') : ''}</head><body>${htmlPadding}</body></html>`;
  return {
    'sign-in-preload.html': { type: 'asset' as const, source: html },
    'assets/app.js': { ...chunk([page, hero], dependencies), code },
    ...Object.fromEntries(
      dependencies.map((file) => [
        file,
        { ...chunk([]), code: 'export default 2;' }
      ])
    )
  };
}

// Stable high-entropy content exercises gzip limits independently of raw limits.
function noise(length: number) {
  let state = 1729;
  return Buffer.from(
    Array.from({ length }, () => {
      state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
      return state >>> 24;
    })
  ).toString('base64');
}

test('reports actual bytes with React included, and honors base-relative HTML URLs', async () => {
  const { assertSignInBudget } = await import('../sign-in-visual-boundary');
  const bundle = budgetFixture({ extra: 1 });
  bundle['assets/part-0.js'] = {
    ...chunk(['/node_modules/react-dom/client.js']),
    code: 'export const react = 1;'
  };
  bundle['sign-in-preload.html'].source = bundle[
    'sign-in-preload.html'
  ].source.replaceAll('href="/', 'href="/nested/');
  const budget = assertSignInBudget(bundle)!;
  expect(budget.files).toHaveLength(2);
  expect(budget.totals.raw).toBe(
    Buffer.byteLength('export default 1;export const react = 1;')
  );
  expect(budget.discoveryDepth).toBe(1);
  expect(budget.files.every((file) => file.gzip > 0 && file.br > 0)).toBe(true);
});

test.each([
  ['JS/CSS count', { extra: 4 }],
  ['JS/CSS raw bytes', { code: 'x'.repeat(240_001) }],
  ['JS/CSS gzip bytes', { code: noise(145_000) }],
  ['HTML raw bytes', { htmlPadding: 'x'.repeat(20_001) }],
  ['HTML gzip bytes', { htmlPadding: noise(8_000) }],
  ['discovery depth', { preload: false }]
])('rejects a real %s budget violation', async (failure, options) => {
  const { assertSignInBudget } = await import('../sign-in-visual-boundary');
  expect(() => assertSignInBudget(budgetFixture(options))).toThrow(failure);
});

test.each([
  'antd/es/alert/index.js',
  '@ant-design/cssinjs/es/index.js',
  '@tanstack/react-router/dist/index.js',
  '@tanstack/react-query/dist/index.js',
  'i18next/dist/index.js',
  'react-i18next/dist/index.js',
  'antd-style/es/index.js'
])('rejects forbidden anonymous framework %s', (module) => {
  expect(() =>
    assertSignInVisualBoundary({
      app: chunk([page, hero], ['heavy']),
      heavy: chunk(['/node_modules/' + module])
    })
  ).toThrow(module);
});
