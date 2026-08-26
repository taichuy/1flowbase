import { describe, expect, test } from 'vitest';

import {
  createUnrestrictedTsxBlockSrcdoc,
  transformUnrestrictedTsxBlockSource
} from '../unrestricted-tsx-block-realm';

describe('unrestricted TSX block realm', () => {
  test('AC-001 transforms standard and HTTPS imports into browser modules', () => {
    const result = transformUnrestrictedTsxBlockSource(`
      import React from 'react';
      import { Space } from 'antd';
      import Widget from 'https://esm.sh/example-widget@1';

      export default function App() {
        return <Space><Widget /></Space>;
      }
    `);

    expect(result.ok).toBe(true);
    if (!result.ok) return;
    expect(result.moduleSource).toContain("https://esm.sh/react@19.0.0");
    expect(result.moduleSource).toContain("https://esm.sh/antd@6.1.1");
    expect(result.moduleSource).toContain(
      "https://esm.sh/example-widget@1"
    );
  });

  test('AC-002 permits external scripts inside the Block-owned document', () => {
    const result = createUnrestrictedTsxBlockSrcdoc({
      moduleSource: 'export default () => null;',
      baseUrl: 'https://console.example.test/demo'
    });

    expect(result).toContain("script-src 'unsafe-inline' https: blob: data:");
    expect(result).toContain("<base href=\"https://console.example.test/demo\">");
    expect(result).toContain('import(moduleUrl)');
  });
});
