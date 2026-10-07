import { useMemo } from 'react';
import {
  createMemoryHistory,
  createRootRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';
import type {
  ConsoleApplicationCatalog,
  ConsoleApplicationCollector
} from '@1flowbase/api-client';
import { ApplicationCollectorPage } from '../../features/applications/pages/ApplicationCollectorPage';
import { CollectorInstallation } from '../../features/applications/components/collector/CollectorInstallation';

const applicationId = '1ab933c4-2f52-405c-bb1d-86d9e998c06c';
const collector: ConsoleApplicationCollector = {
  collector_code: 'codex-logs-collector',
  source_client: 'codex',
  display_name: 'Codex',
  description: '采集本地 Codex 对话日志并上传到当前应用。',
  version: '0.1.0',
  execution_target: 'client',
  documentation_url:
    'https://github.com/taichuy/1flowbase-official-plugins/blob/main/runtime-extensions/@taichuy/codex-logs-collector/README.md',
  shell_installer_url:
    'https://github.com/taichuy/1flowbase-official-plugins/releases/download/codex-logs-collector-v0.1.0/install.sh',
  powershell_installer_url:
    'https://github.com/taichuy/1flowbase-official-plugins/releases/download/codex-logs-collector-v0.1.0/install.ps1'
};
const catalog: ConsoleApplicationCatalog = {
  types: [{ value: 'agent_logs', label: 'Agent Logs' }],
  workflow_triggers: [],
  tags: [],
  collectors: [collector]
};

export function seedStyleBoundaryCollectorFetch() {
  const originalFetch = globalThis.fetch.bind(globalThis);
  globalThis.fetch = async (input, init) => {
    const url = new URL(
      typeof input === 'string'
        ? input
        : input instanceof URL
          ? input.href
          : input.url,
      window.location.origin
    );
    if (url.pathname === '/api/console/applications/catalog') {
      return new Response(JSON.stringify({ data: catalog, meta: null }), {
        status: 200,
        headers: { 'content-type': 'application/json' }
      });
    }
    return originalFetch(input, init);
  };
}

export function CollectorStyleBoundaryScene({
  installation
}: {
  installation: boolean;
}) {
  const router = useMemo(() => {
    const root = createRootRoute({
      component: () =>
        installation ? (
          <div className="application-collector">
            <CollectorInstallation
              collector={collector}
              applicationId={applicationId}
              endpoint="https://console.example.com/api/logs/v1/events"
              onBack={() => undefined}
            />
          </div>
        ) : (
          <ApplicationCollectorPage applicationId={applicationId} />
        )
    });
    return createRouter({
      routeTree: root,
      history: createMemoryHistory({ initialEntries: ['/'] })
    });
  }, [installation]);
  return <RouterProvider router={router} />;
}
