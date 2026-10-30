import type {
  ContainerRoundPath,
  NodeExecutionIdentity
} from '@1flowbase/api-client';
import { i18nText } from '../../../../../../shared/i18n/text';

export interface ContainerRoundOption {
  value: string;
  title: string;
  path: ContainerRoundPath[];
  children: ContainerRoundOption[];
}

export function containerRoundKey(path: ContainerRoundPath[]) {
  return path
    .map((entry) => `${entry.container_execution_id}:${entry.round_index}`)
    .join('/');
}

export function isWithinContainerRound(
  identity: NodeExecutionIdentity | null | undefined,
  path: ContainerRoundPath[]
) {
  return path.every(
    (entry, index) =>
      identity?.round_path[index]?.container_execution_id ===
        entry.container_execution_id &&
      identity?.round_path[index]?.round_index === entry.round_index
  );
}

export function containerRoundLabel(
  entry: ContainerRoundPath,
  aliases: ReadonlyMap<string, string>,
  language?: string
) {
  return i18nText('agentFlow', 'trajectory.container_round', {
    container:
      aliases.get(entry.container_execution_id) || entry.container_node_id,
    round: entry.round_index + 1,
    lng: language
  });
}

export function containerRoundOptions(
  identities: Array<NodeExecutionIdentity | null | undefined>,
  aliases: ReadonlyMap<string, string>,
  language?: string
): ContainerRoundOption[] {
  const roots: ContainerRoundOption[] = [];
  const known = new Map<string, ContainerRoundOption>();
  for (const identity of identities) {
    let level = roots;
    for (
      let depth = 0;
      depth < (identity?.round_path.length ?? 0);
      depth += 1
    ) {
      const path = identity!.round_path.slice(0, depth + 1);
      const value = containerRoundKey(path);
      let option = known.get(value);
      if (!option) {
        option = {
          value,
          title: containerRoundLabel(path[depth], aliases, language),
          path,
          children: []
        };
        known.set(value, option);
        level.push(option);
      }
      level = option.children;
    }
  }
  return roots;
}

export function findContainerRound(
  options: ContainerRoundOption[],
  key: string | undefined
): ContainerRoundPath[] {
  const queue = [...options];
  for (let index = 0; index < queue.length; index += 1) {
    const option = queue[index];
    if (option.value === key) return option.path;
    queue.push(...option.children);
  }
  return [];
}
