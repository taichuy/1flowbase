export type EChartPrimitive = string | number | boolean | null;
export type EChartValue =
  | EChartPrimitive
  | readonly EChartValue[]
  | { readonly [key: string]: EChartValue | undefined };
export type EChartOption = Readonly<Record<string, EChartValue | undefined>>;

const UNSAFE_KEYS = new Set([
  'formatter',
  'renderItem',
  'renderMode',
  'map',
  'geoJSON',
  'geoJson'
]);
const EXTERNAL_RESOURCE = /^(?:https?:)?\/\//i;

export function assertSafeEChartOption(
  option: unknown
): asserts option is EChartOption {
  visit(option, 'option', new WeakSet<object>());
}

function visit(value: unknown, path: string, seen: WeakSet<object>): void {
  if (
    value === null ||
    typeof value === 'string' ||
    typeof value === 'boolean'
  ) {
    if (
      typeof value === 'string' &&
      (EXTERNAL_RESOURCE.test(value) || value.startsWith('image://'))
    ) {
      throw new TypeError(`${path} cannot reference an external image or URL.`);
    }
    return;
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) {
      throw new TypeError(`${path} must contain finite JSON numbers only.`);
    }
    return;
  }
  if (typeof value !== 'object') {
    throw new TypeError(`${path} must contain JSON values only.`);
  }
  if (seen.has(value)) {
    throw new TypeError(`${path} must not contain circular references.`);
  }
  seen.add(value);

  if (Array.isArray(value)) {
    value.forEach((item, index) => visit(item, `${path}[${index}]`, seen));
    seen.delete(value);
    return;
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${path} must contain plain JSON objects only.`);
  }

  for (const [key, item] of Object.entries(value)) {
    if (UNSAFE_KEYS.has(key)) {
      throw new TypeError(
        `${path}.${key} is not supported by controlled EChart.`
      );
    }
    if (key === 'type' && (item === 'custom' || item === 'map')) {
      throw new TypeError(`${path}.type cannot use custom or map series.`);
    }
    visit(item, `${path}.${key}`, seen);
  }
  seen.delete(value);
}
