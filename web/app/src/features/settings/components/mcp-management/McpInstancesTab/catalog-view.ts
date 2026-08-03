import type { ConsoleMcpCatalog, ConsoleMcpGroup } from '@1flowbase/api-client';

import { i18nText } from '../../../../../shared/i18n/text';
import { normalizeMcpDirectoryPath } from '../mcp-management-view-model';

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

function readablePath(
  instanceName: string,
  path: string,
  groups: Map<string, ConsoleMcpGroup>
) {
  if (path === '/') return `${instanceName} /`;
  const pathParts = [instanceName];
  let currentPath = '';
  for (const segment of path.split('/').filter(Boolean)) {
    currentPath += `/${segment}`;
    pathParts.push(
      groups.get(currentPath)?.display_name?.trim() ||
        segment ||
        i18nText('settingsMcpManagement', 'auto.unnamed')
    );
  }
  return pathParts.join(' / ');
}

export function formatMcpDirectoryPath(
  instanceName: string,
  rawPath: string | null | undefined,
  groups: Map<string, ConsoleMcpGroup>
) {
  return readablePath(
    instanceName || 'mcp',
    normalizeMcpDirectoryPath(rawPath || '/'),
    groups
  );
}

export function formatMcpGroupEditorPath({
  instanceName,
  selectedDirectoryKey,
  currentPath,
  parentPath,
  draftDisplayName,
  groups
}: {
  instanceName: string;
  selectedDirectoryKey: string;
  currentPath: string | undefined;
  parentPath: string | null;
  draftDisplayName: string | undefined;
  groups: Map<string, ConsoleMcpGroup>;
}) {
  if (selectedDirectoryKey.startsWith('group:')) {
    return readablePath(instanceName || 'mcp', currentPath || '/', groups);
  }
  const parent = readablePath(
    instanceName || 'mcp',
    normalizeMcpDirectoryPath(parentPath || '/'),
    groups
  );
  return `${parent === `${instanceName || 'mcp'} /` ? instanceName || 'mcp' : parent} / ${draftDisplayName?.trim() || i18nText('settingsMcpManagement', 'auto.unnamed')}`;
}
