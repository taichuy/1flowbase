import type {
  ConsoleAssistantClientTools,
  ConsoleAssistantClientToolExecution
} from '@1flowbase/api-client';
import type { QueryClient } from '@tanstack/react-query';

import {
  getFrontstageAssistantRuntime,
  subscribeFrontstageAssistantRuntime
} from '../../features/frontstage/lib/assistant-frontstage-runtime';

export type AssistantRefreshTarget = () => Promise<void> | void;

export interface AssistantClientSnapshot {
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
  const baseToolIds = ['get_client_context', 'refresh_client_view'] as const;
  const frontstageToolIds = [
    'list_page_blocks',
    'inspect_block_render',
    'search_block_render',
    'read_block_render_fragment',
    'click_block_element',
    'recompile_block'
  ] as const;
  return {
    get toolIds() {
      return getFrontstageAssistantRuntime()
        ? [...baseToolIds, ...frontstageToolIds]
        : [...baseToolIds];
    },
    subscribeCapabilities: subscribeFrontstageAssistantRuntime,
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

      if (
        call.name === 'list_page_blocks' ||
        call.name === 'inspect_block_render' ||
        call.name === 'search_block_render' ||
        call.name === 'read_block_render_fragment' ||
        call.name === 'click_block_element' ||
        call.name === 'recompile_block'
      ) {
        const runtime = getFrontstageAssistantRuntime();
        if (!runtime) {
          return {
            is_error: true,
            result: {
              status: 'unavailable',
              code: 'frontstage_runtime_unmounted'
            }
          };
        }
        return runtime.execute(call.name, call.arguments);
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
