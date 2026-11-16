import {
  emptyDataFilter,
  type DataFilterGroup,
  type TableBlockConfig
} from '../../../../lib/data-blocks/types';
import { restrictDataFilter } from '../../../../lib/data-blocks/filter-variables';
import { tableQueryScope } from '../table-model';

export interface TableSessionContext {
  ownerKey: string;
  config: TableBlockConfig;
  inputs: Readonly<Record<string, unknown>>;
  variables: Readonly<Record<string, unknown>>;
  idField: string;
}

export interface TableSessionState {
  ownerKey: string;
  pageSizeDefault: number;
  sortDefaultKey: string;
  filterDefaultKey: string;
  filterFieldsKey: string;
  scopeKey: string;
  idField: string;
  page: number;
  pageSize: number;
  sort: TableBlockConfig['sort'];
  filter: DataFilterGroup;
  selected: string[];
}

function filterDefaults(config: TableBlockConfig) {
  return config.actions.find((action) => action.enabled && action.kind === 'filter');
}

export function createTableSession(context: TableSessionContext): TableSessionState {
  const defaults = filterDefaults(context.config);
  const filter = defaults?.default_filter ?? emptyDataFilter();
  const state: TableSessionState = {
    ownerKey: context.ownerKey,
    pageSizeDefault: context.config.page_size,
    sortDefaultKey: JSON.stringify(context.config.sort),
    filterDefaultKey: JSON.stringify(defaults?.default_filter ?? null),
    filterFieldsKey: JSON.stringify(defaults?.filter_fields ?? null),
    scopeKey: '',
    idField: context.idField,
    page: 1,
    pageSize: context.config.page_size,
    sort: context.config.sort,
    filter: defaults?.filter_fields ? restrictDataFilter(filter, defaults.filter_fields) : filter,
    selected: []
  };
  return reconcileTableSession(state, context);
}

/** Reconcile semantic defaults before querying; equal host snapshots retain user interaction. */
export function reconcileTableSession(state: TableSessionState, context: TableSessionContext): TableSessionState {
  const { config } = context;
  const defaults = filterDefaults(config);
  const sortDefaultKey = JSON.stringify(config.sort);
  const filterDefaultKey = JSON.stringify(defaults?.default_filter ?? null);
  const filterFieldsKey = JSON.stringify(defaults?.filter_fields ?? null);
  const ownerChanged = state.ownerKey !== context.ownerKey;
  const pageSizeChanged = ownerChanged || state.pageSizeDefault !== config.page_size;
  const sortChanged = ownerChanged || state.sortDefaultKey !== sortDefaultKey;
  const defaultChanged = ownerChanged || state.filterDefaultKey !== filterDefaultKey;
  const fieldsChanged = ownerChanged || state.filterFieldsKey !== filterFieldsKey;
  let filter = defaultChanged ? defaults?.default_filter ?? emptyDataFilter() : state.filter;
  if (fieldsChanged && defaults?.filter_fields) filter = restrictDataFilter(filter, defaults.filter_fields);
  const scopeKey = JSON.stringify(tableQueryScope(config, filter, context.inputs, context.variables).filter);
  const scopeChanged = ownerChanged || state.scopeKey !== scopeKey || state.idField !== context.idField;
  if (!pageSizeChanged && !sortChanged && !defaultChanged && !fieldsChanged && !scopeChanged) return state;
  return {
    ...state,
    ownerKey: context.ownerKey,
    pageSizeDefault: config.page_size,
    sortDefaultKey,
    filterDefaultKey,
    filterFieldsKey,
    scopeKey,
    idField: context.idField,
    page: pageSizeChanged || sortChanged || scopeChanged ? 1 : state.page,
    pageSize: pageSizeChanged ? config.page_size : state.pageSize,
    sort: sortChanged ? config.sort : state.sort,
    filter,
    selected: scopeChanged ? [] : state.selected
  };
}

export function tableSessionContextKey(state: TableSessionState) {
  return JSON.stringify([state.ownerKey, state.scopeKey, state.idField]);
}

export function tableSessionRequestKey(state: TableSessionState) {
  return JSON.stringify([tableSessionContextKey(state), state.page, state.pageSize, state.sort]);
}

type TableSessionEvent =
  | { type: 'context'; context: TableSessionContext }
  | { type: 'paginate'; page: number; pageSize: number }
  | { type: 'sort'; sort: TableBlockConfig['sort'] }
  | { type: 'filter'; filter: DataFilterGroup; context: TableSessionContext }
  | { type: 'select'; selected: string[] }
  | {
      type: 'delete-completed';
      contextKey: string;
      requestKey: string;
      visibleCount: number;
      deleted: string[];
      failed: string[];
    };

export function tableSessionReducer(state: TableSessionState, event: TableSessionEvent): TableSessionState {
  switch (event.type) {
    case 'context': return reconcileTableSession(state, event.context);
    case 'paginate': return { ...state, page: event.page, pageSize: event.pageSize };
    case 'sort': return { ...state, page: 1, sort: event.sort };
    case 'filter': return reconcileTableSession({ ...state, page: 1, filter: event.filter }, event.context);
    case 'select': return { ...state, selected: event.selected };
    case 'delete-completed': {
      if (event.contextKey !== tableSessionContextKey(state)) return state;
      const sameRequest = event.requestKey === tableSessionRequestKey(state);
      return {
        ...state,
        selected: Array.from(new Set([
          ...state.selected.filter((id) => !event.deleted.includes(id)),
          ...event.failed
        ])),
        page: sameRequest && event.deleted.length && event.visibleCount <= event.deleted.length && state.page > 1
          ? state.page - 1 : state.page
      };
    }
  }
}
