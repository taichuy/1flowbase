import { useAuthStore } from '../../../state/auth-store';
import { hasConsoleOperationAccess } from '../../../state/console-operation-access';

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
  const auth = useAuthStore.getState();
  if (
    !hasConsoleOperationAccess(
      auth.actor?.effective_display_role,
      auth.me?.permissions,
      'patch_me_meta'
    )
  )
    return;
  try {
    window.localStorage.setItem(
      STORAGE_KEY,
      String(Math.round(ratio * 10000) / 10000)
    );
  } catch {
    // Keep resizing usable when browser storage is unavailable.
  }
}
