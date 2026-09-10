import { useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import type { ConsolePluginSettingsPage } from '@1flowbase/api-client';
import type { NativeTrustedBlockPreparePlan } from '@1flowbase/page-runtime/browser';
import { Alert, Spin } from 'antd';
import { useTranslation } from 'react-i18next';

import {
  prepareNativeReactSource,
  type NativeReactSourcePreparationResult
} from '../../../../shared/code-block/native-react-source-preparation';
import { useAuthStore } from '../../../../state/auth-store';
import {
  createFrontstageUnavailableBlockContext,
  FrontstageNativeTrustedBlockPortalHost,
  type FrontstageNativeTrustedBlockReactComponent
} from '../../../frontstage/lib/native-trusted-block-react-adapter';
import { createFrontstageNativeReactModuleRegistry } from '../../../frontstage/lib/native-modules/registry';
import { pluginSettingsPageQueryOptions } from '../../api/ui-management';
import { SettingsSectionSurface } from '../../components/SettingsSectionSurface';

export function PluginSettingsPage({ route_id }: { route_id: string }) {
  const { t } = useTranslation('settings');
  const query = useQuery(pluginSettingsPageQueryOptions(route_id));
  return (
    <SettingsSectionSurface>
      {query.isError ? (
        <Alert
          type="error"
          showIcon
          title={t('auto.plugin_settings_page_failed')}
        />
      ) : query.data ? (
        <PublishedPluginSettingsPage
          key={`${query.data.route_id}:${query.data.template_id}:${query.data.revision}`}
          page={query.data}
        />
      ) : (
        <Spin />
      )}
    </SettingsSectionSurface>
  );
}

function PublishedPluginSettingsPage({
  page
}: {
  page: ConsolePluginSettingsPage;
}) {
  const { t } = useTranslation('settings');
  const actor = useAuthStore((state) => state.actor);
  const me = useAuthStore((state) => state.me);
  const [root, setRoot] = useState<HTMLDivElement | null>(null);
  const [prepared, setPrepared] =
    useState<NativeReactSourcePreparationResult | null>(null);
  const [failed, setFailed] = useState(false);
  const renderEpoch = `${page.route_id}:${page.template_id}:${page.revision}`;
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
        ? {
            id: actor.id,
            displayName:
              me?.nickname?.trim() || me?.name?.trim() || actor.account
          }
        : null,
      workspace: { id: actor?.current_workspace_id ?? '' },
      ui: { locale: me?.preferred_locale ?? undefined }
    }),
    [actor, me, plan]
  );

  useEffect(() => {
    let current = true;
    setPrepared(null);
    setFailed(false);
    void prepareNativeReactSource({
      frozenSource: page.source,
      requestId: renderEpoch,
      registryFactory: createFrontstageNativeReactModuleRegistry
    })
      .then((result) => {
        if (current) setPrepared(result);
      })
      .catch(() => {
        if (current) setFailed(true);
      });
    return () => {
      current = false;
    };
  }, [page.source, renderEpoch]);

  if (failed || prepared?.ok === false) {
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
      {prepared?.ok && root ? (
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
