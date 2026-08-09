export {
  archiveConsoleUiTemplate as archiveSettingsUiTemplate,
  createConsoleUiTemplate as createSettingsUiTemplate,
  fetchConsoleUiComponents as fetchSettingsUiComponents,
  fetchConsoleUiTemplates as fetchSettingsUiTemplates,
  publishConsoleUiTemplate as publishSettingsUiTemplate,
  resetConsoleUiTemplateDefault as resetSettingsUiTemplateDefault,
  setConsoleUiTemplateDefault as setSettingsUiTemplateDefault,
  updateConsoleUiComponentContract as updateSettingsUiComponentContract,
  updateConsoleUiComponentState as updateSettingsUiComponentState,
  updateConsoleUiTemplate as updateSettingsUiTemplate,
  type ConsoleUiComponentCandidate as SettingsUiComponentCandidate,
  type ConsoleUiComponentLocator as SettingsUiComponentLocator,
  type ConsoleUiManagedTemplate as SettingsUiManagedTemplate,
  type ConsoleUiOfficialTemplate as SettingsUiOfficialTemplate,
  type ConsoleUiTemplateInput as SettingsUiTemplateInput,
  type UiComponentState as SettingsUiComponentState
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
