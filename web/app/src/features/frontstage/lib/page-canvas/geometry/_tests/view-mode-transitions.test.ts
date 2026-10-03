import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { expect, it } from 'vitest';

it('limits passive transition suppression to the view canvas wrapper', () => {
  // The app wrapper runs Vitest with web/app as cwd. Test setup intentionally
  // mocks ?raw imports, so inspect the real scoped styles through Node instead.
  const sourceRoot = resolve(
    process.cwd(),
    'src/features/frontstage/components'
  );
  const css = readFileSync(resolve(sourceRoot, 'page-canvas.css'), 'utf8');
  const component = readFileSync(resolve(sourceRoot, 'PageCanvas.tsx'), 'utf8');
  expect(css).toMatch(
    /\.frontstage-page-canvas-grid--view \.react-grid-item\s*\{\s*transition:\s*none;/
  );
  expect(component).toContain(
    "isDesignMode ? '' : ' frontstage-page-canvas-grid--view'"
  );
});
