import { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import {
  fetchConsolePluginSettingsModuleAsset,
  type ConsolePluginSettingsPage
} from '@1flowbase/api-client';
import { createFetchedNativeReactModuleRegistry } from '../../../../shared/code-block/fetched-modules/registry';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime/browser';
import { Alert, Spin } from 'antd';
import { useTranslation } from 'react-i18next';

import {
  prepareNativeReactSource,
  type NativeReactSourcePreparationResult
} from '../../../../shared/code-block/native-react-source-preparation';
import { resolveAppLocale } from '../../../../shared/i18n/locales';
import { useAuthStore } from '../../../../state/auth-store';
import {
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../../frontstage/lib/native-adapter/native-trusted-block-react-adapter';
import { createFrontstageUnavailableBlockContext } from '../../../frontstage/lib/native-trusted-block/unavailable-context';
import { evaluateFrontstageReactArtifact } from '../../../frontstage/lib/native-modules/modal/evaluator';
import { createFrontstageNativeReactModuleRegistry } from '../../../frontstage/lib/native-modules/registry';
import { pluginSettingsPageQueryOptions } from '../../api/ui-management';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

function pageRenderEpoch(page: ConsolePluginSettingsPage) {
  return `${page.route_id}:${page.template_id}:${page.applied_plugin_version}:${page.revision}`;
}

type PreparedPage = Extract<NativeReactSourcePreparationResult, { ok: true }>;
const preparedPages = new Map<string, PreparedPage>();
const preparedPageLimit = 16;

function preparationKey(page: ConsolePluginSettingsPage) {
  // The module manifest includes content digests and the declarations used to compile.
  return JSON.stringify([
    pageRenderEpoch(page),
    page.source,
    page.code_modules
  ]);
}

function rememberPreparedPage(key: string, prepared: PreparedPage) {
  preparedPages.delete(key);
  preparedPages.set(key, prepared);
  if (preparedPages.size > preparedPageLimit) {
    const oldest = preparedPages.keys().next();
    if (!oldest.done) preparedPages.delete(oldest.value);
  }
}

export function PluginSettingsPage({ route_id }: { route_id: string }) {
  const { t } = useTranslation('settings');
  const query = useQuery(pluginSettingsPageQueryOptions(route_id));
  const cacheKey = query.data ? preparationKey(query.data) : null;
  return (
    <SettingsSectionSurface heightMode="fill">
      {query.isError ? (
        <Alert
          type="error"
          showIcon
          title={t('auto.plugin_settings_page_failed')}
        />
      ) : query.data && cacheKey !== null ? (
        <PublishedPluginSettingsPage
          key={cacheKey}
          cacheKey={cacheKey}
          page={query.data}
        />
      ) : (
        <Spin />
      )}
    </SettingsSectionSurface>
  );
}

function PublishedPluginSettingsPage({
  page,
  cacheKey
}: {
  cacheKey: string;
  page: ConsolePluginSettingsPage;
}) {
  const { t, i18n } = useTranslation('settings');
  const locale = resolveAppLocale(i18n.resolvedLanguage ?? i18n.language);
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const [prepared, setPrepared] = useState<PreparedPage | null>(
    () => preparedPages.get(cacheKey) ?? null
  );
  const [failed, setFailed] = useState(false);
  const renderEpoch = pageRenderEpoch(page);
  const plan = useMemo<NativeTrustedBlockPreparePlan>(
    () => ({
      runtime: 'native_trusted_block',
      blockId: page.route_id,
      entry: 'default',
      source: page.source,
      normalizedSource: page.source.trim(),
      props: {},
      requiredPermissions: ['ui_block.javascript.native']
    }),
    [page.route_id, page.source]
  );
  const context = useMemo(
    () => ({
      // The page-read authorization grants rendering only. Neither editable
      // source nor feature ownership can grant API operations to this context.
      ...createFrontstageUnavailableBlockContext(plan),
      currentUser: actor
        ? Object.freeze({
            id: actor.id,
            displayName:
              me?.nickname?.trim() || me?.name?.trim() || actor.account,

            policy_fingerprint: me?.policy_fingerprint,
            isRoot: actor.effective_display_role === 'root'
          })
        : null,
      workspace: { id: actor?.current_workspace_id ?? '' },
      ui: { locale }
    }),
    [actor, me, plan, locale]
  );

  useEffect(() => {
    const cached = preparedPages.get(cacheKey);
    if (cached) {
      rememberPreparedPage(cacheKey, cached);
      setPrepared(cached);
      return;
    }
    let current = true;
    let loader: ReturnType<typeof createFetchedNativeReactModuleRegistry>;
    try {
      loader = createFetchedNativeReactModuleRegistry({
        modules: page.code_modules,
        hostRegistryFactory: (registrations) =>
          createFrontstageNativeReactModuleRegistry({}, registrations),
        fetchAsset: (asset, signal) =>
          fetchConsolePluginSettingsModuleAsset(
            page.route_id,
            page.applied_plugin_version,
            asset.sha256,
            signal
          )
      });
    } catch {
      setFailed(true);
      return;
    }
    void prepareNativeReactSource({
      frozenSource: page.source,
      requestId: renderEpoch,
      registryFactory: () => loader.registry,
      evaluateArtifact: evaluateFrontstageReactArtifact
    })
      .then((result) => {
        if (!current) return;
        if (!result.ok) {
          setFailed(true);
          return;
        }
        // Resolved components and stylesheet bytes are independent of the loader's
        // in-flight requests and temporary import URLs after preparation completes.
        rememberPreparedPage(cacheKey, result);
        setPrepared(result);
      })
      .catch(() => {
        if (current) setFailed(true);
      })
      .finally(() => loader.dispose());
    return () => {
      current = false;
      loader.dispose();
    };
  }, [page, cacheKey, renderEpoch]);

  if (failed) {
    return (
      <Alert
        type="error"
        showIcon
        title={t('auto.plugin_settings_page_failed')}
      />
    );
  }
  return (
    <>
      <div ref={setRoot} data-testid="plugin-settings-page" />
      {!prepared ? <Spin /> : null}
      {prepared && root ? (
        <FrontstageNativeTrustedBlockPortalHost
          root={root}
          renderEpoch={renderEpoch}
          plan={plan}
          component={
            prepared.component as FrontstageNativeTrustedBlockReactComponent
          }
          ctx={context}
          moduleAssets={prepared.moduleAssets}
          moduleSources={prepared.artifact.program.injectedModules.map(
            ({ source }) => source
          )}
          onRuntimeError={() => setFailed(true)}
        />
      ) : null}
    </>
  );
}
