export {
  archiveConsoleUiTemplate as archiveSettingsUiTemplate,
  createConsoleUiComponent as createSettingsUiComponent,
  createConsoleUiTemplate as createSettingsUiTemplate,
  deleteConsoleUiComponent as deleteSettingsUiComponent,
  downloadConsoleUiCatalogComponent as downloadSettingsUiCatalogComponent,
  fetchConsoleUiCatalogPage as fetchSettingsUiCatalogPage,
  fetchConsoleUiCatalogIndex as fetchSettingsUiCatalogIndex,
  fetchConsoleUiCatalogUpdateStatus as fetchSettingsUiCatalogUpdateStatus,
  fetchConsoleUiComponent as fetchSettingsUiComponent,
  fetchConsoleUiComponents as fetchSettingsUiComponents,
  fetchConsoleUiTemplates as fetchSettingsUiTemplates,
  publishConsoleUiTemplate as publishSettingsUiTemplate,
  resetConsoleUiTemplateDefault as resetSettingsUiTemplateDefault,
  setConsoleUiTemplateDefault as setSettingsUiTemplateDefault,
  searchConsoleUiCatalog as searchSettingsUiCatalog,
  syncConsoleUiCatalogGroup as syncSettingsUiCatalogGroup,
  updateConsoleUiComponent as updateSettingsUiComponent,
  updateConsoleUiTemplate as updateSettingsUiTemplate,
  type ConsoleUiComponentRecord as SettingsUiComponentRecord,
  type ConsoleUiCatalogComponent as SettingsUiCatalogComponent,
  type ConsoleUiCatalogIndex as SettingsUiCatalogIndex,
  type ConsoleUiCatalogPage as SettingsUiCatalogPage,
  type ConsoleUiCatalogSearchResult as SettingsUiCatalogSearchResult,
  type ConsoleUiCatalogUpdateStatus as SettingsUiCatalogUpdateStatus,
  type CreateConsoleUiComponentInput as CreateSettingsUiComponentInput,
  type UpdateConsoleUiComponentInput as UpdateSettingsUiComponentInput,
  type ConsoleUiManagedTemplate as SettingsUiManagedTemplate,
  type ConsoleUiOfficialTemplate as SettingsUiOfficialTemplate,
  type ConsoleUiTemplateInput as SettingsUiTemplateInput
} from '@1flowbase/api-client';

export const settingsUiTemplatesQueryKey = [
  'settings',
  'ui-management',
  'templates'
] as const;
export const settingsUiComponentsQueryKey = [
  'settings',
  'ui-management',
  'components'
] as const;
