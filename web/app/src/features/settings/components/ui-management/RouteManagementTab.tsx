import { useRef, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Alert, Button, Empty, Spin, Typography } from 'antd';
import { useTranslation } from 'react-i18next';
import { useAuthStore } from '../../../../state/auth-store';
import { selectNavigationQueryScope } from '../../../../state/navigation-query-scope';
import {
  fetchFrontstagePageTree,
  frontstagePageTreeQueryKey,
  type MoveFrontstageNodeInput
} from '../../../frontstage/api/page-tree';
import { useFrontstagePageTreeMutations } from '../../../frontstage/hooks/use-frontstage-page-tree-mutations';
import { NavigationMoveTree } from '../../../frontstage/components/page-tree-actions/NavigationMoveTree';
import { SettingsSectionSurface } from '../SettingsSectionSurface';

export function RouteManagementTab({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const workspaceId = useAuthStore(
    (state) => state.actor?.current_workspace_id
  );
  const scope = useAuthStore(selectNavigationQueryScope);
  const canDesign = useAuthStore(
    (state) =>
      state.actor?.effective_display_role === 'root' ||
      Boolean(state.me?.permissions.includes('frontstage.page.design'))
  );
  const editable = canManage && canDesign;
  const tree = useQuery({
    queryKey: frontstagePageTreeQueryKey(workspaceId ?? '', scope),
    queryFn: () => fetchFrontstagePageTree(workspaceId ?? ''),
    enabled: Boolean(workspaceId),
    retry: false
  });
  const mutations = useFrontstagePageTreeMutations(workspaceId ?? '');
  const moving = useRef(false);
  const [moveError, setMoveError] = useState(false);
  const move = async (id: string, input: MoveFrontstageNodeInput) => {
    if (!editable || !workspaceId || moving.current) return;
    moving.current = true;
    setMoveError(false);
    try {
      await mutations.moveNode(id, input);
    } catch {
      setMoveError(true);
    } finally {
      moving.current = false;
    }
  };
  return (
    <SettingsSectionSurface>
      <Typography.Paragraph type="secondary">
        {editable ? t('routes_hint') : t('routes_readonly')}
      </Typography.Paragraph>
      {!workspaceId ? (
        <Alert type="warning" title={t('routes_workspace_missing')} />
      ) : tree.isError ? (
        <Alert
          type="error"
          showIcon
          title={t('routes_load_failed')}
          action={
            <Button onClick={() => void tree.refetch()}>
              {t('routes_retry')}
            </Button>
          }
        />
      ) : tree.isPending ? (
        <Spin />
      ) : (
        <>
          {moveError ? (
            <Alert type="error" showIcon title={t('routes_move_failed')} />
          ) : null}
          {mutations.isPending ? (
            <Typography.Text role="status">
              {t('routes_saving')}
            </Typography.Text>
          ) : null}
          {tree.data.length ? (
            <NavigationMoveTree
              key={scope}
              pageTree={tree.data}
              disabled={!editable || mutations.isPending}
              height={480}
              onSelectMove={(id, input) => void move(id, input)}
            />
          ) : (
            <Empty description={t('routes_empty')} />
          )}
        </>
      )}
    </SettingsSectionSurface>
  );
}
