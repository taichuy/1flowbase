import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { BlockHoverToolbar } from '../components/BlockHoverToolbar';

describe('BlockHoverToolbar', () => {
  test('uses the page-tree micro action language and exposes a real grid drag handle', async () => {
    const onEditCode = vi.fn();

    render(
      <BlockHoverToolbar
        blockId="orders-block"
        isVisible
        onEditCode={onEditCode}
        onDelete={vi.fn()}
      />
    );

    const actions = screen.getByTestId('frontstage-block-hover-actions');
    const actionButtons = actions.querySelectorAll(
      '.frontstage-node-action-button'
    );
    expect(actionButtons).toHaveLength(3);
    expect(
      screen.getByRole('button', { name: '移动或排序区块' })
    ).toHaveClass('frontstage-block-drag-handle');
    expect(
      screen.getByRole('button', { name: '移动或排序区块' })
        .querySelector('.anticon-drag')
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '更多区块操作' })
        .querySelector('.anticon-menu')
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole('button', { name: '编辑区块' }));
    expect(onEditCode).toHaveBeenCalledTimes(1);
    expect(
      screen.queryByRole('button', { name: '区块配置' })
    ).not.toBeInTheDocument();

    fireEvent.click(
      screen.getByRole('button', { name: '更多区块操作' })
    );
    expect(await screen.findByText('复制 UID')).toBeInTheDocument();
    expect(screen.queryByText('上移区块')).not.toBeInTheDocument();
    expect(screen.queryByText('下移区块')).not.toBeInTheDocument();
  });
});
