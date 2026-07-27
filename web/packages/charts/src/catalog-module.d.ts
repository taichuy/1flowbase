declare module '@1flowbase/charts' {
  import type { ComponentType, CSSProperties } from 'react';

  export type EChartPrimitive = string | number | boolean | null;
  export type EChartValue =
    | EChartPrimitive
    | readonly EChartValue[]
    | { readonly [key: string]: EChartValue | undefined };
  export type EChartOption = Readonly<Record<string, EChartValue | undefined>>;
  export interface EChartProps {
    readonly ariaLabel?: string;
    readonly className?: string;
    readonly option: EChartOption;
    readonly style?: CSSProperties;
  }
  export const EChart: ComponentType<EChartProps>;
}
