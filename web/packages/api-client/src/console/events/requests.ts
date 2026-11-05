import { apiFetch } from '../../transport';
import type {
  CreateEventDefinitionInput,
  UpdateEventDefinitionInput,
  EventCatalogEntry,
  EventDefinitionRecord,
  EventRuntimeSnapshot
} from './types';

const BASE_PATH = '/api/console/events';
export const listEventCatalog = (baseUrl?: string) =>
  apiFetch<EventCatalogEntry[]>({ path: BASE_PATH, baseUrl });
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
