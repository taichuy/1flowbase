import { useQuery } from '@tanstack/react-query';
import { Navigate } from '@tanstack/react-router';
import { Button, Result } from 'antd';
import { useTranslation } from 'react-i18next';

import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey
} from '../features/frontstage/api/page-tree';
import { LoadingState } from '../shared/ui/loading-state/LoadingState';
import { useAuthStore } from '../state/auth-store';
import {
  navigationQueryStaleTime,
  selectNavigationQueryScope
} from '../state/navigation-query-scope';

export function HomeRedirect() {
  const { t } = useTranslation('appShell');
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const navigationScope = useAuthStore(selectNavigationQueryScope);
  const pageTreeQuery = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId ?? '', navigationScope),
    queryFn: () => fetchFrontstagePageTree(workspaceId ?? ''),
    staleTime: navigationQueryStaleTime,
    enabled: Boolean(workspaceId),
    retry: false
  });

  if (!workspaceId) return <Navigate to="/me" replace />;
  if (pageTreeQuery.isPending) return <LoadingState />;
  if (pageTreeQuery.isError) {
    return (
      <Result
        status="error"
        title={t('auto.console_navigation_load_failed')}
        extra={
          <Button onClick={() => void pageTreeQuery.refetch()}>
            {t('auto.retry')}
          </Button>
        }
      />
    );
  }

  const firstPage = pageTreeQuery.data.find(
    (node) => node.placement === 'topbar' && node.slug
  );
  return <Navigate to={firstPage ? `/${firstPage.slug}` : '/me'} replace />;
}
