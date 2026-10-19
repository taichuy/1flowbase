import { useMutation, useQueryClient } from '@tanstack/react-query';
import { App } from 'antd';
import { useCallback, useEffect, useRef, useState } from 'react';
import type { ConsoleMcpCatalog } from '@1flowbase/api-client';

import {
  deleteSettingsMcpTool,
  exportSettingsMcpCatalog,
  refreshSettingsMcpToolDescription,
  settingsMcpCatalogQueryKey
} from '../../../api/mcp-management';
import { useAuthStore } from '../../../../../state/auth-store';

import { i18nText } from '../../../../../shared/i18n/text';

import { downloadMcpExportPackage } from '../mcp-management-utils';
import { toolSourceLabel } from './tool-editor-model';

import {
  MCP_TOOLS_PAGE_SIZE,
  readMcpToolsRouteState,
  writeMcpToolsRouteState,
  type McpToolsRouteState
} from './route-state';

import type { useMcpToolEditor } from './use-mcp-tool-editor';

/** Owns browser route/filter/pagination state and catalog refresh/delete/export actions. */
export function useMcpToolsCatalog(
  catalog: ConsoleMcpCatalog,
  {
    setExportingCatalog
  }: Pick<ReturnType<typeof useMcpToolEditor>, 'setExportingCatalog'>
) {
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
  const deleteToolMutation = useMutation({
    mutationFn: (toolId: string) => deleteSettingsMcpTool(toolId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_deleted'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
    }
  });
  const refreshMutation = useMutation({
    mutationFn: (toolId: string) =>
      refreshSettingsMcpToolDescription(toolId, csrfToken),
    onSuccess: async () => {
      message.success(i18nText('settings', 'auto.mcp_des_id_refreshed'));
      await queryClient.invalidateQueries({
        queryKey: settingsMcpCatalogQueryKey
      });
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
  const filteredTools = catalog.tools.filter((tool) => {
    const source = toolSourceLabel(tool);
    const text =
      `${tool.name} ${tool.tool_id} ${tool.operation} ${source}`.toLowerCase();
    return (
      (!routeState.keyword ||
        text.includes(routeState.keyword.toLowerCase())) &&
      (!routeState.execution_target_kind ||
        tool.execution_target.kind === routeState.execution_target_kind) &&
      (!routeState.interface_id ||
        (tool.execution_target.kind === 'interface_wrapper' &&
          tool.execution_target.interface_id === routeState.interface_id)) &&
      (!routeState.risk_level || tool.risk_level === routeState.risk_level) &&
      (!routeState.status || tool.status === routeState.status) &&
      (routeState.des_id_required === undefined ||
        tool.des_id_required === routeState.des_id_required)
    );
  });
  const pageCount = Math.max(
    1,
    Math.ceil(filteredTools.length / MCP_TOOLS_PAGE_SIZE)
  );
  const visibleTools = filteredTools.slice(
    (routeState.page - 1) * MCP_TOOLS_PAGE_SIZE,
    routeState.page * MCP_TOOLS_PAGE_SIZE
  );

  useEffect(() => {
    if (routeState.page <= pageCount) {
      return;
    }
    const nextState = { ...routeState, page: pageCount };
    updateRouteState(nextState, 'replace');
  }, [pageCount, routeState, updateRouteState]);

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
    filteredTools,
    visibleTools,
    applyFilters,
    resetFilters
  };
}
