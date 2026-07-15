import { render, screen } from '@testing-library/react';
import { fireEvent } from '@testing-library/react';
import { describe, expect, test } from 'vitest';

import { CollapseShell } from '../CollapseShell';

describe('CollapseShell', () => {
  test('renders shell items and expands detail content', async () => {
    render(
      <CollapseShell
        items={[
          {
            key: 'item-1',
            header: <div>连接配置</div>,
            children: <div>详细配置内容</div>
          }
        ]}
      />
    );

    expect(screen.getByText('连接配置')).toBeInTheDocument();
    expect(screen.queryByText('详细配置内容')).not.toBeInTheDocument();

    fireEvent.click(screen.getByText('连接配置'));

    expect(await screen.findByText('详细配置内容')).toBeInTheDocument();
  });

  test('applies compact variant class for embedded advanced sections', () => {
    const { container } = render(
      <CollapseShell
        variant="compact"
        items={[
          {
            key: 'item-1',
            header: <div>高级配置</div>,
            children: <div>详细配置内容</div>
          }
        ]}
      />
    );

    expect(container.firstChild).toHaveClass(
      'collapse-shell',
      'collapse-shell--compact'
    );
  });
});
