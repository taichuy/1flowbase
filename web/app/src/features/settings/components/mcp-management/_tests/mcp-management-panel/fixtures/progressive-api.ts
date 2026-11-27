import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstance,
  ConsoleMcpInstanceDiscoveryPolicy,
  ConsoleMcpInstancePage,
  ConsoleMcpInstancesPageQuery,
  ConsoleMcpInstancesQuery,
  ConsoleMcpInterfaceCapabilitiesPageQuery,
  ConsoleMcpInterfaceCapability,
  ConsoleMcpInterfaceCapabilityPage,
  ConsoleMcpInterfaceCapabilitySummary,
  ConsoleMcpTool,
  ConsoleMcpToolSummary,
  ConsoleMcpToolSummaryPage,
  ConsoleMcpToolsPageQuery
} from '@1flowbase/api-client';
import type { Mock } from 'vitest';

export interface ProgressiveMcpApiMock {
  fetchSettingsMcpCatalog?: Mock;
  fetchSettingsMcpInstances?: Mock;
  fetchSettingsMcpInstancesPage?: Mock;
  fetchSettingsMcpToolsPage?: Mock;
  fetchSettingsMcpTool?: Mock;
  fetchSettingsMcpInstanceDiscoveryPolicy?: Mock;
  fetchSettingsMcpInterfaceCapabilities?: Mock;
  fetchSettingsMcpInterfaceCapabilitiesPage?: Mock;
  fetchSettingsMcpInterfaceCapability?: Mock;
}

function withInstanceCounts(
  catalog: ConsoleMcpCatalog
): ConsoleMcpInstance[] {
  return catalog.instances.map((instance) => ({
    ...structuredClone(instance),
    group_count:
      instance.group_count ??
      catalog.groups.filter((group) => group.instance_record_id === instance.id)
        .length,
    tool_count:
      instance.tool_count ??
      catalog.bindings.filter(
        (binding) => binding.instance_record_id === instance.id
      ).length
  }));
}

function toToolSummary(tool: ConsoleMcpTool): ConsoleMcpToolSummary {
  return {
    id: tool.id,
    workspace_id: tool.workspace_id,
    tool_id: tool.tool_id,
    name: tool.name,
    short_description: tool.short_description,
    managed_by: tool.managed_by,
    execution_target: structuredClone(tool.execution_target),
    operation: tool.operation,
    permission_code: tool.permission_code,
    risk_level: tool.risk_level,
    des_id: tool.des_id,
    des_id_required: tool.des_id_required,
    status: tool.status,
    availability_status: tool.availability_status,
    availability_reason: tool.availability_reason,
    revision: tool.revision
  };
}

function toInterfaceCapabilitySummary(
  capability: ConsoleMcpInterfaceCapability
): ConsoleMcpInterfaceCapabilitySummary {
  return {
    interface_id: capability.interface_id,
    method: capability.method,
    path: capability.path,
    name: capability.name,
    short_description: capability.short_description,
    permission_code: capability.permission_code,
    risk_level: capability.risk_level,
    bindable: capability.bindable,
    disabled_reason: capability.disabled_reason
  };
}

