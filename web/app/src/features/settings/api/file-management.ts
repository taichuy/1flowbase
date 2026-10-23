import {
  fetchConsoleFileStorages,
  fetchConsoleFileTables,
  type ConsoleFileStorage,
  type ConsoleFileTable,
  type CreateConsoleFileStorageInput,
  type CreateConsoleFileTableInput,
  type UpdateConsoleFileStorageInput,
  type UpdateConsoleFileTableBindingInput
} from '@1flowbase/api-client';

export {
  createConsoleFileStorage as createSettingsFileStorage,
  createConsoleFileTable as createSettingsFileTable,
  updateConsoleFileTableBinding as updateSettingsFileTableBinding,
  updateConsoleFileStorage as updateSettingsFileStorage,
  deleteConsoleFileStorage as deleteSettingsFileStorage,
  deleteConsoleFileTable as deleteSettingsFileTable
} from '@1flowbase/api-client';

export type SettingsFileStorage = ConsoleFileStorage;
export type SettingsFileTable = ConsoleFileTable;
export type CreateSettingsFileStorageInput = CreateConsoleFileStorageInput;
export type CreateSettingsFileTableInput = CreateConsoleFileTableInput;
export type UpdateSettingsFileTableBindingInput =
  UpdateConsoleFileTableBindingInput;

export type UpdateSettingsFileStorageInput = UpdateConsoleFileStorageInput;

export const settingsFileStoragesQueryKey = [
  'settings',
  'files',
  'storages'
] as const;

export const settingsFileTablesQueryKey = [
  'settings',
  'files',
  'tables'
] as const;

export function fetchSettingsFileStorages() {
  return fetchConsoleFileStorages();
}

export function fetchSettingsFileTables() {
  return fetchConsoleFileTables();
}
