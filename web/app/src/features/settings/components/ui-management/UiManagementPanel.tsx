import { lazy, Suspense } from 'react';
import { Grid, Tabs } from 'antd';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { LoadingState } from '../../../../shared/ui/loading-state/LoadingState';

import { RegisteredBlocksTab } from './RegisteredBlocksTab';

const CodeTemplatesTab = lazy(() =>
  import('./CodeTemplatesTab').then(
    ({ CodeTemplatesTab }) => ({ default: CodeTemplatesTab })
  )
);
const ComponentRecordsTab = lazy(() =>
  import('./ComponentRecordsTab').then(
    ({ ComponentRecordsTab }) => ({ default: ComponentRecordsTab })
  )
);
const RouteManagementTab = lazy(() =>
  import('./RouteManagementTab').then(
    ({ RouteManagementTab }) => ({ default: RouteManagementTab })
  )
);

export function UiManagementPanel(_props: { canManage?: boolean } = {}) {
  const { t } = useTranslation('settingsUiManagement');
  const navigate = useNavigate();
  const screens = Grid.useBreakpoint();
  const pathname = useRouterState({
    select: (state) => state.location.pathname
  });
  const active = pathname.endsWith('/components')
    ? 'components'
    : pathname.endsWith('/routes')
      ? 'routes'
      : pathname.endsWith('/code-templates')
        ? 'code-templates'
        : 'blocks';
  const fillStyle =
    screens.lg !== false ? { height: '100%', minHeight: 0 } : undefined;

  return (
    <Tabs
      className="ui-management-panel"
      styles={{ root: fillStyle, body: fillStyle, content: fillStyle }}
      activeKey={active}
      onChange={(key) =>
        navigate({
          to:
            key === 'routes'
              ? '/settings/ui-management/routes'
              : key === 'components'
                ? '/settings/ui-management/components'
                : key === 'code-templates'
                  ? '/settings/ui-management/code-templates'
                  : '/settings/ui-management/blocks'
        })
      }
      items={[
        {
          key: 'blocks',
          label: t('blocks'),
          style: fillStyle,
          children: active === 'blocks' ? <RegisteredBlocksTab /> : null
        },
        {
          key: 'code-templates',
          label: t('code_templates'),
          style: fillStyle,
          children: active === 'code-templates' ? (
            <Suspense fallback={<LoadingState compact />}>
              <CodeTemplatesTab />
            </Suspense>
          ) : null
        },
        {
          key: 'components',
          label: t('components'),
          style: fillStyle,
          children: active === 'components' ? (
            <Suspense fallback={<LoadingState compact />}>
              <ComponentRecordsTab />
            </Suspense>
          ) : null
        },
        {
          key: 'routes',
          label: t('routes'),
          style: fillStyle,
          children: active === 'routes' ? (
            <Suspense fallback={<LoadingState compact />}>
              <RouteManagementTab />
            </Suspense>
          ) : null
        }
      ]}
    />
  );
}
