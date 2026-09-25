const STORAGE_KEY = '1flowbase.settings.process_tree_split_ratio';
const DEFAULT_RATIO = 40;

export function readProcessTreeSplitRatio() {
  if (typeof window === 'undefined') {
    return DEFAULT_RATIO;
  }
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    const ratio = saved === null ? NaN : Number(saved);
    return Number.isFinite(ratio) && ratio >= 20 && ratio <= 80
      ? ratio
      : DEFAULT_RATIO;
  } catch {
    return DEFAULT_RATIO;
  }
}

export function persistProcessTreeSplitRatio(sizes: number[]) {
  const total = sizes[0] + sizes[1];
  if (!Number.isFinite(total) || total <= 0) {
    return null;
  }
  const ratio = Math.min(
    80,
    Math.max(20, Math.round((sizes[0] / total) * 1000) / 10)
  );
  try {
    window.localStorage.setItem(STORAGE_KEY, String(ratio));
  } catch {
    // The current split remains usable when browser storage is unavailable.
  }
  return ratio;
}
