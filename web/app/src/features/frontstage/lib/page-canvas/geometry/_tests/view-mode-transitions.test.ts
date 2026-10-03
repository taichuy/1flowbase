import css from '../../../../components/page-canvas.css?raw';
import component from '../../../../components/PageCanvas.tsx?raw';
import { expect, it } from 'vitest';

it('limits passive transition suppression to the view canvas wrapper', () => {
  expect(css).toMatch(
    /\.frontstage-page-canvas-grid--view \.react-grid-item\s*\{\s*transition:\s*none;/
  );
  expect(component).toContain(
    "isDesignMode ? '' : ' frontstage-page-canvas-grid--view'"
  );
});
