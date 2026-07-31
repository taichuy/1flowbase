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
  fetchSettingsOfficialPluginCatalog,
  fetchSettingsPluginFamilies,
  settingsOfficialPluginsQueryKey,
  settingsPluginFamiliesQueryKey
} from '../../../api/plugins';
import {
  EMPTY_MODEL_PROVIDER_CATALOG,
  EMPTY_MODEL_PROVIDER_INSTANCES,
  EMPTY_PLUGIN_FAMILIES,
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
  instanceModalState,
  officialSearchQuery
}: {
  drawerState: ModelProviderDrawerState;
  instanceModalState: ModelProviderInstanceModalState;
  officialSearchQuery?: string;
}) {
  const { i18n } = useTranslation();
  const appLocale =
    toAppLocale(i18n.resolvedLanguage) ??
    toAppLocale(i18n.language) ??
    FALLBACK_APP_LOCALE;
  const normalizedOfficialSearchQuery = officialSearchQuery?.trim() ?? '';
  const catalogQuery = useQuery({
    queryKey: [...settingsModelProviderCatalogQueryKey, appLocale],
    queryFn: () => fetchSettingsModelProviderCatalog(appLocale)
  });
  const familiesQuery = useQuery({
    queryKey: [...settingsPluginFamiliesQueryKey, appLocale],
    queryFn: () => fetchSettingsPluginFamilies(appLocale)
  });
  const officialCatalogQuery = useQuery({
    queryKey: [
      ...settingsOfficialPluginsQueryKey,
      appLocale,
      normalizedOfficialSearchQuery
    ],
    queryFn: () =>
      fetchSettingsOfficialPluginCatalog({
        locale: appLocale,
        q: normalizedOfficialSearchQuery || undefined
      })
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
  const families = familiesQuery.data ?? EMPTY_PLUGIN_FAMILIES;
  const officialCatalogEntries = officialCatalogQuery.data?.entries ?? [];
  const providerOptions = optionsQuery.data?.providers;
  const officialSourceMeta = officialCatalogQuery.data
    ? {
        sourceKind: officialCatalogQuery.data.source_kind,
        sourceLabel: officialCatalogQuery.data.source_label,
        registryUrl: officialCatalogQuery.data.registry_url,
        sourceFreshness: officialCatalogQuery.data.source_freshness
      }
    : null;

  const catalogEntriesByInstallationId = useMemo(() => {
    const grouped: Record<string, (typeof catalogEntries)[number]> = {};

    for (const entry of catalogEntries) {
      grouped[entry.installation_id] = entry;
    }

    return grouped;
  }, [catalogEntries]);

  const currentCatalogEntriesByProviderCode = useMemo(() => {
    const grouped: Record<string, (typeof catalogEntries)[number] | null> = {};

    for (const family of families) {
      grouped[family.provider_code] =
        catalogEntriesByInstallationId[family.current_installation_id] ??
        catalogEntries.find(
          (entry) => entry.provider_code === family.provider_code
        ) ??
        null;
    }

    return grouped;
  }, [catalogEntries, catalogEntriesByInstallationId, families]);

  const familiesByProviderCode = useMemo(() => {
    const grouped: Record<string, (typeof families)[number]> = {};

    for (const family of families) {
      grouped[family.provider_code] = family;
    }

    return grouped;
  }, [families]);

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
      ? (currentCatalogEntriesByProviderCode[drawerState.providerCode] ??
        catalogEntries[0] ??
        null)
      : editingInstance
        ? (catalogEntriesByInstallationId[editingInstance.installation_id] ??
          currentCatalogEntriesByProviderCode[editingInstance.provider_code] ??
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
    ? (currentCatalogEntriesByProviderCode[instanceModalState.providerCode] ??
      null)
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
  const providerCount = families.length;
  const officialCount = officialCatalogEntries.length;
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
    },
    {
      key: 'official',
      label: i18nText('settings', 'auto.installable_providers'),
      value: String(officialCount)
    }
  ];

  return {
    catalogQuery,
    familiesQuery,
    officialCatalogQuery,
    instancesQuery,
    optionsQuery,
    mainInstanceQuery,
    instances,
    families,
    officialCatalogEntries,
    officialSourceMeta,
    currentCatalogEntriesByProviderCode,
    familiesByProviderCode,
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
