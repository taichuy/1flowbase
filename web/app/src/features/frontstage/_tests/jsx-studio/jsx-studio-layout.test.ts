import fs from 'node:fs';
import path from 'node:path';

import { describe, expect, test } from 'vitest';

const cssSource = fs.readFileSync(
  path.resolve(
    import.meta.dirname,
    '../../components/jsx-studio/jsx-studio.css'
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
    expect(cssBlock('\\.frontstage-jsx-studio__workspace')).toContain(
      'grid-template-columns: minmax(0, 1fr) minmax(260px, 320px) 44px;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__editor-panel')).toContain(
      'grid-column: 1;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__editor-panel')).toContain(
      'grid-row: 1;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__resource-panel')).toContain(
      'grid-column: 2;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__resource-panel')).toContain(
      'grid-row: 1;'
    );
    expect(cssBlock('\\.frontstage-jsx-studio__rail')).toContain(
      'grid-column: 3;'
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
});
