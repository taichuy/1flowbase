export const mcpManagementTabKeys = ['instances', 'tools'] as const;

export type McpManagementTabKey = (typeof mcpManagementTabKeys)[number];

export const defaultMcpManagementTabKey: McpManagementTabKey = 'instances';

type HistoryUpdateMode = 'push' | 'replace';

export function isMcpManagementTabKey(
  value: unknown
): value is McpManagementTabKey {
  return (
    typeof value === 'string' &&
    mcpManagementTabKeys.includes(value as McpManagementTabKey)
  );
}

export function resolveMcpManagementTabKey(
  value: unknown
): McpManagementTabKey {
  return isMcpManagementTabKey(value) ? value : defaultMcpManagementTabKey;
}

export function updateMcpManagementTabQuery(
  tab: McpManagementTabKey,
  mode: HistoryUpdateMode = 'push'
) {
  const nextUrl = new URL(window.location.href);
  nextUrl.searchParams.set('tab', tab);

  const nextPath = `${nextUrl.pathname}${nextUrl.search}`;

  if (mode === 'replace') {
    window.history.replaceState({}, '', nextPath);
  } else {
    window.history.pushState({}, '', nextPath);
  }

  window.dispatchEvent(new PopStateEvent('popstate'));
}
