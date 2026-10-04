import { Activity, StrictMode } from 'react';
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const chart = {
  on: vi.fn(),
  off: vi.fn(),
  dispose: vi.fn(),
  resize: vi.fn(),
  setOption: vi.fn(),
  getWidth: () => 1280,
  getHeight: () => 800,
  getZr: () => ({ animation, wakeUp: animation.start })
};

const animation = {
  start: vi.fn(),
  resume: vi.fn(),
  pause: vi.fn(),
  stop: vi.fn()
};

vi.mock('echarts/core', () => ({
  init: vi.fn((mount: HTMLElement) => {
    const canvas = document.createElement('canvas');
    mount.append(canvas);
    chart.dispose.mockImplementation(() => canvas.remove());
    return chart;
  }),
  use: vi.fn()
}));

import { EChart } from '../index';
import { EChartResourceBoundary } from '../lifecycle';

describe('@1flowbase/charts EChart (AC-PUB-004/005)', () => {
  afterEach(async () => {
    cleanup();
    await Promise.resolve();
    vi.clearAllMocks();
  });

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
        option={{
          yAxis: { type: 'value', axisLabel: { color: '#777' } },
          series: []
        }}
        yAxisValueUnit="MB"
      />
    );

    const option = chart.setOption.mock.calls[0]?.[0] as {
      yAxis?: {
        axisLabel?: { color?: string; formatter?: (value: number) => string };
      };
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

  it('retains the canvas across Activity while pausing work and disposes on eviction', async () => {
    const option = { series: [{ id: 'tokens', type: 'line', data: [1, 2] }] };
    const tree = (visible: boolean, present = true) => (
      <EChartResourceBoundary>
        <Activity mode={visible ? 'visible' : 'hidden'}>
          {present && <EChart option={option} replaceMerge={['series']} />}
        </Activity>
      </EChartResourceBoundary>
    );
    const view = render(tree(true));
    const canvas = view.container.querySelector('canvas');
    expect(canvas).not.toBeNull();
    view.rerender(tree(false));
    await act(async () => {});
    expect(chart.dispose).not.toHaveBeenCalled();
    expect(animation.stop).toHaveBeenCalled();
    view.rerender(tree(true));
    expect(view.container.querySelector('canvas')).toBe(canvas);
    expect(chart.setOption).toHaveBeenCalledTimes(1);
    expect(animation.start).toHaveBeenCalledTimes(2);
    expect(chart.resize).not.toHaveBeenCalled();
    view.unmount();
    await act(async () => {});
    expect(canvas!.isConnected).toBe(false);
    expect(chart.dispose).toHaveBeenCalledTimes(1);
  });

  it('can replay effects under StrictMode and subsequently hide and reveal', async () => {
    const tree = (visible: boolean) => (
      <StrictMode>
        <EChartResourceBoundary>
          <Activity mode={visible ? 'visible' : 'hidden'}>
            <EChart option={{ series: [] }} />
          </Activity>
        </EChartResourceBoundary>
      </StrictMode>
    );
    const view = render(tree(true));
    const canvas = view.container.querySelector('canvas');
    expect(canvas).not.toBeNull();
    const disposed = chart.dispose.mock.calls.length;
    view.rerender(tree(false));
    await act(async () => {});
    view.rerender(tree(true));
    expect(view.container.querySelector('canvas')).toBe(canvas);
    expect(chart.dispose).toHaveBeenCalledTimes(disposed);
    view.unmount();
    expect(chart.dispose).toHaveBeenCalledTimes(disposed + 1);
  });

  it('releases a removed chart even when its resource owner remains mounted', async () => {
    const view = render(
      <EChartResourceBoundary>
        <EChart option={{ series: [] }} />
      </EChartResourceBoundary>
    );
    view.rerender(
      <EChartResourceBoundary>
        <span>empty</span>
      </EChartResourceBoundary>
    );
    await act(async () => {});
    expect(chart.dispose).toHaveBeenCalledTimes(1);
    view.unmount();
    expect(chart.dispose).toHaveBeenCalledTimes(1);
  });

  it('skips equal options and requests series replacement when the series disappear', () => {
    const option = { series: [{ id: 'tokens', type: 'line', data: [1, 2] }] };
    const view = render(<EChart option={option} replaceMerge={['series']} />);
    view.rerender(
      <EChart option={structuredClone(option)} replaceMerge={['series']} />
    );
    expect(chart.setOption).toHaveBeenCalledTimes(1);
    view.rerender(<EChart option={{ series: [] }} replaceMerge={['series']} />);
    expect(chart.setOption).toHaveBeenLastCalledWith(
      expect.objectContaining({ series: [] }),
      {
        notMerge: false,
        replaceMerge: ['series'],
        lazyUpdate: true
      }
    );
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
