import { useCallback, useMemo, useState, type ReactNode } from 'react';

import {
  activateWindowWorkspaceEntry,
  closeWindowWorkspaceEntry,
  createWindowWorkspaceState,
  openWindowWorkspaceEntry,
  setWindowWorkspaceDirty,
  setWindowWorkspaceRect,
  toggleWindowWorkspaceMaximized
} from './window-workspace-state';

import {
  WindowWorkspaceContext,
  type WindowWorkspaceController
} from './window-workspace-context';

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
