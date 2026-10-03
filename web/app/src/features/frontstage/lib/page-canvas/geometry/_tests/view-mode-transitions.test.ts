import { readFileSync } from 'node:fs';
import { expect, it } from 'vitest';

it('limits passive transition suppression to the view canvas wrapper', () => {
  const css = readFileSync(
    new URL('../../../../components/page-canvas.css', import.meta.url),
    'utf8'
  );
  const component = readFileSync(
    new URL('../../../../components/PageCanvas.tsx', import.meta.url),
    'utf8'
  );
  expect(css).toMatch(
    /\.frontstage-page-canvas-grid--view \.react-grid-item\s*\{\s*transition:\s*none;/
  );
  expect(component).toContain(
    "isDesignMode ? '' : ' frontstage-page-canvas-grid--view'"
  );
});
