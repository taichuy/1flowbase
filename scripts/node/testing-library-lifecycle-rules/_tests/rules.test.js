const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { createRequire } = require('node:module');
const fromWeb = createRequire(path.resolve(__dirname, '../../../../web/package.json'));
const { Linter } = fromWeb('eslint');
const { parser } = fromWeb('typescript-eslint');
const testingLibrary = fromWeb('eslint-plugin-testing-library');
const reactRefresh = fromWeb('eslint-plugin-react-refresh');
const reactHooks = fromWeb('eslint-plugin-react-hooks');
const { refineTestingLibraryPlugin, refineReactRefreshPlugin, refineReactHooksPlugin } = require('../index.js');

const plugins = {
  'testing-library': refineTestingLibraryPlugin(testingLibrary),
  'react-refresh': refineReactRefreshPlugin(reactRefresh),
  'react-hooks': refineReactHooksPlugin(reactHooks)
};
const filename = 'web/app/src/shared/schema-ui/v1/_tests/controlled-fixture.tsx';
function messages(code, rule, file = filename, original = false) {
  const selected = original ? {
    'testing-library': testingLibrary, 'react-refresh': reactRefresh, 'react-hooks': reactHooks
  } : plugins;
  return new Linter().verify(code, [{
    files: ['**/*.tsx'],
    languageOptions: { parser, ecmaVersion: 2022, sourceType: 'module' },
    plugins: selected,
    rules: { [rule]: rule === 'react-refresh/only-export-components' ? ['warn', { allowConstantExport: true }] : 'error' }
  }], { filename: file });
}
function accepted(code, rule, file = filename) {
  assert.equal(messages(code, rule, file, true).length > 0, true, 'counterexample must reproduce an upstream diagnostic');
  assert.deepEqual(messages(code, rule, file), []);
}
function retained(code, rule, file = filename) {
  const baseline = messages(code, rule, file, true);
  assert.equal(baseline.length > 0, true, 'negative fixture must trigger the upstream rule');
  assert.deepEqual(messages(code, rule, file).map(({ ruleId, message, messageId, severity }) => ({ ruleId, message, messageId, severity })),
    baseline.map(({ ruleId, message, messageId, severity }) => ({ ruleId, message, messageId, severity })));
}

const actRule = 'testing-library/no-unnecessary-act';
const rtl = "import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';\n";
for (const [name, body] of [
  ['direct event', 'await act(async () => { fireEvent.click(button); });'],
  ['render assignment', 'let view; await act(async () => { view = render(null); });'],
  ['implicit return', 'await act(async () => fireEvent.change(input));']
]) test(`awaited async act accepts ${name} with a real synchronous RTL interaction`, () => accepted(rtl + body, actRule));

test('act keeps real bindings through import aliases and namespace members', () => {
  accepted("import { act as scope, fireEvent as events } from '@testing-library/react'; await scope(async () => { events.click(button); });", actRule);
  accepted("import * as rtl from '@testing-library/react'; await rtl.act(async () => { rtl.render(null); });", actRule);
});
for (const [name, body] of [
  ['sync callback', 'act(() => { fireEvent.click(button); });'],
  ['unawaited async callback', 'act(async () => { fireEvent.click(button); });'],
  ['empty callback', 'await act(async () => {});'],
  ['query only', "await act(async () => { screen.getByRole('button'); });"],
  ['already wrapped waitFor', 'await act(async () => { await waitFor(() => {}); });'],
  ['mixed render and waitFor', 'await act(async () => { render(null); await waitFor(() => {}); });'],
  ['mixed event and async query', "await act(async () => { fireEvent.click(button); await screen.findByRole('button'); });"]
]) test(`act preserves upstream diagnostic for ${name}`, () => retained(rtl + body, actRule));

test('React act imports remain visible rather than being renamed around the rule', () => accepted("import { act } from 'react'; import { fireEvent } from '@testing-library/react'; await act(async () => { fireEvent.click(button); });", actRule));

