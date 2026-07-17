import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { expect, test } from 'vitest';

test('ac_001 keeps the ECharts host shrinkable inside the viewport scroller', async () => {
  const css = await readFile(
    path.resolve(
      import.meta.dirname,
      '../../components/system-runtime/system-runtime-panel.css'
    ),
    'utf8'
  );
  const chartRule = css.match(
    /\.system-runtime-panel__chart\s*\{[\s\S]*?\n\}/
  )?.[0];

  expect(chartRule).toContain('min-width: 0;');
  expect(chartRule).toContain('max-width: 100%;');
  expect(chartRule).toContain('overflow: hidden;');
});
