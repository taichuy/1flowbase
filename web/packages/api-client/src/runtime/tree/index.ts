import { apiFetch } from '../../transport';
export interface TreePage<T> {
  items: T[];
  has_more: boolean;
  next_cursor: string | null;
}
export interface TreePageParams {
  limit?: number;
  cursor?: string;
}
export interface TreeDescendant<T> {
  record: T;
  depth: number;
  has_children: boolean;
  path: string[] | null;
}
export interface TreeSearchItem<T> {
  record: T;
  is_match: boolean;
}
export interface TreeDescendantParams extends TreePageParams {
  max_depth?: number;
  include_path?: boolean;
}
function request<T>(model_code: string, route: string, params: object = {}) {
  const query = new URLSearchParams();
  for (const [key, value] of Object.entries(params))
    if (value !== undefined) query.set(key, String(value));
  return apiFetch<T>({
    path: `/api/runtime/models/${encodeURIComponent(model_code)}/tree/${route}${query.size ? `?${query}` : ''}`
  });
}
export const listRuntimeTreeRoots = <T = Record<string, unknown>>(
  model_code: string,
  params: TreePageParams = {}
) => request<TreePage<T>>(model_code, 'roots', params);
export const listRuntimeTreeChildren = <T = Record<string, unknown>>(
  model_code: string,
  id: string,
  params: TreePageParams = {}
) =>
  request<TreePage<T>>(
    model_code,
    `children/${encodeURIComponent(id)}`,
    params
  );
export const listRuntimeTreeAncestors = <T = Record<string, unknown>>(
  model_code: string,
  id: string
) => request<T[]>(model_code, `ancestors/${encodeURIComponent(id)}`);
export const listRuntimeTreeDescendants = <T = Record<string, unknown>>(
  model_code: string,
  id: string,
  params: TreeDescendantParams = {}
) =>
  request<TreePage<TreeDescendant<T>>>(
    model_code,
    `descendants/${encodeURIComponent(id)}`,
    params
  );
export const searchRuntimeTree = <T = Record<string, unknown>>(
  model_code: string,
  params: TreePageParams & { prefix: string }
) => request<TreePage<TreeSearchItem<T>>>(model_code, 'search', params);
