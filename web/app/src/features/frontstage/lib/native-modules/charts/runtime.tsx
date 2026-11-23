import { useEffect, useState, type ComponentType } from 'react';
import type { EChartProps } from '@1flowbase/charts';
import { assertSafeEChartOption } from '../../../../../../../packages/charts/src/safe-option';
import type { TokenTrendChartProps } from '../../../../../shared/ui/token-trend/TokenTrendChart';
import {
  appI18n,
  applicationI18nLoader
} from '../../../../../shared/i18n/app-i18n';
import { resolveAppLocale } from '../../../../../shared/i18n/locales';

let cachedEChartComponent: ComponentType<EChartProps> | null = null;
let echartFlight: Promise<ComponentType<EChartProps>> | null = null;

function loadEChartComponent(): Promise<ComponentType<EChartProps>> {
  if (cachedEChartComponent) return Promise.resolve(cachedEChartComponent);
  echartFlight ??= import('@1flowbase/charts')
    .then((mod) => {
      cachedEChartComponent = mod.EChart;
      return mod.EChart;
    })
    .catch((err) => {
      echartFlight = null;
      throw err;
    });
  return echartFlight;
}

let cachedTokenTrendComponent: ComponentType<TokenTrendChartProps> | null =
  null;
let tokenTrendFlight: Promise<ComponentType<TokenTrendChartProps>> | null =
  null;

function loadTokenTrendComponent(): Promise<
  ComponentType<TokenTrendChartProps>
> {
  const locale = resolveAppLocale(appI18n.language);
  const readyI18n = applicationI18nLoader.ready(locale, ['applications']);
  if (cachedTokenTrendComponent && readyI18n) {
    return Promise.resolve(cachedTokenTrendComponent);
  }
  tokenTrendFlight ??= Promise.all([
    import('../../../../../shared/ui/token-trend/TokenTrendChart'),
    applicationI18nLoader.load(locale, ['applications'])
  ])
    .then(([mod]) => {
      cachedTokenTrendComponent = mod.TokenTrendChart;
      return mod.TokenTrendChart;
    })
    .finally(() => {
      tokenTrendFlight = null;
    });
  return tokenTrendFlight;
}

/**
 * Universal mount-deferred EChart facade for dynamic TSX blocks:
 * 1. Enforces synchronous assertSafeEChartOption security invariants immediately.
 * 2. Never blocks block-level UI (filters, KPI cards, Empty states) on the 196 KB gz
 *    ECharts engine; if a block renders <Empty /> instead of <EChart />, 0 bytes load.
 */
export function NativeBlockEChart(props: EChartProps) {
  assertSafeEChartOption(props.option);
  const [Impl, setImpl] = useState<ComponentType<EChartProps> | null>(
    () => cachedEChartComponent
  );

  useEffect(() => {
    if (Impl) return undefined;
    let active = true;
    void loadEChartComponent()
      .then((loaded) => {
        if (active) setImpl(() => loaded);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [Impl]);

  if (!Impl) {
    return (
      <div
        aria-label={props.ariaLabel}
        className={props.className}
        role="img"
        style={props.style}
      />
    );
  }
  return <Impl {...props} />;
}

/**
 * Universal mount-deferred TokenTrendChart facade for dynamic TSX blocks:
 * Defers both @1flowbase/charts (ECharts) and the 'applications' i18n dictionary
 * until <TokenTrendChart /> actually mounts on screen.
 */
export function NativeBlockTokenTrendChart(props: TokenTrendChartProps) {
  const locale = resolveAppLocale(appI18n.language);
  const [Impl, setImpl] = useState<ComponentType<TokenTrendChartProps> | null>(
    () =>
      cachedTokenTrendComponent &&
      applicationI18nLoader.ready(locale, ['applications'])
        ? cachedTokenTrendComponent
        : null
  );

  useEffect(() => {
    if (Impl) return undefined;
    let active = true;
    void loadTokenTrendComponent()
      .then((loaded) => {
        if (active) setImpl(() => loaded);
      })
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [Impl]);

  if (!Impl) {
    return (
      <div
        aria-label={props.ariaLabel}
        role="img"
        style={{ height: props.height ?? 320, width: '100%', minWidth: 0 }}
      />
    );
  }
  return <Impl {...props} />;
}
