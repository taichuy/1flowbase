import type {
  ConsoleMcpCatalog,
  ConsoleMcpInstanceDirectory,
  ReplaceConsoleMcpInstanceDirectoryBody
} from '@1flowbase/api-client';
import type { Mock } from 'vitest';

/** A server-shaped snapshot: IDs are stable and new rows receive server IDs. */
export function installDirectorySnapshotApi(
  api: {
    fetchSettingsMcpInstanceDirectory: Mock;
    replaceSettingsMcpInstanceDirectory: Mock;
  },
  catalog: ConsoleMcpCatalog
) {
  const snapshots = new Map<string, ConsoleMcpInstanceDirectory>(
    catalog.instances.map((instance) => [
      instance.instance_id,
      {
        instance_record_id: instance.id,
        groups: structuredClone(
          catalog.groups.filter(
            (group) => group.instance_record_id === instance.id
          )
        ),
        bindings: structuredClone(
          catalog.bindings.filter(
            (binding) => binding.instance_record_id === instance.id
          )
        )
      }
    ])
  );
  api.fetchSettingsMcpInstanceDirectory.mockImplementation(
    async (instanceId: string) => {
      const snapshot = snapshots.get(instanceId);
      if (!snapshot) throw new Error(`Unknown fixture instance: ${instanceId}`);
      return structuredClone(snapshot);
    }
  );
  api.replaceSettingsMcpInstanceDirectory.mockImplementation(
    async (
      instanceId: string,
      body: ReplaceConsoleMcpInstanceDirectoryBody
    ) => {
      const previous = snapshots.get(instanceId);
      if (!previous) throw new Error(`Unknown fixture instance: ${instanceId}`);
      const snapshot: ConsoleMcpInstanceDirectory = {
        instance_record_id: previous.instance_record_id,
        groups: body.groups.map((group, index) => ({
          ...group,
          id: group.id ?? `server-group-${index}`,
          instance_record_id: previous.instance_record_id
        })),
        bindings: body.bindings.map((binding, index) => {
          const toolRecordId =
            previous.bindings.find((row) => row.id === binding.id)
              ?.tool_record_id ??
            catalog.tools.find((tool) => tool.tool_id === binding.tool_id)?.id;
          if (!toolRecordId)
            throw new Error(`Unknown fixture Tool: ${binding.tool_id}`);
          return {
            ...binding,
            id: binding.id ?? `server-binding-${index}`,
            instance_record_id: previous.instance_record_id,
            tool_record_id: toolRecordId
          };
        })
      };
      snapshots.set(instanceId, snapshot);
      return structuredClone(snapshot);
    }
  );
  return snapshots;
}

/** Commit on the mock server now, but hold the response until the test releases it. */
export function holdNextDirectoryReplace(api: {
  replaceSettingsMcpInstanceDirectory: Mock;
}) {
  const replace =
    api.replaceSettingsMcpInstanceDirectory.getMockImplementation();
  if (!replace)
    throw new Error(
      'Install the directory snapshot API before holding a response'
    );
  let release!: () => void;
  const responseGate = new Promise<void>((resolve) => {
    release = resolve;
  });
  api.replaceSettingsMcpInstanceDirectory.mockImplementationOnce(
    async (
      instanceId: string,
      body: ReplaceConsoleMcpInstanceDirectoryBody,
      csrfToken: string
    ) => {
      const committed: ConsoleMcpInstanceDirectory = await replace(
        instanceId,
        body,
        csrfToken
      );
      await responseGate;
      return committed;
    }
  );
  return { release };
}
