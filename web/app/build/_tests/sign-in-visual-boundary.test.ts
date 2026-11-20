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

test('session and entry requests do not wait for the UI runtime through a document cleanup helper', () => {
  expect(() =>
    assertSignInVisualBoundary({
      app: chunk([page, hero]),
      entry: chunk(['/src/bootstrap.ts'], ['ui']),
      ui: chunk(['/node_modules/react/index.js'])
    })
  ).toThrow('react');
});
