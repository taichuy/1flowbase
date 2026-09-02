import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeAll, expect, test, vi } from 'vitest';

import { AppProviders } from '../../../../app/AppProviders';
import {
  appI18n,
  loadApplicationI18nResources
} from '../../../../shared/i18n/app-i18n';
import { PageWorkspaceActionMenu } from '../../pages/frontstage-page/PageWorkspaceActionMenu';

beforeAll(async () => {
  await loadApplicationI18nResources();
  await appI18n.changeLanguage('zh_Hans');
});

test('AC-009 defaults the page layout selector to automatic and emits free selection', async () => {
  const onLayoutModeChange = vi.fn();

  render(
    <AppProviders>
      <PageWorkspaceActionMenu
        tabsEnabled={false}
        layoutMode="auto"
        disabled={false}
        onEdit={vi.fn()}
        onRefresh={vi.fn()}
        onTabsEnabledChange={vi.fn()}
        onLayoutModeChange={onLayoutModeChange}
      />
    </AppProviders>
  );

  fireEvent.click(screen.getByRole('button', { name: '配置页面' }));
  const selector = await screen.findByRole('combobox', { name: '布局方式' });
  expect(screen.getByText('自动布局')).toBeInTheDocument();

  fireEvent.mouseDown(selector);
  fireEvent.click(await screen.findByText('自由网格'));

  await waitFor(() => {
    expect(onLayoutModeChange).toHaveBeenCalledWith('free');
  });
});

test('#1975 AC-001 exposes refresh current page from the page settings menu', async () => {
  const onRefresh = vi.fn();

  render(
    <AppProviders>
      <PageWorkspaceActionMenu
        tabsEnabled={false}
        layoutMode="auto"
        disabled={false}
        onEdit={vi.fn()}
        onRefresh={onRefresh}
        onTabsEnabledChange={vi.fn()}
        onLayoutModeChange={vi.fn()}
      />
    </AppProviders>
  );

  fireEvent.click(screen.getByRole('button', { name: '配置页面' }));
  fireEvent.click(
    await screen.findByRole('menuitem', {
      name: /刷新当前页面/
    })
  );

  expect(onRefresh).toHaveBeenCalledTimes(1);
});
