import { apiFetch } from '../../transport';
import type {
  ApiEventOperationDetails,
  ApiEventOperationSummary,
  CreateEventDefinitionInput,
  UpdateEventDefinitionInput,
  EventCatalogEntry,
  EventCatalogQuery,
  EventDefinitionRecord,
  EventRuntimeSnapshot
} from './types';

const BASE_PATH = '/api/console/events';
export const listEventCatalog = (
  baseUrl?: string,
  query?: EventCatalogQuery
) => {
  const search = query ? `?${new URLSearchParams({ ...query })}` : '';
  return apiFetch<EventCatalogEntry[]>({
    path: `${BASE_PATH}${search}`,
    baseUrl
  });
};
export const getEventRuntime = (baseUrl?: string) =>
  apiFetch<EventRuntimeSnapshot>({ path: `${BASE_PATH}/runtime`, baseUrl });
export const createEventDefinition = (
  body: CreateEventDefinitionInput,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<EventDefinitionRecord>({
    path: BASE_PATH,
    method: 'POST',
    body,
    csrfToken,
    baseUrl
  });
export const updateEventDefinition = (
  event_definition_id: string,
  body: UpdateEventDefinitionInput,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<EventDefinitionRecord>({
    path: `${BASE_PATH}/${encodeURIComponent(event_definition_id)}`,
    method: 'PUT',
    body,
    csrfToken,
    baseUrl
  });

export const listApiEventOperations = (baseUrl?: string) =>
  apiFetch<ApiEventOperationSummary[]>({
    path: `${BASE_PATH}/api-operations`,
    baseUrl
  });
export const getApiEventOperation = (operation_id: string, baseUrl?: string) =>
  apiFetch<ApiEventOperationDetails>({
    path: `${BASE_PATH}/api-operations/${encodeURIComponent(operation_id)}`,
    baseUrl
  });
