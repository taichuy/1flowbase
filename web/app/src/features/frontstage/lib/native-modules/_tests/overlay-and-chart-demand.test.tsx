import { render, screen, waitFor } from '@testing-library/react';
import type { ComponentType } from 'react';
import { beforeEach, describe, expect, test, vi } from 'vitest';

const chartSpies = vi.hoisted(() => ({
  echartModuleImports: 0,
  tokenTrendModuleImports: 0,
  datePickerModuleImports: 0
}));

vi.mock('antd/es/date-picker', () => {
  chartSpies.datePickerModuleImports += 1;
  const MockDatePicker = Object.assign(
    ({ placeholder }: { placeholder?: string }) => (
      <div data-testid="mounted-date-picker">{placeholder ?? 'picker'}</div>
    ),
    {
      RangePicker: ({ className }: { className?: string }) => (
        <div data-testid="mounted-range-picker">{className ?? 'range'}</div>
      ),
      MonthPicker: () => <div data-testid="mounted-month-picker" />,
      QuarterPicker: () => <div data-testid="mounted-quarter-picker" />,
      TimePicker: () => <div data-testid="mounted-time-picker" />,
      WeekPicker: () => <div data-testid="mounted-week-picker" />,
      YearPicker: () => <div data-testid="mounted-year-picker" />
    }
  );
  return { default: MockDatePicker };
});

vi.mock('@1flowbase/charts', () => {
  chartSpies.echartModuleImports += 1;
  return {
    EChart: ({ ariaLabel }: { ariaLabel?: string }) => (
      <div data-testid="mounted-echart">{ariaLabel ?? 'chart'}</div>
    )
  };
});

vi.mock('../../../../../shared/ui/token-trend/TokenTrendChart', () => {
  chartSpies.tokenTrendModuleImports += 1;
  return {
    TokenTrendChart: ({ ariaLabel }: { ariaLabel?: string }) => (
      <div data-testid="mounted-token-trend">{ariaLabel ?? 'trend'}</div>
    )
  };
});

import { createFrontstageNativeReactModuleRegistry } from '../registry';

describe('universal dynamic block overlay and chart demand', () => {
  beforeEach(() => {
    chartSpies.echartModuleImports = 0;
    chartSpies.tokenTrendModuleImports = 0;
    chartSpies.datePickerModuleImports = 0;
  });

  test('resolving @1flowbase/token-trend and @1flowbase/charts does not load chart engines until mounted, and preserves synchronous option validation', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const resolved = await registry.resolveInjectedModuleMap!([
      {
        source: '@1flowbase/token-trend',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/token-trend',
            imported: 'TokenTrendChart',
            local: 'TokenTrendChart'
          }
        ]
      },
      {
        source: '@1flowbase/charts',
        bindings: [
          {
            kind: 'named',
            source: '@1flowbase/charts',
            imported: 'EChart',
            local: 'EChart'
          }
        ]
      }
    ]);

    // Empty-state blocks (like Reports with 0 requests) never mount the chart component
    expect(chartSpies.echartModuleImports).toBe(0);
    expect(chartSpies.tokenTrendModuleImports).toBe(0);

    const EChartFacade = (
      resolved['@1flowbase/charts'] as {
        EChart: ComponentType<{
          option: Record<string, unknown>;
          ariaLabel?: string;
        }>;
      }
    ).EChart;
    const TokenTrendFacade = (
      resolved['@1flowbase/token-trend'] as {
        TokenTrendChart: ComponentType<{
          points: readonly never[];
          ariaLabel?: string;
        }>;
      }
    ).TokenTrendChart;

    // Security invariant: unsafe option still throws synchronously before async load
    expect(() =>
      render(<EChartFacade option={{ series: [{ type: 'custom' }] }} />)
    ).toThrow(TypeError);

    render(<TokenTrendFacade ariaLabel="Token Trend" points={[]} />);
    await waitFor(() => {
      expect(screen.getByTestId('mounted-token-trend')).toHaveTextContent(
        'Token Trend'
      );
    });
    expect(chartSpies.tokenTrendModuleImports).toBe(1);
  });

  test('selective antd host overlay loaders preserve static subcomponents per leaf', async () => {
    const registry = createFrontstageNativeReactModuleRegistry();
    const resolved = await registry.resolveInjectedModuleMap!([
      {
        source: 'antd',
        bindings: [
          {
            kind: 'named',
            source: 'antd',
            imported: 'DatePicker',
            local: 'DatePicker'
          },
          {
            kind: 'named',
            source: 'antd',
            imported: 'Select',
            local: 'Select'
          },
          {
            kind: 'named',
            source: 'antd',
            imported: 'Tooltip',
            local: 'Tooltip'
          }
        ]
      }
    ]);
    const antd = resolved.antd as {
      DatePicker: ComponentType<{ placeholder?: string }> & {
        RangePicker: ComponentType<{ className?: string }>;
      };
      Select: { Option?: unknown };
      Tooltip: unknown;
    };
    expect(typeof antd.DatePicker.RangePicker).toBe('object');
    expect(typeof antd.Select.Option).toBe('function');
    expect(typeof antd.Tooltip).toBe('object');

    // Unmounted DatePicker / RangePicker (e.g. Reports default 24h view) loads 0 B of antd-picker
    expect(chartSpies.datePickerModuleImports).toBe(0);

    const RangePickerFacade = antd.DatePicker.RangePicker;
    render(<RangePickerFacade className="custom-range" />);
    await waitFor(() => {
      expect(screen.getByTestId('mounted-range-picker')).toHaveTextContent(
        'custom-range'
      );
    });
    expect(chartSpies.datePickerModuleImports).toBe(1);
  });
});
