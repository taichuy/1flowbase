import type {
  ConsoleAssistantClientTools,
  ConsoleAssistantClientToolExecution
} from '@1flowbase/api-client';
import { type QueryClient, useQueryClient } from '@tanstack/react-query';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  type PropsWithChildren
} from 'react';
import { useTranslation } from 'react-i18next';

import { APP_ROUTES } from '../routes/route-config';
import { useAuthStore } from '../state/auth-store';

type AssistantRefreshTarget = () => Promise<void> | void;

interface AssistantClientSnapshot {
  href: string;
  route_id: string;
  page_title: string;
  locale: string;
  workspace_id: string | null;
  viewport: { width: number; height: number };
}

interface AssistantClientToolFactoryInput {
  queryClient: QueryClient;
  refreshTargets: Map<string, AssistantRefreshTarget>;
  snapshot(): AssistantClientSnapshot;
}

export function createAssistantClientTools({
  queryClient,
  refreshTargets,
  snapshot
}: AssistantClientToolFactoryInput): ConsoleAssistantClientTools {
  return {
    toolIds: ['get_client_context', 'refresh_client_view'],
    async execute(call): Promise<ConsoleAssistantClientToolExecution> {
      if (call.name === 'get_client_context') {
        const current = snapshot();
        return {
          is_error: false,
          result: {
            url: current.href,
            route_id: current.route_id,
            page_title: current.page_title,
            locale: current.locale,
            workspace_id: current.workspace_id,
            viewport: current.viewport
          }
        };
      }

      const scope = call.arguments.scope;
      const targetId = call.arguments.target_id;
      if (scope === 'page' && targetId === 'current') {
        await queryClient.invalidateQueries({ refetchType: 'active' });
        return {
          is_error: false,
          result: { status: 'refreshed', scope, target_id: targetId }
        };
      }
      if (scope === 'section' && typeof targetId === 'string') {
        const refresh = refreshTargets.get(targetId);
        if (!refresh) {
          return {
            is_error: false,
            result: { status: 'unavailable', scope, target_id: targetId }
          };
        }
        try {
          await refresh();
          return {
            is_error: false,
            result: { status: 'refreshed', scope, target_id: targetId }
          };
        } catch {
          return {
            is_error: true,
            result: { status: 'failed', scope, target_id: targetId }
          };
        }
      }
      return {
        is_error: true,
        result: { status: 'failed', code: 'invalid_refresh_target' }
      };
    }
  };
}

interface AssistantClientToolContextValue {
  clientTools: ConsoleAssistantClientTools;
  registerRefreshTarget(
    targetId: string,
    refresh: AssistantRefreshTarget
  ): () => void;
}

const AssistantClientToolContext =
  createContext<AssistantClientToolContextValue | null>(null);

export function AssistantClientToolProvider({ children }: PropsWithChildren) {
  const queryClient = useQueryClient();
  const { i18n } = useTranslation();
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id ?? null
  );
  const refreshTargets = useRef(new Map<string, AssistantRefreshTarget>());
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
        refreshTargets: refreshTargets.current,
        snapshot
      }),
    [queryClient, snapshot]
  );
  const registerRefreshTarget = useCallback(
    (targetId: string, refresh: AssistantRefreshTarget) => {
      refreshTargets.current.set(targetId, refresh);
      return () => {
        if (refreshTargets.current.get(targetId) === refresh) {
          refreshTargets.current.delete(targetId);
        }
      };
    },
    []
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

export function useAssistantClientTools() {
  const value = useContext(AssistantClientToolContext);
  if (!value) {
    throw new Error('Assistant client tools require AppShell ownership');
  }
  return value.clientTools;
}

export function useAssistantRefreshTarget(
  targetId: string,
  refresh: AssistantRefreshTarget
) {
  const value = useContext(AssistantClientToolContext);
  useEffect(() => {
    if (!value) {
      return;
    }
    return value.registerRefreshTarget(targetId, refresh);
  }, [refresh, targetId, value]);
}
