import { fireEvent, render, screen, within } from '@testing-library/react';
import { beforeEach, expect, test, vi } from 'vitest';
import { appI18n } from '../../../../../shared/i18n/app-i18n';
import { FrontStagePageTreeSidebar } from '../../FrontStagePageTreeSidebar';
import { createBlockI18n } from '../../../lib/runtime-i18n/translator';

beforeEach(async () => {
  localStorage.clear();
  await appI18n.changeLanguage('zh_Hans');
});

test('translates nested titles and tooltips while editing the original descriptor', async () => {
  const node = {
    id: 'account',
    title: 'Account',
    tooltip: 'Account guide',
    kind: 'page' as const
  };
  const rename = vi.fn();
  const editTooltip = vi.fn();
  const view = (locale: string) => (
    <FrontStagePageTreeSidebar
      pageTree={[
        { id: 'group', title: 'Reports', kind: 'group', children: [node] }
      ]}
      translateText={
        createBlockI18n({
          locale,
          status: 'ready',
          messages:
            locale === 'zh_Hans'
              ? {
                  Account: '账号',
                  Reports: '报表',
                  'Account guide': '账号使用说明'
                }
              : {}
        }).t
      }
      selectedPageId={null}
      canEdit
      isOperationPending={false}
      onAddGroup={vi.fn()}
      onAddPage={vi.fn()}
      onAddPageInGroup={vi.fn()}
      onRenameNode={rename}
      onUpdateNodeMetadata={vi.fn()}
      onEditNodeTooltip={editTooltip}
      onMoveNodeToPosition={vi.fn()}
      onDeleteNode={vi.fn()}
      onSelectPage={vi.fn()}
    />
  );
  const utils = render(view('zh_Hans'));
  expect(screen.getByText('报表')).toBeInTheDocument();
  const item = screen.getByTestId('frontstage-tree-node-page-Account');
  const title = within(item).getByText('账号');
  fireEvent.mouseEnter(title);
  expect(await screen.findByRole('tooltip')).toHaveTextContent('账号使用说明');
  fireEvent.mouseLeave(title);
  fireEvent.click(within(item).getByRole('button', { name: '页面操作菜单' }));
  fireEvent.click(await screen.findByRole('menuitem', { name: /编辑$/ }));
  expect(rename).toHaveBeenCalledWith(node);
  fireEvent.click(within(item).getByRole('button', { name: '页面操作菜单' }));
  fireEvent.click(await screen.findByRole('menuitem', { name: /编辑描述/ }));
  expect(editTooltip).toHaveBeenCalledWith('account', 'Account guide');
  utils.rerender(view('en_US'));
  expect(within(item).getByText('Account')).toBeInTheDocument();
  expect(screen.getByText('Reports')).toBeInTheDocument();
});
