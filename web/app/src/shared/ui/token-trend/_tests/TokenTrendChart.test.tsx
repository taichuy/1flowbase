import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { EChartProps } from '@1flowbase/charts';
import { assertSafeEChartOption } from '../../../../../../packages/charts/src/safe-option';
import { TokenTrendChart } from '../TokenTrendChart';

const chart = vi.hoisted(() => ({
  props: undefined as EChartProps | undefined
}));
vi.mock('@1flowbase/charts', () => ({
  EChart: (props: EChartProps) => {
    chart.props = props;
    return <button onClick={() => props.onDataClick?.(0)}>point</button>;
  }
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key })
}));

const point = {
  bucket_start: '2026-10-03T00:00:00Z',
  input_tokens: 100,
  output_tokens: 20,
  input_cache_hit_tokens: 40,
  input_cache_hit_rate: 0.4
};
describe('TokenTrendChart', () => {
  it('keeps token quantities on the first axis and backend rates on the percent axis', () => {
    const onDataClick = vi.fn();
    render(
      <TokenTrendChart
        points={[
          point,
          { ...point, input_tokens: null, input_cache_hit_rate: null }
        ]}
        bucketLabels={['Oct 3', 'Oct 4']}
        onDataClick={onDataClick}
      />
    );
    const option = chart.props!.option;
    expect(() => assertSafeEChartOption(option)).not.toThrow();
    expect(JSON.parse(JSON.stringify(option))).toEqual(option);
    expect(option.xAxis).toMatchObject({ data: ['Oct 3', 'Oct 4'] });
    expect(option.yAxis).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ name: '%', min: 0, max: 100 })
      ])
    );
    expect(option.series).toHaveLength(4);
    expect(option.series).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ data: [100, null], connectNulls: false }),
        expect.objectContaining({ yAxisIndex: 1, data: [40, null] })
      ])
    );
    fireEvent.click(screen.getByText('point'));
    expect(onDataClick).toHaveBeenCalledWith(0);
  });
  it('shows supplied cache writes while preserving zero and missing records', () => {
    render(
      <TokenTrendChart
        points={[
          { ...point, cache_write_tokens: 0 },
          point,
          { ...point, cache_write_tokens: 12 }
        ]}
        labels={{ cache_write_tokens: 'Cache writes' }}
      />
    );
    expect(chart.props!.option.series).toHaveLength(5);
    expect(chart.props!.option.series).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ name: 'Cache writes', data: [0, null, 12] })
      ])
    );
    expect(chart.props!.option.xAxis).toMatchObject({
      data: [point.bucket_start, point.bucket_start, point.bucket_start]
    });
  });
});
