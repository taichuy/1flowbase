import { useMemo } from 'react';

import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';

import {
  fetchSettingsModelProviderCatalog,
  fetchSettingsModelProviderInstances,
  fetchSettingsModelProviderMainInstance,
  fetchSettingsModelProviderModels,
  fetchSettingsModelProviderOptions,
  settingsModelProviderCatalogQueryKey,
  settingsModelProviderInstancesQueryKey,
  settingsModelProviderOptionsQueryKey,
  settingsModelProviderModelsQueryKey,
  type SettingsModelProviderOptions
} from '../../../api/model-providers';
import {
  EMPTY_MODEL_PROVIDER_CATALOG,
  EMPTY_MODEL_PROVIDER_INSTANCES,
  IDLE_MODEL_PROVIDER_MODELS_QUERY_KEY,
  MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX,
  type ModelProviderDrawerState,
  type ModelProviderInstanceModalState
} from './shared';
import { i18nText } from '../../../../../shared/i18n/text';
import {
  FALLBACK_APP_LOCALE,
  toAppLocale
} from '../../../../../shared/i18n/locales';

export function useModelProviderData({
  drawerState,
  instanceModalState
}: {
  drawerState: ModelProviderDrawerState;
  instanceModalState: ModelProviderInstanceModalState;
}) {
  const { i18n } = useTranslation();
  const appLocale =
    toAppLocale(i18n.resolvedLanguage) ??
    toAppLocale(i18n.language) ??
    FALLBACK_APP_LOCALE;
  const catalogQuery = useQuery({
    queryKey: [...settingsModelProviderCatalogQueryKey, appLocale],
    queryFn: () => fetchSettingsModelProviderCatalog(appLocale)
  });
  const instancesQuery = useQuery({
    queryKey: settingsModelProviderInstancesQueryKey,
    queryFn: fetchSettingsModelProviderInstances
  });
  const optionsQuery = useQuery({
    queryKey: settingsModelProviderOptionsQueryKey,
    queryFn: fetchSettingsModelProviderOptions
  });

  const instances = instancesQuery.data ?? EMPTY_MODEL_PROVIDER_INSTANCES;
  const catalogEntries = catalogQuery.data ?? EMPTY_MODEL_PROVIDER_CATALOG;
  const providerOptions = optionsQuery.data?.providers;

  const catalogEntriesByInstallationId = useMemo(() => {
    const grouped: Record<string, (typeof catalogEntries)[number]> = {};

    for (const entry of catalogEntries) {
      grouped[entry.installation_id] = entry;
    }

    return grouped;
  }, [catalogEntries]);

  const catalogEntriesByProviderCode = useMemo(() => {
    const grouped: Record<string, (typeof catalogEntries)[number] | null> = {};

    for (const entry of catalogEntries) {
      grouped[entry.provider_code] = entry;
    }

    return grouped;
  }, [catalogEntries]);

  const instancesByProviderCode = useMemo(() => {
    const grouped: Record<string, typeof instances> = {};

    for (const instance of instances) {
      grouped[instance.provider_code] ??= [];
      grouped[instance.provider_code]!.push(instance);
    }

    return grouped;
  }, [instances]);

  const providerOptionsByProviderCode = useMemo(() => {
    const grouped: Record<
      string,
      SettingsModelProviderOptions['providers'][number]
    > = {};

    for (const provider of providerOptions ?? []) {
      grouped[provider.provider_code] = provider;
    }

    return grouped;
  }, [providerOptions]);

  const editingInstance =
    drawerState?.mode === 'edit'
      ? (instances.find((instance) => instance.id === drawerState.instanceId) ??
        null)
      : null;

  const drawerCatalogEntry =
    drawerState?.mode === 'create'
      ? (catalogEntriesByProviderCode[drawerState.providerCode] ??
        catalogEntries[0] ??
        null)
      : editingInstance
        ? (catalogEntriesByInstallationId[editingInstance.installation_id] ??
          catalogEntriesByProviderCode[editingInstance.provider_code] ??
          null)
        : null;

  const modalInstances = useMemo(
    () =>
      instanceModalState
        ? (instancesByProviderCode[instanceModalState.providerCode] ??
          EMPTY_MODEL_PROVIDER_INSTANCES)
        : EMPTY_MODEL_PROVIDER_INSTANCES,
    [instanceModalState, instancesByProviderCode]
  );

  const modalCatalogEntry = instanceModalState
    ? (catalogEntriesByProviderCode[instanceModalState.providerCode] ?? null)
    : null;

  const modalProviderOption = instanceModalState
    ? (providerOptionsByProviderCode[instanceModalState.providerCode] ?? null)
    : null;

  const mainInstanceProviderCode =
    drawerState?.mode === 'create'
      ? drawerState.providerCode
      : (instanceModalState?.providerCode ?? null);

  const mainInstanceQuery = useQuery({
    queryKey: mainInstanceProviderCode
      ? [
          ...MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX,
          mainInstanceProviderCode
        ]
      : [...MODEL_PROVIDER_MAIN_INSTANCE_QUERY_KEY_PREFIX, 'idle'],
    queryFn: () =>
      fetchSettingsModelProviderMainInstance(mainInstanceProviderCode!),
    enabled: Boolean(mainInstanceProviderCode)
  });

  const drawerDefaultIncludedInMain =
    drawerState?.mode === 'create'
      ? (mainInstanceQuery.data?.auto_include_new_instances ??
        providerOptionsByProviderCode[drawerState.providerCode]?.main_instance
          .auto_include_new_instances ??
        false)
      : (editingInstance?.included_in_main ?? false);

  const editingModelsQuery = useQuery({
    queryKey: editingInstance
      ? settingsModelProviderModelsQueryKey(editingInstance.id)
      : IDLE_MODEL_PROVIDER_MODELS_QUERY_KEY,
    queryFn: () => fetchSettingsModelProviderModels(editingInstance!.id),
    enabled: Boolean(editingInstance)
  });

  const readyCount = instances.filter(
    (instance) => instance.status === 'ready'
  ).length;
  const invalidCount = instances.filter(
    (instance) => instance.status === 'invalid'
  ).length;
  const providerCount = catalogEntries.length;
  const overviewRows = [
    {
      key: 'providers',
      label: i18nText('settings', 'auto.provider_installed'),
      value: String(providerCount)
    },
    {
      key: 'ready',
      label: i18nText('settings', 'auto.available_instances'),
      value: String(readyCount)
    },
    {
      key: 'invalid',
      label: i18nText('settings', 'auto.exception_instance'),
      value: String(invalidCount)
    }
  ];

  return {
    catalogQuery,
    instancesQuery,
    optionsQuery,
    mainInstanceQuery,
    instances,
    catalogEntries,
    instancesByProviderCode,
    providerOptionsByProviderCode,
    editingInstance,
    editingModelCatalog: editingModelsQuery.data ?? null,
    drawerCatalogEntry,
    drawerDefaultIncludedInMain,
    modalInstances,
    modalCatalogEntry,
    modalProviderOption,
    overviewRows
  };
}