export function installProgressiveMcpApi(
  api: ProgressiveMcpApiMock,
  catalog: ConsoleMcpCatalog,
  interfaceCapabilities: ConsoleMcpInterfaceCapability[] = []
) {
  const instances = withInstanceCounts(catalog);

  api.fetchSettingsMcpCatalog?.mockImplementation(async () =>
    structuredClone({
      ...catalog,
      instances
    })
  );

  api.fetchSettingsMcpInstances?.mockImplementation(
    async (options?: ConsoleMcpInstancesQuery) => {
      const status = options?.status?.trim();
      return structuredClone(
        status
          ? instances.filter((instance) => instance.status === status)
          : instances
      );
    }
  );

  api.fetchSettingsMcpInstancesPage?.mockImplementation(
    async (
      options?: ConsoleMcpInstancesPageQuery
    ): Promise<ConsoleMcpInstancePage> => {
      const page = Math.max(1, options?.page ?? 1);
      const pageSize = Math.max(1, options?.page_size ?? 20);
      const status = options?.status?.trim();
      const keyword = options?.keyword?.trim().toLowerCase();
      const filtered = instances.filter((instance) => {
        if (status && instance.status !== status) {
          return false;
        }
        if (keyword) {
          const haystack = [
            instance.instance_id,
            instance.name,
            instance.description_short ?? ''
          ]
            .join(' ')
            .toLowerCase();
          if (!haystack.includes(keyword)) {
            return false;
          }
        }
        return true;
      });
      const start = (page - 1) * pageSize;
      return {
        items: structuredClone(filtered.slice(start, start + pageSize)),
        total: filtered.length,
        page,
        page_size: pageSize
      };
    }
  );

  api.fetchSettingsMcpToolsPage?.mockImplementation(
    async (
      options?: ConsoleMcpToolsPageQuery
    ): Promise<ConsoleMcpToolSummaryPage> => {
      const page = Math.max(1, options?.page ?? 1);
      const pageSize = Math.max(1, options?.page_size ?? 20);
      const keyword = options?.keyword?.trim().toLowerCase();
      const toolIds =
        options?.tool_ids && options.tool_ids.length > 0
          ? new Set(options.tool_ids)
          : null;
      const filtered = catalog.tools.filter((tool) => {
        if (toolIds && !toolIds.has(tool.tool_id)) {
          return false;
        }
        if (
          options?.execution_target_kind &&
          tool.execution_target.kind !== options.execution_target_kind
        ) {
          return false;
        }
        if (options?.interface_id) {
          if (
            tool.execution_target.kind !== 'interface_wrapper' ||
            tool.execution_target.interface_id !== options.interface_id
          ) {
            return false;
          }
        }
        if (options?.risk_level && tool.risk_level !== options.risk_level) {
          return false;
        }
        if (options?.status && tool.status !== options.status) {
          return false;
        }
        if (
          typeof options?.des_id_required === 'boolean' &&
          tool.des_id_required !== options.des_id_required
        ) {
          return false;
        }
        if (keyword) {
          const targetId =
            tool.execution_target.kind === 'interface_wrapper'
              ? tool.execution_target.interface_id
              : tool.execution_target.kind === 'mcp_proxy'
                ? tool.execution_target.remote_tool_name
                : '';
          const haystack = [
            tool.tool_id,
            tool.name,
            tool.short_description,
            tool.operation,
            tool.des_id ?? '',
            targetId
          ]
            .join(' ')
            .toLowerCase();
          if (!haystack.includes(keyword)) {
            return false;
          }
        }
        return true;
      });
      const start = (page - 1) * pageSize;
      return {
        items: filtered.slice(start, start + pageSize).map(toToolSummary),
        total: filtered.length,
        page,
        page_size: pageSize
      };
    }
  );

  api.fetchSettingsMcpTool?.mockImplementation(async (toolId: string) => {
    const tool = catalog.tools.find((entry) => entry.tool_id === toolId);
    if (!tool) {
      throw new Error(`Unknown fixture tool: ${toolId}`);
    }
    return structuredClone(tool);
  });

  api.fetchSettingsMcpInstanceDiscoveryPolicy?.mockImplementation(
    async (instanceId: string): Promise<ConsoleMcpInstanceDiscoveryPolicy> => {
      const policy = catalog.discovery_policies.find(
        (entry) => entry.instance_id === instanceId
      );
      if (policy) {
        return structuredClone(policy);
      }
      const instance = catalog.instances.find(
        (entry) => entry.instance_id === instanceId
      );
      if (!instance) {
        throw new Error(`Unknown fixture instance policy: ${instanceId}`);
      }
      return {
        id: `policy-${instance.id}`,
        workspace_id: instance.workspace_id,
        instance_record_id: instance.id,
        instance_id: instance.instance_id,
        list_default_limit: 20,
        list_max_depth: 3,
        list_regex_enabled: false,
        list_regex_max_length: 120,
        list_return_fields: []
      };
    }
  );

  api.fetchSettingsMcpInterfaceCapabilities?.mockImplementation(async () =>
    structuredClone(interfaceCapabilities)
  );

  api.fetchSettingsMcpInterfaceCapabilitiesPage?.mockImplementation(
    async (
      options?: ConsoleMcpInterfaceCapabilitiesPageQuery
    ): Promise<ConsoleMcpInterfaceCapabilityPage> => {
      const offset = Math.max(0, options?.offset ?? 0);
      const limit = Math.max(1, options?.limit ?? 50);
      const bindableOnly = options?.bindable_only ?? false;
      const method = options?.method?.trim().toUpperCase();
      const q = options?.q?.trim().toLowerCase();
      const filtered = interfaceCapabilities.filter((capability) => {
        if (bindableOnly && !capability.bindable) {
          return false;
        }
        if (method && capability.method.toUpperCase() !== method) {
          return false;
        }
        if (q) {
          const haystack = [
            capability.interface_id,
            capability.method,
            capability.path,
            capability.name,
            capability.short_description,
            capability.permission_code ?? ''
          ]
            .join(' ')
            .toLowerCase();
          if (!haystack.includes(q)) {
            return false;
          }
        }
        return true;
      });
      const items = filtered
        .slice(offset, offset + limit)
        .map(toInterfaceCapabilitySummary);
      const nextOffset =
        offset + items.length < filtered.length ? offset + items.length : null;
      return {
        items,
        total: filtered.length,
        offset,
        limit,
        has_more: nextOffset !== null,
        next_offset: nextOffset
      };
    }
  );

  api.fetchSettingsMcpInterfaceCapability?.mockImplementation(
    async (interfaceId: string) => {
      const capability = interfaceCapabilities.find(
        (entry) => entry.interface_id === interfaceId
      );
      if (!capability) {
        throw new Error(`Unknown fixture interface capability: ${interfaceId}`);
      }
      return structuredClone(capability);
    }
  );
}
