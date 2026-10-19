import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useMemo, useRef, type PropsWithChildren } from 'react';
import { useTranslation } from 'react-i18next';

import { APP_ROUTES } from '../routes/route-config';
import { useAuthStore } from '../state/auth-store';
import { AssistantClientToolContext } from './assistant-client-tools/context';
import {
  createAssistantClientTools,
  type AssistantClientSnapshot,
  type AssistantRefreshTarget
} from './assistant-client-tools/factory';

export function AssistantClientToolProvider({ children }: PropsWithChildren) {
  const queryClient = useQueryClient();
  const { i18n } = useTranslation();
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );
  const refreshTargetsRef = useRef<Map<string, AssistantRefreshTarget> | null>(
    null
  );
  if (refreshTargetsRef.current === null) {
    refreshTargetsRef.current = new Map<string, AssistantRefreshTarget>();
  }
  const refreshTargets = refreshTargetsRef.current;
  const snapshot = useCallback((): AssistantClientSnapshot => {
    const pathname = window.location.pathname;
    const routeId =
      APP_ROUTES.find((route) =>
        route.selectedMatchers.some((matches) => matches(pathname))
      )?.id ?? 'home';
    return {
      href: window.location.href,
      route_id: routeId,
      page_title: document.title,
      locale: i18n.language,
      workspace_id: workspaceId,
      viewport: { width: window.innerWidth, height: window.innerHeight }
    };
  }, [i18n.language, workspaceId]);
  const clientTools = useMemo(
    () =>
      createAssistantClientTools({
        queryClient,
        refreshTargets,
        snapshot
      }),
    [queryClient, refreshTargets, snapshot]
  );
  const registerRefreshTarget = useCallback(
    (targetId: string, refresh: AssistantRefreshTarget) => {
      refreshTargets.set(targetId, refresh);
      return () => {
        if (refreshTargets.get(targetId) === refresh) {
          refreshTargets.delete(targetId);
        }
      };
    },
    [refreshTargets]
  );
  const value = useMemo(
    () => ({ clientTools, registerRefreshTarget }),
    [clientTools, registerRefreshTarget]
  );

  return (
    <AssistantClientToolContext.Provider value={value}>
      {children}
    </AssistantClientToolContext.Provider>
  );
}
