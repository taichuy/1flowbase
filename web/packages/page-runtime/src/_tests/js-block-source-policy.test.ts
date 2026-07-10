import { describe, expect, test } from 'vitest';

import { validateJsBlockSource } from '../index';

const validBlockSkeleton = `
import { defineBlock } from '@1flowbase/block-sdk';
import { Stack, Text } from '@1flowbase/block-renderer/antd-facade';

export default defineBlock({
  render() {
    return Stack({ children: [Text({ children: 'Ready' })] });
  }
});
`;

describe('JS block source static policy', () => {
  test('rejects imports outside a catalog-scoped allowlist', () => {
    expect(
      validateJsBlockSource(
        "import { Button } from '@1flowbase/block-renderer/antd-facade'; export default {};",
        { allowedImports: ['@1flowbase/block-sdk'] }
      )
    ).toMatchObject({
      ok: false,
      errors: [
        {
          code: 'import_denied',
          sourceLocation: { line: 1, column: 1 }
        }
      ]
    });
  });
  test('accepts a block skeleton that imports only the first-party SDKs', () => {
    const result = validateJsBlockSource(validBlockSkeleton);

    expect(result).toEqual({
      ok: true,
      source: validBlockSkeleton,
      normalizedSource: validBlockSkeleton.trim(),
      errors: []
    });
  });

  test.each([
    ['react import', "import React from 'react';"],
    ['antd import', "import { Button } from 'antd';"],
    ['old facade import', "import { Text } from '@1flowbase/antd-facade';"],
    ['dom import', "import { createRoot } from 'react-dom/client';"],
    ['npm package import', "import dayjs from 'dayjs';"]
  ])('rejects denied static import: %s', (_label, source) => {
    const result = validateJsBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'import_denied',
      path: 'source.imports[0]'
    });
  });

  test('JS block policy rejects dynamic import with a stable import error', () => {
    const result = validateJsBlockSource(
      "const mod = await import('@1flowbase/block-sdk');"
    );

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'import_denied',
      path: 'source.imports[0]'
    });
  });

  test.each([
    [
      'require',
      "const sdk = require('@1flowbase/block-sdk');",
      'import_denied'
    ],
    ['eval', "eval('2 + 2');", 'transform_failed'],
    [
      'Function constructor',
      "const fn = new Function('return 1');",
      'transform_failed'
    ]
  ] as const)('rejects executable escape hatch: %s', (_label, source, code) => {
    const result = validateJsBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code
    });
  });

  test.each([
    [
      'optional require',
      "const sdk = require?.('@1flowbase/block-sdk');",
      'import_denied'
    ],
    [
      'require.call',
      "require.call(null, '@1flowbase/block-sdk');",
      'import_denied'
    ],
    ['optional eval', "eval?.('2 + 2');", 'transform_failed'],
    ['eval.call', "eval.call(null, '2 + 2');", 'transform_failed'],
    [
      'optional Function',
      "const fn = Function?.('return 1');",
      'transform_failed'
    ],
    [
      'Function.apply',
      "const fn = Function.apply(null, ['return 1']);",
      'transform_failed'
    ]
  ] as const)(
    'rejects equivalent executable escape hatch: %s',
    (_label, source, code) => {
      const result = validateJsBlockSource(source);

      expect(result.ok).toBe(false);
      expect(result.errors[0]).toMatchObject({
        code
      });
    }
  );

  test.each([
    ['fetch', "await fetch('/api/private');"],
    ['optional fetch', "await fetch?.('/api/private');"],
    ['fetch.call', "fetch.call(null, '/api/private');"],
    ['fetch.apply', "fetch.apply(null, ['/api/private']);"],
    ['fetch.bind', "const boundFetch = fetch.bind(null);"],
    ['XMLHttpRequest', 'const xhr = new XMLHttpRequest();'],
    ['optional XMLHttpRequest', "const xhr = XMLHttpRequest?.('/api/private');"],
    ['WebSocket', "const socket = new WebSocket('wss://example.com');"],
    ['WebSocket.call', "WebSocket.call(null, 'wss://example.com');"],
    ['sendBeacon', "navigator.sendBeacon('/track');"],
    ['optional sendBeacon', "navigator.sendBeacon?.('/track');"],
    ['computed sendBeacon', "navigator['sendBeacon']('/track');"]
  ])('rejects network capability: %s', (_label, source) => {
    const result = validateJsBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['window', 'window.location.href;'],
    ['document', 'document.querySelector("#root");'],
    ['globalThis', 'globalThis.crypto;'],
    ['self', 'self.postMessage({});'],
    ['localStorage', "localStorage.getItem('token');"],
    ['sessionStorage', "sessionStorage.setItem('token', '1');"],
    ['cookie', 'const cookie = document.cookie;']
  ])('rejects DOM or storage capability: %s', (_label, source) => {
    const result = validateJsBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['constructor call', "''.sub.constructor('return globalThis')();"],
    ['computed constructor call', "''.sub['constructor']('return globalThis')();"],
    ['prototype access', 'const proto = Text.prototype;'],
    ['computed prototype access', "const proto = Text['prototype'];"],
    ['__proto__ access', 'const proto = ({}).__proto__;'],
    ['computed __proto__ access', "const proto = ({})['__proto__'];"]
  ])('rejects prototype-chain escape capability: %s', (_label, source) => {
    const result = validateJsBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test('JS block policy ignores dangerous words inside comments and strings', () => {
    const source = `
const label = 'fetch eval Function require XMLHttpRequest WebSocket sendBeacon';
const description = "navigator['sendBeacon']('/track')";
const words = ['constructor', 'prototype', '__proto__'];
// fetch?.('/api/private')
/* eval?.('2 + 2') */
`;

    const result = validateJsBlockSource(source);

    expect(result).toEqual({
      ok: true,
      source,
      normalizedSource: source.trim(),
      errors: []
    });
  });

  test('JS block policy returns syntax_invalid for malformed source without throwing', () => {
    expect(() =>
      validateJsBlockSource('const value = "unterminated')
    ).not.toThrow();

    const result = validateJsBlockSource('const value = "unterminated');

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'syntax_invalid',
      path: 'source'
    });
  });

  test('JS block policy returns a structured failure for non-string source without throwing', () => {
    expect(() => validateJsBlockSource(null)).not.toThrow();

    const result = validateJsBlockSource(null);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed',
      path: 'source'
    });
  });
});
