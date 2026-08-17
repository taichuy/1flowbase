export const MCP_TOOLS_PAGE_SIZE = 20;

export interface McpToolsRouteState {
  page: number;
  keyword?: string;
  execution_target_kind?: 'interface_wrapper' | 'mcp_proxy';
  interface_id?: string;
  risk_level?: 'low' | 'medium' | 'high' | 'critical';
  des_id_required?: boolean;
  status?: 'draft' | 'enabled' | 'disabled' | 'archived';
}

type HistoryUpdateMode = 'push' | 'replace';

const executionTargetKinds = ['interface_wrapper', 'mcp_proxy'] as const;
const riskLevels = ['low', 'medium', 'high', 'critical'] as const;
const statuses = ['draft', 'enabled', 'disabled', 'archived'] as const;

function optionalSearchValue(search: URLSearchParams, key: string) {
  const value = search.get(key)?.trim();
  return value ? value : undefined;
}

function optionalEnumValue<T extends string>(
  search: URLSearchParams,
  key: string,
  values: readonly T[]
) {
  const value = optionalSearchValue(search, key);
  return value && values.includes(value as T) ? (value as T) : undefined;
}

export function readMcpToolsRouteState(): McpToolsRouteState {
  const search = new URLSearchParams(window.location.search);
  const parsedPage = Number.parseInt(search.get('page') ?? '1', 10);
  const desIdRequired = search.get('des_id_required');

  return {
    page: Number.isFinite(parsedPage) && parsedPage > 0 ? parsedPage : 1,
    keyword: optionalSearchValue(search, 'keyword'),
    execution_target_kind: optionalEnumValue(
      search,
      'execution_target_kind',
      executionTargetKinds
    ),
    interface_id: optionalSearchValue(search, 'interface_id'),
    risk_level: optionalEnumValue(search, 'risk_level', riskLevels),
    des_id_required:
      desIdRequired === 'true'
        ? true
        : desIdRequired === 'false'
          ? false
          : undefined,
    status: optionalEnumValue(search, 'status', statuses)
  };
}

export function writeMcpToolsRouteState(
  state: McpToolsRouteState,
  mode: HistoryUpdateMode = 'push'
) {
  const nextUrl = new URL(window.location.href);
  const managedKeys = [
    'page',
    'keyword',
    'execution_target_kind',
    'interface_id',
    'risk_level',
    'des_id_required',
    'status'
  ];

  for (const key of managedKeys) {
    nextUrl.searchParams.delete(key);
  }

  nextUrl.searchParams.set('tab', 'tools');
  if (state.page > 1) nextUrl.searchParams.set('page', String(state.page));
  if (state.keyword) nextUrl.searchParams.set('keyword', state.keyword);
  if (state.execution_target_kind) {
    nextUrl.searchParams.set(
      'execution_target_kind',
      state.execution_target_kind
    );
  }
  if (state.interface_id) {
    nextUrl.searchParams.set('interface_id', state.interface_id);
  }
  if (state.risk_level) {
    nextUrl.searchParams.set('risk_level', state.risk_level);
  }
  if (state.des_id_required !== undefined) {
    nextUrl.searchParams.set('des_id_required', String(state.des_id_required));
  }
  if (state.status) nextUrl.searchParams.set('status', state.status);

  const nextPath = `${nextUrl.pathname}${nextUrl.search}${nextUrl.hash}`;
  if (mode === 'replace') {
    window.history.replaceState({}, '', nextPath);
  } else {
    window.history.pushState({}, '', nextPath);
  }
}
