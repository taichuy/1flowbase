import { useMemo } from 'react';
import {
  Outlet,
  RouterProvider,
  createRootRoute,
  createRoute,
  createRouter
} from '@tanstack/react-router';
import { MemberSettingsTabs } from '../features/settings/pages/settings-page/MemberSettingsTabs';
export function SettingsOrganizationStyleBoundaryScene({
  tab
}: {
  tab: 'members' | 'departments';
}) {
  const router = useMemo(() => {
    window.history.replaceState({}, '', `/settings/members?tabs=${tab}`);
    const root = createRootRoute({ component: () => <Outlet /> });
    const page = createRoute({
      getParentRoute: () => root,
      path: '/settings/members',
      validateSearch: (search: Record<string, unknown>) => ({
        tabs: search.tabs === 'departments' ? 'departments' : 'members'
      }),
      component: () => <MemberSettingsTabs canManageMembers />
    });
    return createRouter({ routeTree: root.addChildren([page]) });
  }, [tab]);
  return <RouterProvider router={router} />;
}
