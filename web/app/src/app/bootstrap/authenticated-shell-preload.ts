import type { QueryClient } from '@tanstack/react-query';
import type { ComponentType } from 'react';

import {
  prepareApplicationI18n,
  prepareShellTopbarI18n
} from '../../shared/i18n/app-i18n';
import { resolveAuthenticatedLocale } from '../../shared/i18n/initial-locale';
import { resolveAppLocale, type AppLocale } from '../../shared/i18n/locales';
import { useAuthStore } from '../../state/auth-store';

type AuthenticatedRuntimeComponent = ComponentType<{
  queryClient: QueryClient;
}>;
type AppShellFrameComponent =
  (typeof import('../../app-shell/AppShellFrame'))['AppShellFrame'];
type FrontstageWorkspacePageComponent =
  (typeof import('../../features/frontstage/pages/FrontstageWorkspacePage'))['FrontstageWorkspacePage'];

let resolvedAuthenticatedRuntime: AuthenticatedRuntimeComponent | null = null;
let resolvedAppShellFrame: ComponentType<
  Parameters<AppShellFrameComponent>[0]
> | null = null;
let resolvedFrontstageWorkspacePage: ComponentType<
  Parameters<FrontstageWorkspacePageComponent>[0]
> | null = null;

export function getResolvedAuthenticatedRuntime(): AuthenticatedRuntimeComponent | null {
  return resolvedAuthenticatedRuntime;
}

export function getResolvedAppShellFrame(): ComponentType<
  Parameters<AppShellFrameComponent>[0]
> | null {
  return resolvedAppShellFrame;
}

export function getResolvedFrontstageWorkspacePage(): ComponentType<
  Parameters<FrontstageWorkspacePageComponent>[0]
> | null {
  return resolvedFrontstageWorkspacePage;
}

export function loadAppShellFrameModule() {
  return import('../../app-shell/AppShellFrame').then((module) => {
    if (!import.meta.env.VITEST) {
      resolvedAppShellFrame = module.AppShellFrame;
    }
    return module;
  });
}

export function loadFrontstageWorkspacePageModule() {
  const locale = resolveAppLocale(
    resolveAuthenticatedLocale(useAuthStore.getState().me)
  );
  return Promise.all([
    import('../../features/frontstage/pages/FrontstageWorkspacePage'),
    prepareApplicationI18n(locale, '/')
  ]).then(([module]) => {
    if (!import.meta.env.VITEST) {
      resolvedFrontstageWorkspacePage = module.FrontstageWorkspacePage;
    }
    return module;
  });
}

export async function preloadAuthenticatedStartupRuntime(
  locale: AppLocale,
  pathname: string
): Promise<AuthenticatedRuntimeComponent> {
  const [runtimeModule] = await Promise.all([
    import('../AuthenticatedAppRuntime'),
    prepareShellTopbarI18n(locale, pathname),
    loadAppShellFrameModule()
  ]);
  if (!import.meta.env.VITEST) {
    resolvedAuthenticatedRuntime = runtimeModule.AuthenticatedAppRuntime;
  }
  return runtimeModule.AuthenticatedAppRuntime;
}
