import {
  createContext,
  useContext,
  useEffect,
  useState,
  type PropsWithChildren,
  type ReactNode
} from 'react';
import {
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider
} from '@tanstack/react-router';

import { publicNavigationEvent } from '../../lib/anonymous-navigation';

const Content = createContext<ReactNode>(null);
// Keep full router capabilities for custom blocks/MCP, but instantiate this
// graph only at their lazy boundary. Native history owns the anonymous shell.
function CurrentContent() {
  return useContext(Content);
}
const root = createRootRoute({
  component: CurrentContent,
  notFoundComponent: CurrentContent
});
const route = createRoute({ getParentRoute: () => root, path: '/$' });
const routeTree = root.addChildren([route]);
export function PublicAuthRouter({ children }: PropsWithChildren) {
  const [router] = useState(() => createRouter({ routeTree }));
  useEffect(
    () =>
      router.subscribe('onResolved', () => {
        window.dispatchEvent(new Event(publicNavigationEvent));
      }),
    [router]
  );
  return (
    <Content.Provider value={children}>
      <RouterProvider router={router} />
    </Content.Provider>
  );
}