test('a locally shadowed fireEvent is not proof of an RTL interaction', () => retained(rtl + 'async function example(fireEvent) { await act(async () => { fireEvent.click(button); }); }', actRule));
test('an async util alias mixed with render remains a violation', () => retained("import { act, render, waitFor as poll } from '@testing-library/react'; await act(async () => { render(null); await poll(() => {}); });", actRule));
test('a user-event instance mixed with render remains a violation', () => retained(rtl + "import userEvent from '@testing-library/user-event'; const user = userEvent.setup(); await act(async () => { render(null); await user.click(button); });", actRule));

const cleanupRule = 'testing-library/no-manual-cleanup';
const cleanupImports = "import { act, cleanup } from '@testing-library/react'; import { afterEach } from 'vitest'; import { resetAuthStore } from '../../../../state/auth-store';\n";
test('cleanup may precede the subscribed auth reset in the same hook', () => accepted(cleanupImports + 'afterEach(() => { cleanup(); resetAuthStore(); });', cleanupRule));
test('cleanup may precede an awaited async-act reset in finally', () => accepted(cleanupImports + 'afterEach(async () => { try { await requests(); } finally { cleanup(); await act(async () => { resetAuthStore(); }); } });', cleanupRule));
test('cleanup may be registered through a named lifecycle-only owner', () => accepted(cleanupImports + 'afterEach(release); async function release() { cleanup(); await act(async () => { resetAuthStore(); }); }', cleanupRule));
test('cleanup/reset aliases keep import provenance', () => accepted("import { cleanup as release } from '@testing-library/react'; import { afterEach as after } from 'vitest'; import { resetAuthStore as reset } from '../../../../state/auth-store'; after(() => { release(); reset(); });", cleanupRule));
test('cleanup releases queries before a real QueryClient.clear', () => accepted("import { cleanup } from '@testing-library/react'; import { afterEach } from 'vitest'; import { QueryClient } from '@tanstack/react-query'; const client = new QueryClient(); afterEach(() => { cleanup(); client.clear(); });", cleanupRule));
test('cleanup releases typed query-client inventories before clearing them', () => accepted("import { cleanup } from '@testing-library/react'; import { afterEach } from 'vitest'; import { QueryClient } from '@tanstack/react-query'; const clients = new Set<QueryClient>(); afterEach(() => { cleanup(); for (const client of clients) client.clear(); });", cleanupRule));
test('cleanup releases language subscribers before app i18n reset', () => accepted("import { cleanup } from '@testing-library/react'; import { afterEach } from 'vitest'; import { appI18n as language } from '../../../../shared/i18n/app-i18n'; afterEach(async () => { cleanup(); await language.changeLanguage('en_US'); });", cleanupRule));
for (const [name, body] of [
  ['standalone cleanup', 'afterEach(() => { cleanup(); });'],
  ['reset before cleanup', 'afterEach(() => { resetAuthStore(); cleanup(); });'],
  ['unknown reset name', 'afterEach(() => { cleanup(); resetSomething(); });'],
  ['mock restoration only', 'afterEach(() => { cleanup(); vi.restoreAllMocks(); });'],
  ['conditional reset', 'afterEach(() => { cleanup(); if (flag) resetAuthStore(); });'],
  ['unawaited reset wrapper', 'afterEach(() => { cleanup(); act(async () => { resetAuthStore(); }); });'],
  ['multiple use with one redundant call', 'afterEach(() => { cleanup(); resetAuthStore(); }); cleanup();'],
  ['cleanup outside the hook', 'cleanup(); afterEach(() => { resetAuthStore(); });'],
  ['separately registered reset', 'afterEach(() => { cleanup(); }); afterEach(() => { resetAuthStore(); });'],
  ['nested deferred cleanup', 'afterEach(() => { setTimeout(() => { cleanup(); resetAuthStore(); }); });'],
  ['named helper used outside teardown', 'afterEach(release); function release() { cleanup(); resetAuthStore(); } release();']
]) test(`cleanup preserves upstream diagnostic for ${name}`, () => retained(cleanupImports + body, cleanupRule));
test('a lookalike clear method is not a query reset', () => retained("import { cleanup } from '@testing-library/react'; import { afterEach } from 'vitest'; const client = { clear() {} }; afterEach(() => { cleanup(); client.clear(); });", cleanupRule));
test('a lookalike imported reset is not the auth owner', () => retained("import { cleanup } from '@testing-library/react'; import { afterEach } from 'vitest'; import { resetAuthStore } from './unrelated'; afterEach(() => { cleanup(); resetAuthStore(); });", cleanupRule));

