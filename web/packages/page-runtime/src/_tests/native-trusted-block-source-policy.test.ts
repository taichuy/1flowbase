import { describe, expect, test } from 'vitest';

import {
  NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS,
  NATIVE_TRUSTED_BLOCK_PERMISSION,
  NATIVE_TRUSTED_BLOCK_RUNTIME,
  validateNativeTrustedBlockSource
} from '../index';

const validNativeTrustedBlock = `
import React from 'react';
import { Button, Space } from 'antd';
import { Surface } from '@1flowbase/ui';

export default function NativeTrustedBlock() {
  return React.createElement(Surface, null, React.createElement(Space, null, React.createElement(Button, null, 'Run')));
}
`;

describe('Native trusted block source static policy', () => {
  test('exports the runtime contract constants', () => {
    expect(NATIVE_TRUSTED_BLOCK_RUNTIME).toBe('native_trusted_block');
    expect(NATIVE_TRUSTED_BLOCK_PERMISSION).toBe('ui_block.javascript.native');
    expect(NATIVE_TRUSTED_BLOCK_ALLOWED_IMPORTS).toEqual([
      'react',
      'react/jsx-runtime',
      'antd',
      '@1flowbase/ui'
    ]);
  });

  test('accepts native component imports for future trusted rendering', () => {
    const result = validateNativeTrustedBlockSource(validNativeTrustedBlock);

    expect(result).toEqual({
      ok: true,
      source: validNativeTrustedBlock,
      normalizedSource: validNativeTrustedBlock.trim(),
      errors: []
    });
  });

  test.each([
    ['react named import', "import { useMemo } from 'react';"],
    ['antd component import', "import { Button } from 'antd';"],
    ['first-party UI import', "import { Surface } from '@1flowbase/ui';"],
    ['allowed re-export', "export { Surface } from '@1flowbase/ui';"]
  ])('accepts allowed source import: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result).toEqual({
      ok: true,
      source,
      normalizedSource: source.trim(),
      errors: []
    });
  });

  test.each([
    ['react-dom import', "import ReactDOM from 'react-dom';"],
    ['react-dom client import', "import { createRoot } from 'react-dom/client';"],
    ['CSS import', "import './native-block.css';"],
    ['arbitrary npm import', "import dayjs from 'dayjs';"]
  ])('rejects denied static import: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'import_denied',
      path: 'source.imports[0]'
    });
  });

  test('native trusted block policy rejects dynamic import with a stable import error', () => {
    const result = validateNativeTrustedBlockSource(
      "const mod = await import('antd');"
    );

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'import_denied',
      path: 'source.imports[0]'
    });
  });

  test.each([
    ['require', "const antd = require('antd');", 'import_denied'],
    ['eval', "eval('2 + 2');", 'transform_failed'],
    ['Function constructor', "const fn = new Function('return 1');", 'transform_failed']
  ] as const)('rejects executable escape hatch: %s', (_label, source, code) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code
    });
  });

  test.each([
    ['fetch', "await fetch('/api/private');"],
    ['XMLHttpRequest', 'const xhr = new XMLHttpRequest();'],
    ['WebSocket', "const socket = new WebSocket('wss://example.com');"],
    ['sendBeacon', "navigator.sendBeacon('/track');"]
  ])('rejects network capability: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['localStorage', "localStorage.getItem('token');"],
    ['sessionStorage', "sessionStorage.setItem('token', '1');"],
    ['document cookie', 'const token = document.cookie;'],
    ['window', 'window.location.href;'],
    ['document', 'document.querySelector("#root");'],
    ['globalThis', 'globalThis.crypto;'],
    ['self', 'self.postMessage({});']
  ])('rejects DOM or storage capability: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['ReactDOM.createPortal', 'ReactDOM.createPortal(node, target);'],
    ['createPortal identifier', 'createPortal(node, target);'],
    ['createRoot identifier', 'createRoot(target);']
  ])('rejects portal or root ownership escape: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['message global API', "message.success('done');"],
    ['notification global API', "notification.open({ message: 'done' });"],
    ['Modal static method', 'Modal.confirm({ title: "Confirm" });'],
    ['computed Modal static method', "Modal['info']({ title: 'Info' });"],
    ['Upload component usage', 'return React.createElement(Upload);']
  ])('rejects AntD global or privileged API: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    [
      'named import alias',
      "import { Modal as Dialog } from 'antd'; Dialog.confirm({ title: 'Confirm' });"
    ],
    ['local Modal alias', 'const Dialog = Modal; Dialog.confirm({ title: "Confirm" });'],
    [
      'antd destructuring alias',
      'const { Modal: Dialog } = antd; Dialog.confirm({ title: "Confirm" });'
    ],
    [
      'antd.Modal alias',
      'const Dialog = antd.Modal; Dialog.confirm({ title: "Confirm" });'
    ]
  ])('rejects AntD Modal static method through alias: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['constructor call', "''.sub.constructor('return globalThis')();"],
    ['computed constructor call', "''.sub['constructor']('return globalThis')();"],
    ['prototype access', 'const proto = Button.prototype;'],
    ['computed prototype access', "const proto = Button['prototype'];"],
    ['__proto__ access', 'const proto = ({}).__proto__;']
  ])('rejects prototype-chain escape capability: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test.each([
    ['CSSStyleSheet constructor', 'const sheet = new CSSStyleSheet();'],
    ['adoptedStyleSheets assignment', 'root.adoptedStyleSheets = [];'],
    ['styleSheets access', 'const sheets = root.styleSheets;'],
    ['insertRule invocation', "sheet.insertRule('body { color: red; }');"],
    ['computed insertRule invocation', "sheet['insertRule']('body { color: red; }');"],
    ['React style tag injection', "return React.createElement('style', null, 'body { color: red; }');"],
    ['direct style tag injection', "return createElement('style', null, 'body { color: red; }');"]
  ])('rejects stylesheet injection capability: %s', (_label, source) => {
    expect(() => validateNativeTrustedBlockSource(source)).not.toThrow();

    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed'
    });
  });

  test('native trusted block policy ignores dangerous words inside comments and strings', () => {
    const source = `
const label = 'fetch eval Function require XMLHttpRequest WebSocket sendBeacon ReactDOM createPortal Upload';
const words = ['constructor', 'prototype', '__proto__', 'message', 'notification'];
// window.document.cookie
/* Modal.confirm({}) */
`;

    const result = validateNativeTrustedBlockSource(source);

    expect(result).toEqual({
      ok: true,
      source,
      normalizedSource: source.trim(),
      errors: []
    });
  });

  test('native trusted block policy returns syntax_invalid for malformed source without throwing', () => {
    expect(() =>
      validateNativeTrustedBlockSource('const value = "unterminated')
    ).not.toThrow();

    const result = validateNativeTrustedBlockSource('const value = "unterminated');

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'syntax_invalid',
      path: 'source'
    });
  });

  test('native trusted block policy returns a structured failure for non-string source without throwing', () => {
    expect(() => validateNativeTrustedBlockSource(null)).not.toThrow();

    const result = validateNativeTrustedBlockSource(null);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code: 'transform_failed',
      path: 'source'
    });
  });
});
