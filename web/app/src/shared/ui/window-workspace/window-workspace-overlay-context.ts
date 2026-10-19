import { createContext, useContext } from 'react';

export const WindowWorkspaceOverlayZIndexContext = createContext<
  number | undefined
>(undefined);

export function useWindowWorkspaceOverlayZIndex(): number | undefined {
  return useContext(WindowWorkspaceOverlayZIndexContext);
}
