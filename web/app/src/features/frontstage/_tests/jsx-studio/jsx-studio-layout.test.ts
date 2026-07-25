import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

const cssSource = fs.readFileSync(
  path.resolve(
    import.meta.dirname,
    '../../../../shared/code-block/block-source-studio.css'
  ),
  'utf8'
);

function cssBlock(selector: string): string {
  return (
    cssSource.match(
      new RegExp(`(?:^|\\n)${selector}\\s*\\{[\\s\\S]*?\\n\\}`)
    )?.[0] ?? ''
  );
}

describe('TSX Studio layout', () => {
  test('places the editor first and mirrors the resource workspace to the right', () => {
    const workspace = cssBlock('\\.frontstage-jsx-studio__workspace');
    expect(workspace).toContain('minmax(0, 1fr) 8px');
    expect(workspace).toContain(
      'minmax(260px, var(--resource-panel-width, 320px))'
    );
    expect(workspace).toContain('44px;');
    expect(cssBlock('\\.frontstage-jsx-studio__editor-panel')).toContain(
      'grid-column: 1;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__editor-panel')).toContain(
      'grid-row: 1;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__editor-panel')).toContain(
      'grid-template-rows: minmax(320px, 1fr) auto;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__resource-panel')).toContain(
      'grid-column: 3;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__resource-panel')).toContain(
      'grid-row: 1;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__rail')).toContain(
      'grid-column: 4;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__rail')).toContain(
      'grid-row: 1;'
    );
  });

  test('keeps the right rail while the code-only editor consumes the remaining width', () => {
    expect(
      cssBlock('\\.frontstage-jsx-studio__workspace--code-only')
    ).toContain('grid-template-columns: minmax(0, 1fr) 44px;');
    expect(cssSource).toContain(
      '.frontstage-jsx-studio__workspace--code-only\n  .frontstage-jsx-studio__resource-panel {\n  display: none;'
    );
  });

  test('allows window header actions to shrink within a mobile viewport', () => {
    expect(
      cssBlock('\\.frontstage-jsx-studio__window-actions')
    ).toContain('min-width: 0;');
  });
});
