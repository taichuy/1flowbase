export interface WindowWorkspaceRect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface WindowWorkspaceEntry {
  id: string;
  owner: string;
  parent_id: string | null;
  rect: WindowWorkspaceRect;
  restore_rect: WindowWorkspaceRect | null;
  z_index: number;
  maximized: boolean;
  dirty: boolean;
}

export interface WindowWorkspaceState {
  windows: WindowWorkspaceEntry[];
  next_z_index: number;
}

export function createWindowWorkspaceState(): WindowWorkspaceState {
  return { windows: [], next_z_index: 1 };
}

export function openWindowWorkspaceEntry(
  state: WindowWorkspaceState,
  entry: Omit<WindowWorkspaceEntry, 'z_index' | 'maximized' | 'restore_rect'>
): WindowWorkspaceState {
  const existing = state.windows.find((window) => window.id === entry.id);
  if (existing) return activateWindowWorkspaceEntry(state, existing.id);
  return {
    windows: [
      ...state.windows,
      {
        ...entry,
        rect: avoidInitialCollision(
          entry.rect,
          state.windows.map((window) => window.rect)
        ),
        restore_rect: null,
        maximized: false,
        z_index: state.next_z_index
      }
    ],
    next_z_index: state.next_z_index + 1
  };
}

export function activateWindowWorkspaceEntry(
  state: WindowWorkspaceState,
  id: string
): WindowWorkspaceState {
  return {
    windows: state.windows.map((window) =>
      window.id === id ? { ...window, z_index: state.next_z_index } : window
    ),
    next_z_index: state.next_z_index + 1
  };
}

export function closeWindowWorkspaceEntry(
  state: WindowWorkspaceState,
  id: string
): { state: WindowWorkspaceState; closed: WindowWorkspaceEntry[] } {
  const ids = new Set([id]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const window of state.windows) {
      if (
        window.parent_id &&
        ids.has(window.parent_id) &&
        !ids.has(window.id)
      ) {
        ids.add(window.id);
        changed = true;
      }
    }
  }
  return {
    state: {
      ...state,
      windows: state.windows.filter((window) => !ids.has(window.id))
    },
    closed: state.windows.filter((window) => ids.has(window.id))
  };
}

export function setWindowWorkspaceDirty(
  state: WindowWorkspaceState,
  id: string,
  dirty: boolean
): WindowWorkspaceState {
  return {
    ...state,
    windows: state.windows.map((window) =>
      window.id === id ? { ...window, dirty } : window
    )
  };
}

export function toggleWindowWorkspaceMaximized(
  state: WindowWorkspaceState,
  id: string,
  viewport: WindowWorkspaceRect
): WindowWorkspaceState {
  return {
    ...state,
    windows: state.windows.map((window) => {
      if (window.id !== id) return window;
      return window.maximized
        ? {
            ...window,
            maximized: false,
            rect: window.restore_rect ?? window.rect,
            restore_rect: null
          }
        : {
            ...window,
            maximized: true,
            restore_rect: window.rect,
            rect: viewport
          };
    })
  };
}

function avoidInitialCollision(
  rect: WindowWorkspaceRect,
  existing: WindowWorkspaceRect[]
): WindowWorkspaceRect {
  let candidate = rect;
  for (let index = 0; index < existing.length; index += 1) {
    if (!overlaps(candidate, existing[index])) continue;
    candidate = {
      ...candidate,
      left: candidate.left + 24,
      top: candidate.top + 24
    };
  }
  return candidate;
}

function overlaps(
  left: WindowWorkspaceRect,
  right: WindowWorkspaceRect
): boolean {
  return (
    left.left < right.left + right.width &&
    left.left + left.width > right.left &&
    left.top < right.top + right.height &&
    left.top + left.height > right.top
  );
}
