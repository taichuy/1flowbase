import { createContext, useContext } from 'react';

import type {
  WindowWorkspaceEntry,
  WindowWorkspaceRect,
  WindowWorkspaceState
} from './window-workspace-state';

export interface WindowWorkspaceController {
  state: WindowWorkspaceState;
  open(
    entry: Omit<WindowWorkspaceEntry, 'z_index' | 'maximized' | 'restore_rect'>
  ): void;
  activate(id: string): void;
  close(id: string): void;
  setDirty(id: string, dirty: boolean): void;
  setRect(id: string, rect: WindowWorkspaceRect): void;
  toggleMaximized(id: string, viewport: WindowWorkspaceRect): void;
}

export const WindowWorkspaceContext =
  createContext<WindowWorkspaceController | null>(null);

export function useWindowWorkspace(): WindowWorkspaceController {
  const workspace = useContext(WindowWorkspaceContext);
  if (!workspace) {
    throw new Error('WindowWorkspaceProvider is required.');
  }
  return workspace;
}

export function useOptionalWindowWorkspace(): WindowWorkspaceController | null {
  return useContext(WindowWorkspaceContext);
}