const renderRule = 'testing-library/render-result-naming-convention';
test('renderer registries are not RTL render results', () => accepted("import { render } from '@testing-library/react'; import { createRendererRegistry } from '../registry/create-renderer-registry'; const registry = createRendererRegistry({ fields: {}, views: {}, dynamicForms: {}, shells: {} });", renderRule));
test('aliased known registry factories remain registry values', () => accepted("import { render } from '@testing-library/react'; import { createRendererRegistry as makeRenderRegistry } from '../registry/create-renderer-registry'; const registry = makeRenderRegistry({});", renderRule));
test('real render results still require the original naming convention', () => retained("import { render } from '@testing-library/react'; const registry = render(null);", renderRule));
test('an unrelated render factory does not gain a name exemption', () => retained("import { render } from '@testing-library/react'; import { createRendererRegistry } from './unrelated'; const registry = createRendererRegistry({});", renderRule));
test('local wrappers of render still retain the naming diagnostic', () => retained("import { render } from '@testing-library/react'; function renderPage() { return render(null); } const registry = renderPage();", renderRule));

const refreshRule = 'react-refresh/only-export-components';
const mixed = 'export function Widget() { return null; } export function fixtureSetup() { return {}; }';
test('an explicit unit harness is not a Vite HMR entry', () => accepted("import { vi } from 'vitest'; " + mixed, refreshRule));
test('a direct RTL unit harness can export setup and components', () => accepted("import { render } from '@testing-library/react'; " + mixed, refreshRule));
test('a product module retains HMR export diagnostics', () => retained(mixed, refreshRule, 'web/app/src/features/example/Mixed.tsx'));
test('a test-directory module without direct harness provenance retains diagnostics', () => retained(mixed, refreshRule));
test('browser fixtures retain HMR diagnostics even with a test-library import', () => retained("import { vi } from 'vitest'; " + mixed, refreshRule, 'web/app/src/features/example/_tests/browser-fixtures/entry.tsx'));
test('type-only test imports are not a unit harness', () => retained("import type { Mock } from 'vitest'; " + mixed, refreshRule));

const depsRule = 'react-hooks/exhaustive-deps';
test('effect-only non-reactive EffectEvents do not belong in dependency arrays', () => accepted("import { useEffect, useEffectEvent } from 'react'; function Widget({ value }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, []); }", depsRule));
test('all missing names must be effect-only React EffectEvent bindings', () => accepted("import { useEffect, useEffectEvent } from 'react'; function Widget({ value }) { const first = useEffectEvent(() => value); const second = useEffectEvent(() => value); useEffect(() => { first(); second(); }, []); }", depsRule));

test('React namespace EffectEvents may run from an effect-owned timer', () => accepted("import * as React from 'react'; function Widget({ value }) { const refresh = React.useEffectEvent(() => value); React.useEffect(() => { const timer = setTimeout(() => refresh(), 100); return () => clearTimeout(timer); }, []); }", depsRule));
test('effect events can coexist with intentionally declared effect triggers', () => accepted("import { useEffect, useEffectEvent } from 'react'; function Widget({ value, topology }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, [topology]); }", depsRule));
for (const [name, code] of [
  ['ordinary missing callback', "import { useEffect } from 'react'; function Widget({ refresh }) { useEffect(() => { refresh(); }, []); }"],
  ['mixed missing event and ordinary callback', "import { useEffect, useEffectEvent } from 'react'; function Widget({ value, ordinary }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); ordinary(); }, []); }"],
  ['event used in a UI handler', "import { useEffect, useEffectEvent } from 'react'; function Widget({ value }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, []); return <button onClick={() => refresh()}>Refresh</button>; }"],
  ['event passed outside an effect', "import { useEffect, useEffectEvent } from 'react'; function Widget({ value }) { const refresh = useEffectEvent(() => value); consume(refresh); useEffect(() => { refresh(); }, []); }"],
  ['fake local useEffectEvent', "import { useEffect } from 'react'; function useEffectEvent(callback) { return callback; } function Widget({ value }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, []); }"],
  ['hidden unnecessary outer dependency', "import { useEffect, useEffectEvent } from 'react'; const outer = 1; function Widget({ value }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, [outer]); }"],
  ['duplicate declared dependency', "import { useEffect, useEffectEvent } from 'react'; function Widget({ value, topology }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, [topology, topology]); }"]
]) test(`EffectEvent refinement preserves ${name}`, () => retained(code, depsRule));

