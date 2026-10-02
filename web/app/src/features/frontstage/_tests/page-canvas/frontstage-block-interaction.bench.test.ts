import { describe, expect, test } from 'vitest';
import type { Layout } from 'react-grid-layout';

import { solveFrontstageBlockInteraction } from '../../lib/page-canvas/frontstage-block-interaction';

function createLayout(size: number): Layout {
  return Array.from({ length: size }, (_, index) => ({
    i: `block-${index}`,
    x: 0,
    y: index * 2,
    w: 24,
    h: 1,
    minW: 1
  }));
}

function percentile95(values: number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.ceil(sorted.length * 0.95) - 1] ?? 0;
}

describe('frontstage block interaction performance', () => {
  test('preserves solver output and reports the interaction budget in its measurement mode', () => {
    const results: Record<string, { p95Ms: number; samples: number }> = {};

    for (const size of [20, 100, 500]) {
      const layout = createLayout(size);
      const samples: number[] = [];
      for (let iteration = 0; iteration < 140; iteration += 1) {
        const startedAt = performance.now();
        const result = solveFrontstageBlockInteraction({
          committedLayout: layout,
          activeId: `block-${size - 1}`,
          proposedPosition: { x: 0, y: 0 },
          columns: 24
        });
        const duration = performance.now() - startedAt;
        expect(result.previewLayout).toHaveLength(size);
        if (iteration >= 20) samples.push(duration);
      }
      results[String(size)] = {
        p95Ms: Number(percentile95(samples).toFixed(3)),
        samples: samples.length
      };
    }

    console.info('FRONTSTAGE_INTERACTION_BENCHMARK', JSON.stringify(results));
    // Coverage sampling adds overhead and cannot certify the uninstrumented latency budget.
    // Keep the same solver samples/assertions; enforce the budget in the regular run lane.
    if (process.env.ONEFLOWBASE_VITEST_MODE === 'coverage') {
      if (results['100']!.p95Ms > 4) {
        console.warn(
          'FRONTSTAGE_INTERACTION_COVERAGE_OVERHEAD',
          JSON.stringify({
            p95Ms: results['100']!.p95Ms,
            uninstrumentedBudgetMs: 4
          })
        );
      }
    } else {
      expect(results['100']!.p95Ms).toBeLessThanOrEqual(4);
    }
  });
});
