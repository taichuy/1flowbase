import { Tabs } from 'antd';
import { useRouterState } from '@tanstack/react-router';
import { useCallback, useEffect } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import {
  isMcpManagementTabKey,
  resolveMcpManagementTabKey,
  updateMcpManagementTabQuery
} from './mcp-management-route-state';
import { useMcpTablePagination } from './mcp-management-state';
import { McpInstancesTab } from './McpInstancesTab';
import { McpToolsTab } from './McpToolsTab';
import { McpBundleActions } from './bundle/McpBundleActions';
import { ThirdPartyMcpTab } from './upstream/ThirdPartyMcpTab';
import './mcp-management-panel.css';

export function McpManagementPanel(_props: { canManage?: boolean } = {}) {
  const instancesPagination = useMcpTablePagination();
  const upstreamPagination = useMcpTablePagination();
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
      tabBarExtraContent={<McpBundleActions />}
      items={[
        {
          key: 'instances',
          label: i18nText('settings', 'auto.mcp_instances'),
          children: activeTab === 'instances' ? <McpInstancesTab {...instancesPagination} /> : null
        },
        {
          key: 'tools',
          label: i18nText('settings', 'auto.mcp_tool_config'),
          children: activeTab === 'tools' ? <McpToolsTab /> : null
        },
        {
          key: 'third-party',
          label: i18nText('settings', 'auto.mcp_third_party'),
          children:
            activeTab === 'third-party' ? (
              <ThirdPartyMcpTab
                {...upstreamPagination}
                onImported={() => updateMcpManagementTabQuery('tools')}
              />
            ) : null
        }
      ]}
      onChange={handleTabChange}
    />
  );
}
