import { fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { App } from 'antd';
import { expect, test, vi } from 'vitest';

import { TopbarNavigationItemLabel } from '../TopbarNavigationDesigner';

const renameNode = vi.hoisted(() => vi.fn().mockResolvedValue(undefined));
vi.mock(
  '../../features/frontstage/hooks/use-frontstage-page-tree-mutations',
  () => ({
    useFrontstagePageTreeMutations: () => ({ isPending: false, renameNode })
  })
);

test.each(['page', 'group'] as const)(
  'editing a topbar %s saves metadata without changing its route slug',
  async (kind) => {
    renameNode.mockClear();
    const node = {
      id: 'topbar-route',
      title: 'route',
      kind,
      placement: 'topbar' as const,
      slug: 'route',
      icon: 'FileTextOutlined',
      tooltip: '原描述',
      children: []
    };
    render(
      <App>
        <TopbarNavigationItemLabel
          workspaceId="workspace-1"
          node={node}
          siblings={[node]}
        >
          route
        </TopbarNavigationItemLabel>
      </App>
    );
    fireEvent.click(screen.getByRole('button', { name: '配置route' }));
    fireEvent.click(await screen.findByText('编辑'));
    const dialog = await screen.findByRole('dialog', { name: '编辑顶部栏目' });
    expect(
      within(dialog).queryByRole('textbox', { name: '访问路径' })
    ).not.toBeInTheDocument();
    const [title, tooltip] = within(dialog).getAllByRole('textbox');
    expect(title).toHaveValue('route');
    fireEvent.change(title, { target: { value: 'gateway' } });
    fireEvent.change(tooltip, { target: { value: '新描述' } });
    fireEvent.click(within(dialog).getByRole('button', { name: /确\s*定/ }));
    await waitFor(() =>
      expect(renameNode).toHaveBeenCalledWith(node.id, {
        title: 'gateway',
        icon: 'FileTextOutlined',
        tooltip: '新描述'
      })
    );
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
  }
);
