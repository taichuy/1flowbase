import { withUserPreferenceWriteAccess } from '../../../shared/user-preferences/user-preferences';

const STORAGE_KEY = '1flowbase.notifications.inbox_split_ratio';

export function readInboxSplitRatio(): number | null {
  try {
    const saved = window.localStorage.getItem(STORAGE_KEY);
    const ratio = saved === null ? NaN : Number(saved);
    return Number.isFinite(ratio) && ratio > 0 && ratio < 1 ? ratio : null;
  } catch {
    return null;
  }
}

export function persistInboxSplitRatio(ratio: number) {
  withUserPreferenceWriteAccess(() => {
    try {
      window.localStorage.setItem(
        STORAGE_KEY,
        String(Math.round(ratio * 10000) / 10000)
      );
    } catch {
      // Keep resizing usable when browser storage is unavailable.
    }
  });
}
