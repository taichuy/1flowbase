import { render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const chart = {
  dispose: vi.fn(),
  resize: vi.fn(),
  setOption: vi.fn()
};

vi.mock('echarts/core', () => ({
  init: vi.fn(() => chart),
  use: vi.fn()
}));

import { EChart } from '../index';

describe('@1flowbase/charts EChart (AC-PUB-004/005)', () => {
  afterEach(() => vi.clearAllMocks());

  it('owns init, controlled updates and repeatable dispose', () => {
    const view = render(
      <EChart option={{ tooltip: { trigger: 'axis' }, series: [] }} />
    );
    expect(chart.setOption).toHaveBeenCalledWith(
      expect.objectContaining({
        tooltip: expect.objectContaining({ renderMode: 'richText' })
      }),
      { lazyUpdate: true, notMerge: true }
    );

    view.unmount();
    expect(chart.dispose).toHaveBeenCalledTimes(1);
  });

  it.each([
    { series: [{ type: 'custom' }] },
    { series: [{ type: 'map' }] },
    { tooltip: { formatter: '{value}' } },
    { symbol: 'image://https://example.test/a.png' }
  ])('rejects unsafe option fixture %#', (option) => {
    expect(() => render(<EChart option={option} />)).toThrow(TypeError);
  });
});
// @vitest-environment jsdom
