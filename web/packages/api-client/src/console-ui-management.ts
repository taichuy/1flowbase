import { apiFetch } from './transport';

export type UiCodeTemplateLanguage = 'jsx' | 'tsx';
export type UiComponentState = 'inherit' | 'published' | 'hidden';

export interface ConsoleUiTemplateRevision {
  revision: number;
  source: string;
  language: UiCodeTemplateLanguage;
  is_published: boolean;
}
export interface ConsoleUiManagedTemplate {
  id: string;
  provider_code: string;
  contribution_code: string;
  name: string;
  latest_revision: ConsoleUiTemplateRevision;
  published_revision: ConsoleUiTemplateRevision | null;
  is_default: boolean;
  is_archived: boolean;
}
export interface ConsoleUiOfficialTemplate {
  provider_code: string;
  contribution_code: string;
  title: string;
  source: string;
  language: UiCodeTemplateLanguage;
  version: string;
  is_default: boolean;
}
export interface ConsoleUiTemplateList {
  official: ConsoleUiOfficialTemplate[];
  managed: ConsoleUiManagedTemplate[];
}
export interface ConsoleUiComponentLocator {
  provider_code: string;
  contribution_code: string;
  module_source: string;
  export_name: string;
}
export interface ConsoleUiComponentUpstream {
  package: string;
  component: string;
  version: string;
}
export interface ConsoleUiComponentProp {
  name: string;
  type: string;
  required: boolean;
  description: string;
}
export interface ConsoleUiComponentExample {
  title: string;
  code: string;
}
export interface ConsoleUiComponentContract {
  component_code: string;
  export_name: string;
  upstream: ConsoleUiComponentUpstream | null;
  description: string;
  props: ConsoleUiComponentProp[];
  limitations: string[];
  examples: ConsoleUiComponentExample[];
  insert_snippet: string;
}
export interface ConsoleUiComponentCandidate extends ConsoleUiComponentLocator {
  module_version: string;
  state: UiComponentState;
  official_contract: ConsoleUiComponentContract | null;
  latest_contract: ConsoleUiComponentContract | null;
  published_contract: ConsoleUiComponentContract | null;
  latest_revision: number | null;
  published_revision: number | null;
}
export interface CreateConsoleUiTemplateInput extends ConsoleUiComponentLocator {
  name: never;
}
export interface ConsoleUiTemplateInput {
  provider_code: string;
  contribution_code: string;
  name: string;
  source: string;
  language: UiCodeTemplateLanguage;
}

const root = '/api/console/settings/ui-management';
export const fetchConsoleUiTemplates = (
  includeArchived = false,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiTemplateList>({
    path: `${root}/templates?include_archived=${includeArchived}`,
    baseUrl
  });
export const createConsoleUiTemplate = (
  input: ConsoleUiTemplateInput,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates`,
    method: 'POST',
    body: input,
    csrfToken,
    baseUrl
  });
export const updateConsoleUiTemplate = (
  id: string,
  input: Pick<ConsoleUiTemplateInput, 'name' | 'source' | 'language'>,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates/${id}`,
    method: 'PUT',
    body: input,
    csrfToken,
    baseUrl
  });
export const publishConsoleUiTemplate = (
  id: string,
  revision: number,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates/${id}/publish`,
    method: 'POST',
    body: { revision },
    csrfToken,
    baseUrl
  });
export const setConsoleUiTemplateDefault = (
  id: string,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<void>({
    path: `${root}/templates/${id}/default`,
    method: 'PUT',
    csrfToken,
    baseUrl
  });
export const resetConsoleUiTemplateDefault = (
  locator: Pick<ConsoleUiTemplateInput, 'provider_code' | 'contribution_code'>,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<void>({
    path: `${root}/templates/default`,
    method: 'DELETE',
    body: locator,
    csrfToken,
    baseUrl
  });
export const archiveConsoleUiTemplate = (
  id: string,
  archived: boolean,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiManagedTemplate>({
    path: `${root}/templates/${id}/archive`,
    method: 'PUT',
    body: { archived },
    csrfToken,
    baseUrl
  });
export const fetchConsoleUiComponents = (baseUrl?: string) =>
  apiFetch<ConsoleUiComponentCandidate[]>({
    path: `${root}/components`,
    baseUrl
  });
export const updateConsoleUiComponentContract = (
  locator: ConsoleUiComponentLocator,
  contract: ConsoleUiComponentContract,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiComponentCandidate>({
    path: `${root}/components/contract`,
    method: 'PUT',
    body: { ...locator, contract },
    csrfToken,
    baseUrl
  });
export const updateConsoleUiComponentState = (
  locator: ConsoleUiComponentLocator,
  state: UiComponentState,
  csrfToken: string,
  baseUrl?: string
) =>
  apiFetch<ConsoleUiComponentCandidate>({
    path: `${root}/components/state`,
    method: 'PUT',
    body: { ...locator, state },
    csrfToken,
    baseUrl
  });
