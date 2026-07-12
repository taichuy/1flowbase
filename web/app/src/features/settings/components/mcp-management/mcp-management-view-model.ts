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
  short_description: string;
};

export type McpDirectoryTreeNode = {
  key: string;
  title: string;
  node_type: 'instance' | 'group' | 'binding';
  path: string;
  display_name?: string;
  description_short?: string | null;
  tool_short_description?: string;
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

function parentDirectoryPath(path: string) {
  const segments = normalizeMcpDirectoryPath(path).split('/').filter(Boolean);
  if (segments.length <= 1) return '/';
  return `/${segments.slice(0, -1).join('/')}`;
}

function directoryName(path: string) {
  return (
    normalizeMcpDirectoryPath(path).split('/').filter(Boolean).at(-1) ?? '/'
  );
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
  const rootPath = normalizeMcpDirectoryPath(instance.default_entry_path);
  const instanceGroups = groups.filter(
    (group) => group.instance_record_id === instance.id
  );
  const instanceBindings = bindings.filter(
    (binding) => binding.instance_record_id === instance.id
  );
  const toolByRecordId = new Map(tools.map((tool) => [tool.id, tool]));
  const bindingById = new Map(
    instanceBindings.map((binding) => [binding.id, binding])
  );
  const groupByPath = new Map(
    instanceGroups.map((group) => [
      normalizeMcpDirectoryPath(group.path),
      group
    ])
  );
  const directoryPaths = new Set<string>();

  const registerPath = (rawPath: string) => {
    let path = normalizeMcpDirectoryPath(rawPath);
    while (path !== rootPath && path !== '/') {
      directoryPaths.add(path);
      path = parentDirectoryPath(path);
    }
  };

  for (const group of instanceGroups) registerPath(group.path);
  for (const binding of instanceBindings) registerPath(binding.group_path);

  const groupNodeByPath = new Map<string, McpDirectoryTreeNode>();
  const sortedPaths = Array.from(directoryPaths).sort((left, right) => {
    const leftGroup = groupByPath.get(left);
    const rightGroup = groupByPath.get(right);
    return (
      (leftGroup?.sort_order ?? Number.MAX_SAFE_INTEGER) -
        (rightGroup?.sort_order ?? Number.MAX_SAFE_INTEGER) ||
      left.localeCompare(right)
    );
  });

  for (const path of sortedPaths) {
    const group = groupByPath.get(path);
    groupNodeByPath.set(path, {
      key: `group:${path}`,
      title: group?.display_name || directoryName(path),
      display_name: group?.display_name || undefined,
      description_short: group?.description_short ?? undefined,
      node_type: 'group',
      path,
      children: []
    });
  }

  const rootNode: McpDirectoryTreeNode = {
    key: `instance:${instance.instance_id}:${rootPath}`,
    title: `${instance.name} ${rootPath}`,
    node_type: 'instance',
    path: rootPath,
    children: []
  };

  for (const [path, node] of groupNodeByPath) {
    const parentPath = parentDirectoryPath(path);
    (parentPath === rootPath
      ? rootNode
      : groupNodeByPath.get(parentPath)
    )?.children?.push(node);
  }

  for (const binding of instanceBindings) {
    const path = normalizeMcpDirectoryPath(binding.group_path);
    const tool = toolByRecordId.get(binding.tool_record_id);
    const bindingNode: McpDirectoryTreeNode = {
      key: `binding:${binding.id}`,
      title: binding.tool_id,
      tool_short_description: tool?.short_description,
      node_type: 'binding',
      path,
      binding_id: binding.id
    };

    (path === rootPath ? rootNode : groupNodeByPath.get(path))?.children?.push(
      bindingNode
    );
  }

  const sortChildren = (node: McpDirectoryTreeNode) => {
    node.children?.sort((left, right) => {
      if (left.node_type !== right.node_type) {
        return left.node_type === 'group' ? -1 : 1;
      }
      if (left.node_type === 'binding' && right.node_type === 'binding') {
        const leftBinding = left.binding_id
          ? bindingById.get(left.binding_id)
          : undefined;
        const rightBinding = right.binding_id
          ? bindingById.get(right.binding_id)
          : undefined;
        return (
          (leftBinding?.sort_order ?? 0) - (rightBinding?.sort_order ?? 0) ||
          left.title.localeCompare(right.title)
        );
      }
      const leftGroup = groupByPath.get(left.path);
      const rightGroup = groupByPath.get(right.path);
      return (
        (leftGroup?.sort_order ?? Number.MAX_SAFE_INTEGER) -
          (rightGroup?.sort_order ?? Number.MAX_SAFE_INTEGER) ||
        left.path.localeCompare(right.path)
      );
    });
    node.children?.forEach(sortChildren);
  };

  sortChildren(rootNode);
  return [rootNode];
}

export function nextMcpDirectoryExpandedKeys(
  currentKeys: string[],
  changedKey: string,
  expanded: boolean
) {
  if (expanded) {
    if (changedKey.startsWith('instance:')) return [changedKey];
    if (changedKey.startsWith('group:')) {
      const expandedPath = changedKey.slice('group:'.length);
      return [
        ...currentKeys.filter(
          (key) =>
            key !== changedKey && !key.startsWith(`group:${expandedPath}/`)
        ),
        changedKey
      ];
    }
    return currentKeys.includes(changedKey)
      ? currentKeys
      : [...currentKeys, changedKey];
  }

  if (changedKey.startsWith('instance:')) return [];
  if (changedKey.startsWith('group:')) {
    const collapsedPath = changedKey.slice('group:'.length);
    return currentKeys.filter(
      (key) => key !== changedKey && !key.startsWith(`group:${collapsedPath}/`)
    );
  }
  return currentKeys.filter((key) => key !== changedKey);
}
