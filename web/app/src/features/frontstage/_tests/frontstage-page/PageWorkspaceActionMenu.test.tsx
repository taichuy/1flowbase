import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { PageWorkspaceActionMenu } from '../../pages/frontstage-page/PageWorkspaceActionMenu';

test('AC-009 defaults the page layout selector to automatic and emits free selection', async () => {
  const onLayoutModeChange = vi.fn();

  render(
    <PageWorkspaceActionMenu
      tabsEnabled={false}
      layoutMode="auto"
      disabled={false}
      onEdit={vi.fn()}
      onTabsEnabledChange={vi.fn()}
      onLayoutModeChange={onLayoutModeChange}
    />
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
