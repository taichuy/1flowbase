const COLLAPSED_GROUPS_STORAGE_KEY = 'frontstage_collapsed_groups:v1';
const LEGACY_COLLAPSED_GROUPS_STORAGE_KEY = 'frontstage_collapsed_groups';

export function readCollapsedGroups(storage: Pick<Storage, 'getItem'>): Set<string> {
  try {
    const stored = storage.getItem(COLLAPSED_GROUPS_STORAGE_KEY);
    const legacy = stored === null
      ? storage.getItem(LEGACY_COLLAPSED_GROUPS_STORAGE_KEY)
      : null;
    const value: unknown = JSON.parse(stored ?? legacy ?? '[]');
    return new Set(
      Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string') : []
    );
  } catch {
    return new Set();
  }
}

export function writeCollapsedGroups(
  storage: Pick<Storage, 'setItem'>,
  groups: ReadonlySet<string>
): void {
  storage.setItem(COLLAPSED_GROUPS_STORAGE_KEY, JSON.stringify(Array.from(groups)));
}
