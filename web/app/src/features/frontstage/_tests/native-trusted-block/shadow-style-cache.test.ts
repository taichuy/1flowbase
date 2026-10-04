import { afterEach, describe, expect, test, vi } from 'vitest';
import { createRequire } from 'node:module';
import { updateCSS as updateIconCSS } from '@ant-design/icons/es/cssUtils';

const require = createRequire(import.meta.url);
// Resolve through antd so this exercises its installed transitive icon cache.
const requireFromAntd = createRequire(require.resolve('antd/package.json'));
const { updateCSS: updateTransitiveIconCSS } = requireFromAntd(
  '@ant-design/icons/lib/cssUtils'
) as { updateCSS: typeof updateIconCSS };
const { clearContainerCache, removeCSS, updateCSS } = requireFromAntd(
  '@rc-component/util/lib/Dom/dynamicCSS'
) as {
  clearContainerCache(): void;
  removeCSS(key: string, option: { attachTo: ShadowRoot }): void;
  updateCSS: typeof updateIconCSS;
};

afterEach(() => {
  vi.restoreAllMocks();
  clearContainerCache();
  document.body.replaceChildren();
});

test('covers both the direct and actual antd transitive icon versions', () => {
  expect(require('@ant-design/icons/package.json').version).toBe('6.3.4');
  expect(requireFromAntd('@ant-design/icons/package.json').version).toBe('6.3.2');
  expect(updateTransitiveIconCSS).not.toBe(updateIconCSS);
});

describe.each([
  ['direct icons 6.3.4', updateIconCSS],
  ['antd transitive icons 6.3.2', updateTransitiveIconCSS],
  ['rc-util', updateCSS]
] as const)('%s ShadowRoot style ownership', (_name, update) => {
  test('reuses connected containers without placeholder churn and keeps scopes isolated', () => {
    const first = document.createElement('div');
    const second = document.createElement('div');
    document.body.append(first, second);
    const a = first.attachShadow({ mode: 'open' });
    const b = second.attachShadow({ mode: 'open' });
    const styleA = update('.probe {color:red}', 'probe', { attachTo: a });
    const styleB = update('.probe {color:blue}', 'probe', { attachTo: b });
    const append = vi.spyOn(a, 'appendChild');
    expect(update('.probe {color:green}', 'probe', { attachTo: a })).toBe(
      styleA
    );
    expect(append).not.toHaveBeenCalled();
    expect(a.querySelectorAll('style')).toHaveLength(1);
    expect(styleB?.textContent).toContain('blue');
    first.remove();
    document.body.append(first);
    expect(update('.probe {color:red}', 'probe', { attachTo: a })).toBe(styleA);
    expect(append).not.toHaveBeenCalled();
  });
});

test('rc-util cache reset and removeCSS still support a retained ShadowRoot', () => {
  const host = document.createElement('div');
  document.body.append(host);
  const root = host.attachShadow({ mode: 'open' });
  const style = updateCSS('.probe {}', 'probe', { attachTo: root });
  clearContainerCache();
  expect(updateCSS('.probe {color:red}', 'probe', { attachTo: root })).toBe(
    style
  );
  removeCSS('probe', { attachTo: root });
  expect(root.querySelector('style')).toBeNull();
});
