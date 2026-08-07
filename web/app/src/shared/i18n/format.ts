import { appI18n } from './app-i18n';
import { FALLBACK_APP_LOCALE, type AppLocale, toAppLocale } from './locales';

const APP_LOCALE_TO_INTL_LOCALE: Record<AppLocale, string> = {
  zh_Hans: 'zh-CN',
  en_US: 'en-US'
};

export function getCurrentIntlLocale(): string {
  const appLocale =
    toAppLocale(appI18n.resolvedLanguage) ??
    toAppLocale(appI18n.language) ??
    FALLBACK_APP_LOCALE;

  return APP_LOCALE_TO_INTL_LOCALE[appLocale];
}

export function formatDateTime(value: string | Date, options?: Intl.DateTimeFormatOptions): string {
  const date = typeof value === 'string' ? new Date(value) : value;

  return date.toLocaleString(getCurrentIntlLocale(), options);
}

export function formatDate(value: string | Date, options?: Intl.DateTimeFormatOptions): string {
  const date = typeof value === 'string' ? new Date(value) : value;

  return date.toLocaleDateString(getCurrentIntlLocale(), options);
}

export function formatTime(value: string | Date, options?: Intl.DateTimeFormatOptions): string {
  const date = typeof value === 'string' ? new Date(value) : value;

  return date.toLocaleTimeString(getCurrentIntlLocale(), options);
}

export function formatNumber(value: number, options?: Intl.NumberFormatOptions): string {
  return value.toLocaleString(getCurrentIntlLocale(), options);
}

export function formatTokenCount(value: number): string {
  const absoluteValue = Math.abs(value);
  const unit = [
    { threshold: 1_000_000_000, suffix: 'B' },
    { threshold: 1_000_000, suffix: 'M' },
    { threshold: 1_000, suffix: 'K' }
  ].find((candidate) => absoluteValue >= candidate.threshold);

  if (!unit) {
    return formatNumber(value, { maximumFractionDigits: 0 });
  }

  return `${formatNumber(value / unit.threshold, {
    maximumFractionDigits: 1
  })}${unit.suffix}`;
}
