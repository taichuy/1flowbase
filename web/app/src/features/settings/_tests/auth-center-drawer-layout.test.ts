import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

describe('auth center drawer layout CSS', () => {
  test('stretches the form across the drawer body', () => {
    const cssSource = fs.readFileSync(
      path.resolve(
        import.meta.dirname,
        '../pages/settings-page/auth-center-panel.css'
      ),
      'utf8'
    );

    expect(cssSource).toContain('.settings-auth-center-drawer {');
    expect(cssSource).toContain('width: 100%;');
    expect(cssSource).toContain('.settings-auth-center-drawer .ant-space-item');
    expect(cssSource).toContain('.settings-auth-center-drawer .ant-form');
    expect(cssSource).toContain(
      '.settings-auth-center-drawer__resize-handle {'
    );
    expect(cssSource).toContain('cursor: col-resize;');
    expect(cssSource).toContain('max-width: calc(100vw - 48px);');
  });
});
