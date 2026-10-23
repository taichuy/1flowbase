import {
  fetchConsoleApiDocsCatalog,
  type ConsoleApiDocsCatalog,
  type ConsoleApiDocsCategoryOperations,
  type ConsoleApiDocsCategoryOperationsRequest
} from '@1flowbase/api-client';

export {
  fetchConsoleApiDocsCategoryOperations as fetchSettingsApiDocsCategoryOperations,
  fetchConsoleApiOperationSpec as fetchSettingsApiDocsOperationSpec
} from '@1flowbase/api-client';

export type SettingsApiDocsCatalog = ConsoleApiDocsCatalog;
export type SettingsApiDocsCategoryOperations =
  ConsoleApiDocsCategoryOperations;
export type SettingsApiDocsCategoryOperationsRequest =
  ConsoleApiDocsCategoryOperationsRequest;

export const settingsApiDocsCatalogQueryKey = [
  'settings',
  'docs',
  'catalog'
] as const;
export const settingsApiDocsCategoryOperationsQueryKey = (categoryId: string) =>
  ['settings', 'docs', 'category', categoryId, 'operations'] as const;
export const settingsApiDocsOperationSpecQueryKey = (operationId: string) =>
  ['settings', 'docs', 'operation', operationId, 'openapi'] as const;

export function fetchSettingsApiDocsCatalog(): Promise<SettingsApiDocsCatalog> {
  return fetchConsoleApiDocsCatalog();
}
