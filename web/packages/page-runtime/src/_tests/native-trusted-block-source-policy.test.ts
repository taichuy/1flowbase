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
      'antd',
      'antd-style',
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
    ['antd style import', "import { useResponsive } from 'antd-style';"],
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
    [
      'react-dom client import',
      "import { createRoot } from 'react-dom/client';"
    ],
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

  test('preserves scanner locations for catalog import denials (R5-AC-002 / D1-AC-002)', () => {
    const source = [
      "import React from 'react';",
      "  import { Button } from 'antd';",
      "export { Surface } from '@1flowbase/ui';",
      "    export { format } from 'dayjs';",
      "const lazy = import('react');"
    ].join('\n');

    const result = validateNativeTrustedBlockSource(source, {
      allowedImportSources: new Set(['react', '@1flowbase/ui'])
    });

    expect(result).toEqual({
      ok: false,
      errors: [
        {
          code: 'import_denied',
          path: 'source.imports[1]',
          message: "Import source 'antd' is not allowed.",
          sourceLocation: { line: 2, column: 3 }
        },
        {
          code: 'import_denied',
          path: 'source.imports[3]',
          message: "Import source 'dayjs' is not allowed.",
          sourceLocation: { line: 4, column: 5 }
        },
        {
          code: 'import_denied',
          path: 'source.imports[4]',
          message: 'Dynamic import and import host access are not allowed.',
          sourceLocation: { line: 5, column: 14 }
        }
      ]
    });
  });

  test.each([
    ['require', "const antd = require('antd');", 'import_denied'],
    ['eval', "eval('2 + 2');", 'transform_failed']
  ] as const)('rejects executable escape hatch: %s', (_label, source, code) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(false);
    expect(result.errors[0]).toMatchObject({
      code
    });
  });

  test.each([
    ['Function call', "const fn = Function('return 1');"],
    ['Function constructor', "const fn = new Function('return 1');"]
  ])('AC-001 accepts runtime JavaScript construction: %s', (_label, source) => {
    expect(validateNativeTrustedBlockSource(source)).toMatchObject({
      ok: true,
      normalizedSource: source
    });
  });

  test('AC-004 ignores denied capability words inside JSX text', () => {
    const source = `
export default function Block() {
  return <div>classNames Function eval require</div>;
}
`;

    expect(validateNativeTrustedBlockSource(source)).toMatchObject({
      ok: true,
      normalizedSource: source.trim()
    });
  });

  test('AC-004 keeps TypeScript generic syntax in code context', () => {
    const source = 'const values: Array<Function> = [];';

    expect(validateNativeTrustedBlockSource(source)).toMatchObject({
      ok: true,
      normalizedSource: source
    });
  });

  test('preserves syntax errors after a complete JSX element', () => {
    const result = validateNativeTrustedBlockSource(`
const node = <div>Ready</div>;
const label = "unterminated;
`);

    expect(result).toMatchObject({
      ok: false,
      errors: [{ code: 'syntax_invalid', path: 'source' }]
    });
  });

  test.each([
    ['fetch', "await fetch('/api/private');"],
    ['XMLHttpRequest', 'const xhr = new XMLHttpRequest();'],
    ['WebSocket', "const socket = new WebSocket('wss://example.com');"],
    ['sendBeacon', "navigator.sendBeacon('/track');"]
  ])('AC-005 accepts browser network capability: %s', (_label, source) => {
    const result = validateNativeTrustedBlockSource(source);

    expect(result.ok).toBe(true);
  });

  test.each([
    ['localStorage', "localStorage.getItem('token');"],
    ['sessionStorage', "sessionStorage.setItem('token', '1');"],
    ['document cookie', 'const token = document.cookie;'],
    ['window', 'window.location.href;'],
    ['document', 'document.querySelector("#root");'],
    ['globalThis', 'globalThis.crypto;'],
    ['self', 'self.postMessage({});']
  ])(
    'AC-005 accepts browser DOM or storage capability: %s',
    (_label, source) => {
      const result = validateNativeTrustedBlockSource(source);

      expect(result.ok).toBe(true);
    }
  );

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

  test('allows ordinary object properties named like an AntD global API', () => {
    const result = validateNativeTrustedBlockSource(
      `const cause = new Error('failed');
       const props = { message: cause.message };
       return <Alert message={props.message} />;`
    );

    expect(result).toMatchObject({ ok: true });
  });

  test.each([
    [
      'named import alias',
      "import { Modal as Dialog } from 'antd'; Dialog.confirm({ title: 'Confirm' });"
    ],
    [
      'local Modal alias',
      'const Dialog = Modal; Dialog.confirm({ title: "Confirm" });'
    ],
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
    [
      'computed constructor call',
      "''.sub['constructor']('return globalThis')();"
    ],
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
    [
      'computed insertRule invocation',
      "sheet['insertRule']('body { color: red; }');"
    ],
    [
      'React style tag',
      "return React.createElement('style', null, ':root { --tone: red; } @keyframes pulse {} .same { color: var(--tone); }');"
    ],
    [
      'direct style tag',
      "return createElement('style', null, ':root { --tone: blue; } .same { color: var(--tone); }');"
    ]
  ])(
    'allows ShadowRoot-contained stylesheet capability: %s',
    (_label, source) => {
      expect(validateNativeTrustedBlockSource(source).ok).toBe(true);
    }
  );

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

    const result = validateNativeTrustedBlockSource(
      'const value = "unterminated'
    );

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
