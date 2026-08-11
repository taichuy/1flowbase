import {
  resolveChildContainerPath,
  type ChildContainerNode
} from './child-container-tree';

export const OPEN_CHILD_CONTAINER_EVENT = 'open_child_container';
export const CHILD_CONTAINER_SEARCH_KEY = 'container_id';

export interface ChildContainerRuntimeDiagnostic {
  code: string;
  message: string;
  containerId: string | null;
}

export interface ChildContainerRuntimeResolution {
  current: ChildContainerNode | null;
  path: ChildContainerNode[];
  renderPath: ChildContainerNode[];
  diagnostics: ChildContainerRuntimeDiagnostic[];
}

export interface OpenChildContainerEvent {
  sourceBlockId: string;
  sourceTargetContainerIds: readonly string[];
  name: string;
  payload?: Record<string, unknown>;
}

export function createChildContainerUrl(
  currentUrl: string,
  containerId: string | null
): string {
  const url = new URL(currentUrl, 'http://frontstage.local');
  if (containerId) {
    url.searchParams.set(CHILD_CONTAINER_SEARCH_KEY, containerId);
  } else {
    url.searchParams.delete(CHILD_CONTAINER_SEARCH_KEY);
  }
  return `${url.pathname}${url.search}${url.hash}`;
}

export function resolveChildContainerRuntime(
  containers: readonly ChildContainerNode[],
  containerId: string | null | undefined
): ChildContainerRuntimeResolution {
  if (!containerId) {
    return { current: null, path: [], renderPath: [], diagnostics: [] };
  }
  const path = resolveChildContainerPath([...containers], containerId);
  if (!path) {
    return {
      current: null,
      path: [],
      renderPath: [],
      diagnostics: [unknownContainerDiagnostic(containerId)]
    };
  }

  const current = path.at(-1) ?? null;
  const overlayAncestors = path.filter(
    (container) =>
      container.id !== current?.id && container.presentation !== 'inline'
  );
  const visibleOverlayAncestor = overlayAncestors.at(-1);
  const hiddenOverlayIds = new Set(
    overlayAncestors.slice(0, -1).map((container) => container.id)
  );
  const renderPath = path.filter(
    (container) =>
      container.presentation === 'inline' ||
      container.id === current?.id ||
      container.id === visibleOverlayAncestor?.id
  );
  const diagnostics =
    hiddenOverlayIds.size === 0
      ? []
      : [
          {
            code: 'overlay_depth_exceeded',
            containerId,
            message:
              'Only the current child container and one Drawer or Modal ancestor can be restored at once.'
          }
        ];

  return { current, path, renderPath, diagnostics };
}

export function resolveChildContainerEvent(
  containers: readonly ChildContainerNode[],
  event: OpenChildContainerEvent
): {
  containerId: string | null;
  diagnostic: ChildContainerRuntimeDiagnostic | null;
} {
  if (event.name !== OPEN_CHILD_CONTAINER_EVENT) {
    return { containerId: null, diagnostic: null };
  }
  const containerId =
    typeof event.payload?.container_id === 'string' &&
    event.payload.container_id.trim().length > 0
      ? event.payload.container_id
      : null;
  if (!containerId) {
    return {
      containerId: null,
      diagnostic: {
        code: 'invalid_child_container_event',
        containerId: null,
        message: 'open_child_container requires payload.container_id.'
      }
    };
  }
  const target = containers.find((container) => container.id === containerId);
  if (!target) {
    return {
      containerId: null,
      diagnostic: unknownContainerDiagnostic(containerId)
    };
  }
  if (target.ownerBlockId !== event.sourceBlockId) {
    return {
      containerId: null,
      diagnostic: {
        code: 'child_container_owner_mismatch',
        containerId,
        message: `Block "${event.sourceBlockId}" does not own child container "${containerId}".`
      }
    };
  }
  if (!event.sourceTargetContainerIds.includes(containerId)) {
    return {
      containerId: null,
      diagnostic: {
        code: 'child_container_unregistered',
        containerId,
        message: `Block "${event.sourceBlockId}" has not registered child container "${containerId}" as a target.`
      }
    };
  }
  return { containerId, diagnostic: null };
}

export function resolveChildContainerCloseTarget(
  containers: readonly ChildContainerNode[],
  containerId: string
): string | null {
  const path = resolveChildContainerPath([...containers], containerId);
  return path && path.length > 1 ? (path.at(-2)?.id ?? null) : null;
}

export function getRootPageBlockIds(
  pageBlockIds: readonly string[],
  containers: readonly ChildContainerNode[]
): string[] {
  const assigned = new Set(
    containers.flatMap((container) => container.blockIds)
  );
  return pageBlockIds.filter((blockId) => !assigned.has(blockId));
}

function unknownContainerDiagnostic(
  containerId: string
): ChildContainerRuntimeDiagnostic {
  return {
    code: 'unknown_child_container',
    containerId,
    message: `Child container "${containerId}" does not exist.`
  };
}
