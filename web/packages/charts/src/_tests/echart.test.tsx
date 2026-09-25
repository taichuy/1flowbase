import { render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const chart = {
  on: vi.fn(),
  off: vi.fn(),
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

  it('adds a unit to tooltip values through the trusted chart renderer', () => {
    render(
      <EChart
        option={{ tooltip: { trigger: 'axis' }, series: [] }}
        tooltipValueUnit="MB"
      />
    );

    const option = chart.setOption.mock.calls[0]?.[0] as {
      tooltip?: { valueFormatter?: (value: number) => string };
    };
    expect(option.tooltip?.valueFormatter?.(526.31)).toBe('526.31 MB');
  });

  it('adds a unit to numeric axis labels through the trusted chart renderer', () => {
    render(
      <EChart
        option={{ yAxis: { type: 'value', axisLabel: { color: '#777' } }, series: [] }}
        yAxisValueUnit="MB"
      />
    );

    const option = chart.setOption.mock.calls[0]?.[0] as {
      yAxis?: { axisLabel?: { color?: string; formatter?: (value: number) => string } };
    };
    expect(option.yAxis?.axisLabel?.color).toBe('#777');
    expect(option.yAxis?.axisLabel?.formatter?.(500)).toBe('500 MB');
  });

  it('bridges data clicks and removes listeners when the handler changes', () => {
    const handler = vi.fn();
    const view = render(
      <EChart option={{ series: [] }} onDataClick={handler} />
    );
    const listener = chart.on.mock.calls[0][1];
    listener({ dataIndex: 2 });
    listener({});
    expect(handler).toHaveBeenCalledExactlyOnceWith(2);
    view.rerender(<EChart option={{ series: [] }} />);
    expect(chart.off).toHaveBeenCalledWith('click', listener);
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

import '@testing-library/jest-dom/vitest';
