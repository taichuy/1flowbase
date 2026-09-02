// @vitest-environment jsdom

import { describe, expect, test, vi } from 'vitest';

import { createNativeBlockExternalAssetScope } from '../index';

const ICONFONT_SCRIPT = `(function(window){var svgSprite="<svg>"+"<symbol id='icon-example' viewBox='0 0 16 16'><path d='M0 0h16v16H0z'></path></symbol>"+"</svg>";var script=function(){var scripts=document.getElementsByTagName("script");return scripts[scripts.length-1]}();})(window)`;

describe('native block external asset scope', () => {
  test('AC-002/003/006 keeps style and SVG resources in one ShadowRoot and disposes them together', async () => {
    const host = document.createElement('div');
    document.body.append(host);
    const root = host.attachShadow({ mode: 'open' });
    const fetchText = vi.fn(async (url: string) => {
      expect(url).toBe('https://at.example.test/iconfont.js');
      return ICONFONT_SCRIPT;
    });
    const scope = createNativeBlockExternalAssetScope({ root, fetchText });

    const stylePromise = scope.assets.loadStyle(
      'https://cdn.example.test/theme/main.css'
    );
    const link = root.querySelector('link[rel="stylesheet"]');
    expect(link?.getAttribute('href')).toBe(
      'https://cdn.example.test/theme/main.css'
    );
    link?.dispatchEvent(new Event('load'));
    const styleHandle = await stylePromise;
    const spriteHandle = await scope.assets.loadSvgSprite(
      'https://at.example.test/iconfont.js'
    );

    expect(document.body.querySelector('#icon-example')).toBeNull();
    expect(root.querySelector('#icon-example')).not.toBeNull();

    styleHandle.dispose();
    styleHandle.dispose();
    expect(root.querySelector('link[rel="stylesheet"]')).toBeNull();
    expect(root.querySelector('#icon-example')).not.toBeNull();

    scope.dispose();
    scope.dispose();
    spriteHandle.dispose();
    expect(root.querySelector('#icon-example')).toBeNull();
  });

  test('AC-001/004 loads one HTTPS module flight and scopes removable scripts', async () => {
    const host = document.createElement('div');
    document.body.append(host);
    const root = host.attachShadow({ mode: 'open' });
    const importModule = vi.fn(async () => ({ answer: 42 }));
    const scope = createNativeBlockExternalAssetScope({ root, importModule });

    const [first, second] = await Promise.all([
      scope.assets.importModule<{ answer: number }>(
        'https://esm.example.test/library@1.0.0'
      ),
      scope.assets.importModule<{ answer: number }>(
        'https://esm.example.test/library@1.0.0'
      )
    ]);
    expect(first.answer).toBe(42);
    expect(second).toBe(first);
    expect(importModule).toHaveBeenCalledOnce();

    const scriptPromise = scope.assets.loadScript(
      'https://cdn.example.test/library@1.0.0.js'
    );
    const script = root.querySelector('script');
    expect(script?.getAttribute('src')).toBe(
      'https://cdn.example.test/library@1.0.0.js'
    );
    script?.dispatchEvent(new Event('load'));
    const scriptHandle = await scriptPromise;
    scriptHandle.dispose();
    expect(root.querySelector('script')).toBeNull();

    await expect(
      scope.assets.importModule('http://cdn.example.test/insecure.js')
    ).rejects.toThrow('must use HTTPS');
  });

  test('AC-006 removes and rejects a pending element load when the scope is disposed', async () => {
    const host = document.createElement('div');
    document.body.append(host);
    const root = host.attachShadow({ mode: 'open' });
    const scope = createNativeBlockExternalAssetScope({ root });

    const pendingScript = scope.assets.loadScript(
      'https://cdn.example.test/pending.js'
    );
    const rejectedLoad = expect(pendingScript).rejects.toThrow(
      'scope is disposed'
    );
    expect(root.querySelector('script')).not.toBeNull();

    scope.dispose();

    expect(root.querySelector('script')).toBeNull();
    await rejectedLoad;
  });
});
