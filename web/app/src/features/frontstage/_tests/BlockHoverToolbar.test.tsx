import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, test, vi } from 'vitest';

import { BlockHoverToolbar } from '../components/BlockHoverToolbar';

describe('BlockHoverToolbar', () => {
  test('uses the page-tree micro action language and exposes a real grid drag handle', async () => {
    const onConfigure = vi.fn();
    const onEditCode = vi.fn();

    render(
      <BlockHoverToolbar
        blockId="orders-block"
        isVisible
        canMoveUp
        canMoveDown
        onMoveUp={vi.fn()}
        onMoveDown={vi.fn()}
        onConfigure={onConfigure}
        onEditCode={onEditCode}
        onDelete={vi.fn()}
      />
    );

    const actions = screen.getByTestId('frontstage-block-hover-actions');
    const actionButtons = actions.querySelectorAll(
      '.frontstage-node-action-button'
    );
    expect(actionButtons).toHaveLength(4);
    expect(
      screen.getByRole('button', { name: '移动或排序区块' })
    ).toHaveClass('frontstage-block-drag-handle');

    fireEvent.click(screen.getByRole('button', { name: '区块配置' }));
    fireEvent.click(screen.getByRole('button', { name: '区块代码' }));
    expect(onConfigure).toHaveBeenCalledTimes(1);
    expect(onEditCode).toHaveBeenCalledTimes(1);

    fireEvent.click(
      screen.getByRole('button', { name: '更多区块操作' })
    );
    expect(await screen.findByText('上移区块')).toBeInTheDocument();
  });
});
