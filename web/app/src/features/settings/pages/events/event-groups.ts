import type { EventCatalogEntry } from '@1flowbase/api-client';

interface EventGroupNode {
  title: string;
  value: string;
  children: EventGroupNode[];
}

export function eventGroupTree(entries: EventCatalogEntry[]): EventGroupNode[] {
  const roots: EventGroupNode[] = [];
  const nodesByPath = new Map<string, EventGroupNode>();
  for (const entry of entries) {
    let children = roots;
    (entry.group_path ?? []).forEach((title, index, path) => {
      const value = JSON.stringify(path.slice(0, index + 1));
      let node = nodesByPath.get(value);
      if (!node) {
        node = { title, value, children: [] };
        children.push(node);
        nodesByPath.set(value, node);
      }
      children = node.children;
    });
  }
  return roots;
}
