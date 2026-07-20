import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode
} from 'react';

import {
  activateWindowWorkspaceEntry,
  closeWindowWorkspaceEntry,
  createWindowWorkspaceState,
  openWindowWorkspaceEntry,
  setWindowWorkspaceDirty,
  setWindowWorkspaceRect,
  toggleWindowWorkspaceMaximized,
  type WindowWorkspaceEntry,
  type WindowWorkspaceRect,
  type WindowWorkspaceState
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

const WindowWorkspaceContext = createContext<WindowWorkspaceController | null>(
  null
);

export function WindowWorkspaceProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState(createWindowWorkspaceState);
  const open = useCallback<WindowWorkspaceController['open']>(
    (entry) => setState((current) => openWindowWorkspaceEntry(current, entry)),
    []
  );
  const activate = useCallback<WindowWorkspaceController['activate']>(
    (id) => setState((current) => activateWindowWorkspaceEntry(current, id)),
    []
  );
  const close = useCallback<WindowWorkspaceController['close']>(
    (id) => setState((current) => closeWindowWorkspaceEntry(current, id).state),
    []
  );
  const setDirty = useCallback<WindowWorkspaceController['setDirty']>(
    (id, dirty) =>
      setState((current) => setWindowWorkspaceDirty(current, id, dirty)),
    []
  );
  const setRect = useCallback<WindowWorkspaceController['setRect']>(
    (id, rect) =>
      setState((current) => setWindowWorkspaceRect(current, id, rect)),
    []
  );
  const toggleMaximized = useCallback<
    WindowWorkspaceController['toggleMaximized']
  >(
    (id, viewport) =>
      setState((current) =>
        toggleWindowWorkspaceMaximized(current, id, viewport)
      ),
    []
  );
  const controller = useMemo<WindowWorkspaceController>(
    () => ({
      state,
      open,
      activate,
      close,
      setDirty,
      setRect,
      toggleMaximized
    }),
    [activate, close, open, setDirty, setRect, state, toggleMaximized]
  );
  return (
    <WindowWorkspaceContext.Provider value={controller}>
      {children}
    </WindowWorkspaceContext.Provider>
  );
}

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
