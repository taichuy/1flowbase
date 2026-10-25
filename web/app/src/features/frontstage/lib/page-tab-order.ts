import type { FrontstagePageTab } from '../api/page-tabs';

export function createTabRankUpdates(
  tabs: FrontstagePageTab[],
  draggedTabId: string,
  targetTabId: string
): Array<{ tabId: string; rank: string }> {
  const draggedIndex = tabs.findIndex((tab) => tab.id === draggedTabId);
  const targetIndex = tabs.findIndex((tab) => tab.id === targetTabId);
  if (draggedIndex < 0 || targetIndex < 0 || draggedIndex === targetIndex) {
    return [];
  }

  const reorderedTabs = [...tabs];
  const [draggedTab] = reorderedTabs.splice(draggedIndex, 1);
  if (!draggedTab) {
    return [];
  }
  reorderedTabs.splice(targetIndex, 0, draggedTab);

  return reorderedTabs.flatMap((tab, index) => {
    const rank = tabs[index]?.rank;
    return rank && tab.rank !== rank ? [{ tabId: tab.id, rank }] : [];
  });
}

export function nextRank(tabs: FrontstagePageTab[]): string {
  return String((tabs.length + 1) * 1000).padStart(6, '0');
}
