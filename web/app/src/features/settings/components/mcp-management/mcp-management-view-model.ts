type DirectoryInstance = {
  id: string;
  instance_id: string;
  name: string;
  default_entry_path: string;
};

type DirectoryGroup = {
  id: string;
  instance_record_id: string;
  path: string;
  display_name: string;
  description_short: string | null;
  enabled: boolean;
  sort_order: number;
};

type DirectoryBinding = {
  id: string;
  instance_record_id: string;
  tool_record_id: string;
  group_path: string;
  tool_id: string;
  display_alias: string | null;
  visible: boolean;
  sort_order: number;
};

type DirectoryTool = {
  id: string;
  tool_id: string;
  name: string;
};

export type McpDirectoryTreeNode = {
  key: string;
  title: string;
  node_type: 'instance' | 'group' | 'binding';
  path: string;
  binding_id?: string;
  children?: McpDirectoryTreeNode[];
};

export function normalizeMcpDirectoryPath(path: string | null | undefined) {
  const value = path?.trim();

  if (!value || value === '/') {
    return '/';
  }

  return value.startsWith('/') ? value : `/${value}`;
}

function slugSegment(value: string) {
  return value
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

function normalizeFallback(seed: string) {
  const normalized = seed.replace(/[^A-Za-z0-9_]/g, '').slice(0, 8);

  return normalized || 'tool';
}

export function buildReadableToolId(name: string, fallbackSeed = '') {
  const nameSegment = slugSegment(name);

  return nameSegment || (fallbackSeed ? normalizeFallback(fallbackSeed) : '');
}

export function buildRandomToolIdSeed() {
  return Math.random()
    .toString(36)
    .replace(/[^a-z0-9]/gi, '')
    .slice(0, 8);
}

export function buildMcpDirectoryTreeData({
  instance,
  groups,
  bindings,
  tools
}: {
  instance: DirectoryInstance;
  groups: DirectoryGroup[];
  bindings: DirectoryBinding[];
  tools: DirectoryTool[];
}): McpDirectoryTreeNode[] {
  const instanceGroups = groups.filter(
    (group) => group.instance_record_id === instance.id
  );
  const instanceBindings = bindings.filter(
    (binding) => binding.instance_record_id === instance.id
  );
  const toolByRecordId = new Map(tools.map((tool) => [tool.id, tool]));
  const groupByPath = new Map(
    instanceGroups.map((group) => [normalizeMcpDirectoryPath(group.path), group])
  );

  for (const binding of instanceBindings) {
    const path = normalizeMcpDirectoryPath(binding.group_path);
    if (!groupByPath.has(path)) {
      groupByPath.set(path, {
        id: path,
        instance_record_id: instance.id,
        path,
        display_name: path === '/' ? '/' : '',
        description_short: null,
        enabled: true,
        sort_order: Number.MAX_SAFE_INTEGER
      });
    }
  }

  const groupNodes = Array.from(groupByPath.values())
    .sort(
      (left, right) =>
        left.sort_order - right.sort_order ||
        left.path.localeCompare(right.path)
    )
    .map((group) => {
      const path = normalizeMcpDirectoryPath(group.path);
      const groupBindings = instanceBindings
        .filter(
          (binding) => normalizeMcpDirectoryPath(binding.group_path) === path
        )
        .sort(
          (left, right) =>
            left.sort_order - right.sort_order ||
            left.tool_id.localeCompare(right.tool_id)
        );

      return {
        key: `group:${path}`,
        title:
          group.display_name && group.display_name !== path
            ? `${group.display_name} ${path}`
            : path,
        node_type: 'group' as const,
        path,
        children: groupBindings.map((binding) => {
          const tool = toolByRecordId.get(binding.tool_record_id);
          const label = binding.display_alias || tool?.name || binding.tool_id;

          return {
            key: `binding:${binding.id}`,
            title: `${label} ${binding.tool_id}`,
            node_type: 'binding' as const,
            path,
            binding_id: binding.id
          };
        })
      };
    });

  const rootPath = normalizeMcpDirectoryPath(instance.default_entry_path);

  return [
    {
      key: `instance:${instance.instance_id}:${rootPath}`,
      title: `${instance.name} ${rootPath}`,
      node_type: 'instance',
      path: rootPath,
      children: groupNodes
    }
  ];
}