test('refinements retain every original rule identity and untouched rule object', () => {
  for (const [base, refined, changed] of [
    [testingLibrary, plugins['testing-library'], ['no-unnecessary-act', 'no-manual-cleanup', 'render-result-naming-convention']],
    [reactRefresh, plugins['react-refresh'], ['only-export-components']],
    [reactHooks, plugins['react-hooks'], ['exhaustive-deps']]
  ]) {
    assert.deepEqual(Object.keys(refined.rules), Object.keys(base.rules));
    for (const name of Object.keys(base.rules)) {
      assert.equal(refined.rules[name].meta, base.rules[name].meta);
      if (!changed.includes(name)) assert.equal(refined.rules[name], base.rules[name]);
    }
  }
});

// The installed 5.2.0 rule's message/suggestion shape is a checked contract,
// not an excuse to swallow unfamiliar reports from a future upstream version.
for (const [name, replacements, message, rangeFixer] of [
  ['missing suggestion', [], null],
  ['multiple suggestions', ['[topology, refresh]', '[refresh]'], null],
  ['ordinary extra dependency', ['[topology, refresh, ordinary]'], null],
  ['removed trigger dependency', ['[refresh]'], null],
  ['non-dependency syntax', ['[topology, refresh()]'], null],
  ['array hole in unknown suggestion', ['[topology,,refresh]'], null],
  ['changed upstream message', ['[topology, refresh]'], 'Upstream changed the dependency diagnostic contract'],
  ['unrecognized fixer API', ['[topology, refresh]'], null, true]
]) test(`unknown EffectEvent suggestion contract retains report: ${name}`, () => {
  const originalRule = reactHooks.rules['exhaustive-deps'];
  const synthetic = {
    ...reactHooks,
    rules: {
      ...reactHooks.rules,
      'exhaustive-deps': {
        ...originalRule,
        create(context) {
          return { CallExpression(node) {
            if (node.callee.type !== 'Identifier' || node.callee.name !== 'useEffect') return;
            context.report({
              node: node.arguments[1],
              message: message ?? "React Hook useEffect has a missing dependency: 'refresh'. Either include it or remove the dependency array.",
              suggest: replacements.map((replacement, index) => ({ desc: `Simulated upstream suggestion ${index}`, fix: (fixer) => rangeFixer ? fixer.replaceTextRange(node.arguments[1].range, replacement) : fixer.replaceText(node.arguments[1], replacement) }))
            });
          } };
        }
      }
    }
  };
  const code = "import { useEffect, useEffectEvent } from 'react'; function Widget({ value, topology }) { const refresh = useEffectEvent(() => value); useEffect(() => { refresh(); }, [topology]); }";
  const result = new Linter().verify(code, [{
    files: ['**/*.tsx'], languageOptions: { parser, ecmaVersion: 2022, sourceType: 'module' },
    plugins: { 'react-hooks': refineReactHooksPlugin(synthetic) }, rules: { [depsRule]: 'error' }
  }], { filename });
  assert.equal(result.length, 1);
  assert.equal(result[0].ruleId, depsRule);
  assert.equal(result[0].message, message ?? "React Hook useEffect has a missing dependency: 'refresh'. Either include it or remove the dependency array.");
});
