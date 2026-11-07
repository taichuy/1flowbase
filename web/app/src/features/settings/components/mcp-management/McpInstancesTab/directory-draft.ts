import type {
  ConsoleMcpInstanceDirectory,
  ReplaceConsoleMcpInstanceDirectoryBody
} from '@1flowbase/api-client';

export function directoryPayload(snapshot: ConsoleMcpInstanceDirectory) {
  return {
    groups: snapshot.groups.map(
      ({ id, path, display_name, description_short, enabled, sort_order }) => ({
        ...(id.startsWith('draft:') ? {} : { id }),
        path,
        display_name,
        description_short,
        enabled,
        sort_order
      })
    ),
    bindings: snapshot.bindings.map(
      ({ id, tool_id, group_path, display_alias, visible, sort_order }) => ({
        ...(id.startsWith('draft:') ? {} : { id }),
        tool_id,
        group_path,
        display_alias,
        visible,
        sort_order
      })
    )
  } satisfies Omit<ReplaceConsoleMcpInstanceDirectoryBody, 'expected_snapshot'>;
}

export function sameDirectory(
  left: ConsoleMcpInstanceDirectory,
  right: ConsoleMcpInstanceDirectory
) {
  const canonical = (value: ConsoleMcpInstanceDirectory) => {
    const payload = directoryPayload(value);
    payload.groups.sort((a, b) => a.path.localeCompare(b.path));
    payload.bindings.sort((a, b) =>
      `${a.group_path}/${a.tool_id}`.localeCompare(
        `${b.group_path}/${b.tool_id}`
      )
    );
    return JSON.stringify(payload);
  };
  return (
    left.instance_record_id === right.instance_record_id &&
    canonical(left) === canonical(right)
  );
}

// Reconcile generated server IDs while preserving edits confirmed after a PUT began.
export function reconcileDirectory(
  current: ConsoleMcpInstanceDirectory,
  submitted: ConsoleMcpInstanceDirectory,
  saved: ConsoleMcpInstanceDirectory
): ConsoleMcpInstanceDirectory {
  if (current === submitted) return saved;
  const groupIds = new Map(
    submitted.groups.map((group) => [
      group.id,
      saved.groups.find((row) => row.path === group.path)?.id
    ])
  );
  const bindingIds = new Map(
    submitted.bindings.map((binding) => [
      binding.id,
      saved.bindings.find(
        (row) =>
          row.tool_id === binding.tool_id &&
          row.group_path === binding.group_path
      )?.id
    ])
  );
  return {
    ...current,
    groups: current.groups.map((row) => ({
      ...row,
      id: groupIds.get(row.id) ?? row.id
    })),
    bindings: current.bindings.map((row) => ({
      ...row,
      id: bindingIds.get(row.id) ?? row.id
    }))
  };
}
