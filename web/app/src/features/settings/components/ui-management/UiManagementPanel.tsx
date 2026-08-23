import { Grid, Tabs } from 'antd';
import { useNavigate, useRouterState } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';

import { CodeTemplatesTab } from './CodeTemplatesTab';
import { ComponentRecordsTab } from './ComponentRecordsTab';

export function UiManagementPanel({ canManage }: { canManage: boolean }) {
  const { t } = useTranslation('settingsUiManagement');
  const navigate = useNavigate();
  const screens = Grid.useBreakpoint();
  const pathname = useRouterState({
    select: (state) => state.location.pathname
  });
  const active = pathname.endsWith('/components')
    ? 'components'
    : 'code-templates';
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
            key === 'components'
              ? '/settings/ui-management/components'
              : '/settings/ui-management/code-templates'
        })
      }
      items={[
        {
          key: 'code-templates',
          label: t('code_templates'),
          style: fillStyle,
          children: <CodeTemplatesTab canManage={canManage} />
        },
        {
          key: 'components',
          label: t('components'),
          style: fillStyle,
          children: <ComponentRecordsTab canManage={canManage} />
        }
      ]}
    />
  );
}
