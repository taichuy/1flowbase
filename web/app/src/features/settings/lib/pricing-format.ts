export function formatTokenUnit(value: number) {
  const units = [
    { threshold: 1_000_000_000, suffix: 'B' },
    { threshold: 1_000_000, suffix: 'M' },
    { threshold: 1_000, suffix: 'K' }
  ];
  const unit = units.find(({ threshold }) => value >= threshold);
  if (!unit) return String(value);
  const scaled = (value / unit.threshold)
    .toFixed(2)
    .replace(/\.0+$/u, '')
    .replace(/(\.\d*[1-9])0+$/u, '$1');
  return `${scaled}${unit.suffix}`;
}

export function formatUsdRate(value: string) {
  const [rawInteger = '0', rawFraction = ''] = value.split('.');
  const integer = rawInteger.replace(/^0+(?=\d)/u, '') || '0';
  const significantFraction = rawFraction.replace(/0+$/u, '');
  if (integer === '0' && significantFraction.length === 0) return '0';
  if (significantFraction.length === 0) return `${integer}.00`;
  if (significantFraction.length === 1)
    return `${integer}.${significantFraction}0`;
  return `${integer}.${significantFraction}`;
}

export function formatPricingRate(price: string, tokenUnit: number) {
  return `${formatTokenUnit(tokenUnit)} / ${formatUsdRate(price)}$`;
}
