import fs from 'node:fs';
import path from 'node:path';

import { render, screen, within } from '@testing-library/react';
import { Menu } from 'antd';
import type { MenuProps } from 'antd';
import { describe, expect, test } from 'vitest';

import '../../styles/tokens.css';
import '../../styles/globals.css';
import '../app-shell.css';
import { createAccountMenuItems } from '../account-menu-items';

function getAccountPopupChildren() {
  const items = createAccountMenuItems() ?? [];
  const accountItem = items[0];

  if (
    !accountItem ||
    typeof accountItem !== 'object' ||
    !('children' in accountItem) ||
    !Array.isArray(accountItem.children)
  ) {
    return [];
  }

  return accountItem.children;
}

describe('account popup layout', () => {
  test('keeps native vertical menu row sizing', async () => {
    render(
      <div className="app-shell-account-popup">
        <Menu mode="vertical" selectable={false} items={getAccountPopupChildren()} />
      </div>
    );

    const menu = await screen.findByRole('menu');
    const [profileItem] = within(menu).getAllByRole('menuitem');
    const styles = window.getComputedStyle(profileItem);

    expect(styles.display).toBe('block');
    expect(styles.height).toBe(styles.lineHeight);
  });

  test('horizontal submenu popup does not override native row metrics', async () => {
    render(
      <Menu
        className="app-shell-account-menu"
        mode="horizontal"
        selectable={false}
        items={createAccountMenuItems()}
        openKeys={['account']}
      />
    );

    await screen.findByText('个人资料');
    expect(screen.getByText('退出登录')).toBeInTheDocument();
    expect(screen.getByText('用户')).toBeInTheDocument();

    const appShellCss = fs.readFileSync(
      path.resolve(import.meta.dirname, '../app-shell.css'),
      'utf8'
    );
    const popupItemRuleMatch = appShellCss.match(
      /\.app-shell-account-popup \.ant-menu-item,\s*\n\.app-shell-account-popup \.ant-menu-submenu-title \{([\s\S]*?)\n\}/
    );
    const popupItemRule = popupItemRuleMatch?.[1] ?? '';

    expect(popupItemRuleMatch).not.toBeNull();
    expect(popupItemRule).not.toContain('display: flex;');
    expect(popupItemRule).not.toContain('min-height:');
    expect(popupItemRule).not.toContain('line-height: 1.25;');
    expect(appShellCss).toContain('.app-shell-account-popup .ant-menu-title-content');
    expect(appShellCss).toContain('line-height: inherit;');
  });

  test('multi-workspace submenu keeps native vertical menu row sizing', async () => {
    const items =
      createAccountMenuItems(
        'Ada',
        [{ code: 'admin', name: 'Admin', scope_kind: 'workspace' }],
        'admin',
        null,
        [
          {
            id: 'workspace-1',
            name: 'Alpha Workspace',
            logo_url: null,
            introduction: '',
            is_current: true
          },
          {
            id: 'workspace-2',
            name: 'Beta Workspace',
            logo_url: null,
            introduction: '',
            is_current: false
          }
        ],
        'workspace-1'
      ) ?? [];
    const children = Reflect.get(items[0] ?? {}, 'children') as MenuProps['items'];

    render(
      <div className="app-shell-account-popup">
        <Menu mode="vertical" selectable={false} items={children} />
      </div>
    );

    const menu = await screen.findByRole('menu');
    expect(within(menu).getByText('工作空间')).toBeInTheDocument();
    const [profileItem] = within(menu).getAllByRole('menuitem');
    const styles = window.getComputedStyle(profileItem);
    expect(styles.display).toBe('block');
    expect(styles.height).toBe(styles.lineHeight);
  });
});
