import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { App } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ConsoleMcpToolSummary } from '@1flowbase/api-client';

import {
  deleteSettingsMcpTool,
  exportSettingsMcpCatalog,
  fetchSettingsMcpToolsPage,
  invalidateSettingsMcpInstancesAndTools,
  refreshSettingsMcpToolDescription,
  settingsMcpToolsQueryKey
} from '../../../api/mcp-management';
import { useAuthStore } from '../../../../../state/auth-store';

import { i18nText } from '../../../../../shared/i18n/text';

import { downloadMcpExportPackage } from '../mcp-management-utils';

import {
  MCP_TOOLS_PAGE_SIZE,
  readMcpToolsRouteState,
  writeMcpToolsRouteState,
  type McpToolsRouteState
} from './route-state';

import type { useMcpToolEditor } from './use-mcp-tool-editor';

const EMPTY_TOOLS: ConsoleMcpToolSummary[] = [];

/** Owns browser route/filter/pagination state and server-paginated tool list actions. */
export function useMcpToolsCatalog({
  setExportingCatalog
}: Pick<ReturnType<typeof useMcpToolEditor>, 'setExportingCatalog'>) {
  const { message } = App.useApp();
  const csrfToken = useAuthStore((state) => state.csrfToken ?? '');
  const queryClient = useQueryClient();
  const [routeState, setRouteState] = useState(readMcpToolsRouteState);
  const [filterDraft, setFilterDraft] = useState(routeState);
  const updateRouteState = useCallback(
    (nextState: McpToolsRouteState, mode: 'push' | 'replace' = 'push') => {
      writeMcpToolsRouteState(nextState, mode);
      setRouteState(nextState);
    },
    []
  );

  useEffect(() => {
    const handlePopState = () => {
      const nextState = readMcpToolsRouteState();
      setRouteState(nextState);
      setFilterDraft(nextState);
    };
    window.addEventListener('popstate', handlePopState);
    return () => window.removeEventListener('popstate', handlePopState);
  }, []);

  const toolsQuery = useQuery({
    queryKey: [...settingsMcpToolsQueryKey, 'page', routeState],
    queryFn: () =>
      fetchSettingsMcpToolsPage({
        page: routeState.page,
        page_size: MCP_TOOLS_PAGE_SIZE,
        keyword: routeState.keyword,
        execution_target_kind: routeState.execution_target_kind,
        interface_id: routeState.interface_id,
        risk_level: routeState.risk_level,
        status: routeState.status,
        des_id_required: routeState.des_id_required
      })
  });

  const deleteToolMutation = useMutation({
    mutationFn: (toolId: string) => deleteSettingsMcpTool(toolId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await invalidateSettingsMcpInstancesAndTools(queryClient);
    }
  });
  const refreshMutation = useMutation({
    mutationFn: (toolId: string) =>
      refreshSettingsMcpToolDescription(toolId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_des_id_refreshed'));
      await invalidateSettingsMcpInstancesAndTools(queryClient);
    }
  });
  const deleteToolMutationRef = useRef(deleteToolMutation);
  const refreshMutationRef = useRef(refreshMutation);

  useEffect(() => {
    deleteToolMutationRef.current = deleteToolMutation;
    refreshMutationRef.current = refreshMutation;
  }, [deleteToolMutation, refreshMutation]);

  async function handleExportCatalog() {
    setExportingCatalog(true);
    try {
      const exportPackage = await exportSettingsMcpCatalog();
      downloadMcpExportPackage(exportPackage);
      message.success(i18nText('settings', 'auto.mcp_export_ready'));
    } catch (error) {
      message.error(error instanceof Error ? error.message : String(error));
    } finally {
      setExportingCatalog(false);
    }
  }

  const visibleTools = toolsQuery.data?.items ?? EMPTY_TOOLS;
  const totalTools = toolsQuery.data?.total ?? 0;
  const pageCount = Math.max(1, Math.ceil(totalTools / MCP_TOOLS_PAGE_SIZE));

  useEffect(() => {
    if (!toolsQuery.data || routeState.page <= pageCount) {
      return;
    }
    const nextState = { ...routeState, page: pageCount };
    updateRouteState(nextState, 'replace');
  }, [pageCount, routeState, toolsQuery.data, updateRouteState]);

  const applyFilters = useCallback(() => {
    const nextState: McpToolsRouteState = {
      ...filterDraft,
      page: 1,
      keyword: filterDraft.keyword?.trim() || undefined
    };
    updateRouteState(nextState);
    setFilterDraft(nextState);
  }, [filterDraft, updateRouteState]);

  const resetFilters = useCallback(() => {
    const nextState: McpToolsRouteState = { page: 1 };
    updateRouteState(nextState);
    setFilterDraft(nextState);
  }, [updateRouteState]);

  return {
    routeState,
    filterDraft,
    setFilterDraft,
    updateRouteState,
    refreshMutation,
    deleteToolMutationRef,
    refreshMutationRef,
    handleExportCatalog,
    visibleTools,
    totalTools,
    isLoading: toolsQuery.isLoading,
    isError: toolsQuery.isError,
    applyFilters,
    resetFilters
  };
}
