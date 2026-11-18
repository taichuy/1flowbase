import type { ConsoleMcpCatalog } from '@1flowbase/api-client';

export function countMcpInstanceDirectoryItems(catalog: ConsoleMcpCatalog) {
  const groupCounts = new Map<string, number>();
  for (const group of catalog.groups) {
    groupCounts.set(
      group.instance_record_id,
      (groupCounts.get(group.instance_record_id) ?? 0) + 1
    );
  }
  const toolCounts = new Map<string, number>();
  for (const binding of catalog.bindings) {
    toolCounts.set(
      binding.instance_record_id,
      (toolCounts.get(binding.instance_record_id) ?? 0) + 1
    );
  }
  return { groupCounts, toolCounts };
}
