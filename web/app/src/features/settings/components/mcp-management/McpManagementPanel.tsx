import { Tabs } from 'antd';
import { useRouterState } from '@tanstack/react-router';
import { useCallback, useEffect } from 'react';
import type {
  ConsoleMcpCatalog,
  ConsoleMcpInterfaceCapability
} from '@1flowbase/api-client';

import { i18nText } from '../../../../shared/i18n/text';
import {
  isMcpManagementTabKey,
  resolveMcpManagementTabKey,
  updateMcpManagementTabQuery
} from './mcp-management-route-state';
import { McpInstancesTab } from './McpInstancesTab';
import { McpToolsTab } from './McpToolsTab';
import './mcp-management-panel.css';

export function McpManagementPanel({
  canManage,
  catalog,
  interfaceCapabilities
}: {
  canManage: boolean;
  catalog: ConsoleMcpCatalog;
  interfaceCapabilities: ConsoleMcpInterfaceCapability[];
}) {
  const locationSearch = useRouterState({
    select: (state) => state.location.search as Record<string, unknown>
  });
  const requestedTab =
    typeof locationSearch.tab === 'string' ? locationSearch.tab : null;
  const activeTab = resolveMcpManagementTabKey(requestedTab);
  const handleTabChange = useCallback(
    (nextTab: string) => {
      if (!isMcpManagementTabKey(nextTab) || nextTab === activeTab) {
        return;
      }
      updateMcpManagementTabQuery(nextTab);
    },
    [activeTab]
  );

  useEffect(() => {
    if (requestedTab !== activeTab) {
      updateMcpManagementTabQuery(activeTab, 'replace');
    }
  }, [activeTab, requestedTab]);

  return (
    <Tabs
      activeKey={activeTab}
      className="mcp-management"
      items={[
        {
          key: 'instances',
          label: i18nText('settings', 'auto.mcp_instances'),
          children: <McpInstancesTab canManage={canManage} catalog={catalog} />
        },
        {
          key: 'tools',
          label: i18nText('settings', 'auto.mcp_tool_config'),
          children: (
            <McpToolsTab
              canManage={canManage}
              catalog={catalog}
              interfaceCapabilities={interfaceCapabilities}
            />
          )
        }
      ]}
      onChange={handleTabChange}
    />
  );
}
