export interface ApplicationManagementRouteState {
  page: number;
  application_type?: 'agent_flow' | 'workflow';
  publication_status?: 'published' | 'unpublished';
  created_by?: string;
  tag_id?: string;
  keyword?: string;
  sort: string;
}

export const APPLICATION_MANAGEMENT_DEFAULT_SORT = 'updated_at:desc';

function optionalSearchValue(search: URLSearchParams, key: string) {
  const value = search.get(key)?.trim();
  return value ? value : undefined;
}

export function readApplicationManagementRouteState(): ApplicationManagementRouteState {
  const search = new URLSearchParams(window.location.search);
  const parsedPage = Number.parseInt(search.get('page') ?? '1', 10);
  const applicationType = search.get('application_type');
  const publicationStatus = search.get('publication_status');

  return {
    page: Number.isFinite(parsedPage) && parsedPage > 0 ? parsedPage : 1,
    application_type:
      applicationType === 'agent_flow' || applicationType === 'workflow'
        ? applicationType
        : undefined,
    publication_status:
      publicationStatus === 'published' || publicationStatus === 'unpublished'
        ? publicationStatus
        : undefined,
    created_by: optionalSearchValue(search, 'created_by'),
    tag_id: optionalSearchValue(search, 'tag_id'),
    keyword: optionalSearchValue(search, 'keyword'),
    sort:
      optionalSearchValue(search, 'sort') ?? APPLICATION_MANAGEMENT_DEFAULT_SORT
  };
}

export function pushApplicationManagementRouteState(
  state: ApplicationManagementRouteState
) {
  const search = new URLSearchParams();
  if (state.page > 1) search.set('page', String(state.page));
  if (state.application_type)
    search.set('application_type', state.application_type);
  if (state.publication_status)
    search.set('publication_status', state.publication_status);
  if (state.created_by) search.set('created_by', state.created_by);
  if (state.tag_id) search.set('tag_id', state.tag_id);
  if (state.keyword) search.set('keyword', state.keyword);
  if (state.sort !== APPLICATION_MANAGEMENT_DEFAULT_SORT)
    search.set('sort', state.sort);
  const query = search.toString();
  const nextPath = `${window.location.pathname}${query ? `?${query}` : ''}${window.location.hash}`;

  window.history.pushState({}, '', nextPath);
}
