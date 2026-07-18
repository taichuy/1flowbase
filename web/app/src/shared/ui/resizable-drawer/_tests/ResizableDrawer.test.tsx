import { act, fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { ResizableDrawer } from '../ResizableDrawer';

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  return {
    ...actual,
    Drawer: ({
      children,
      rootClassName,
      title,
      width
    }: {
      children?: ReactNode;
      rootClassName?: string;
      title?: ReactNode;
      width?: number | string;
    }) => (
      <section className={rootClassName}>
        <div className="ant-drawer-content-wrapper" style={{ width }}>
          <div>{title}</div>
          {children}
        </div>
      </section>
    )
  };
});

describe('ResizableDrawer', () => {
  test('owns accessible mouse and keyboard width resizing for shared drawers', async () => {
    let animationFrameCallback: FrameRequestCallback | null = null;
    vi.spyOn(window, 'requestAnimationFrame').mockImplementation((callback) => {
      animationFrameCallback = callback;
      return 41;
    });

    const { container } = render(
      <ResizableDrawer
        open
        title="JSX Studio"
        defaultWidth={840}
        minWidth={640}
        maxWidth={1200}
        resizeLabel="调整 JSX Studio 宽度"
        onClose={vi.fn()}
      >
        <div>studio body</div>
      </ResizableDrawer>
    );

    const handle = screen.getByRole('separator', {
      name: '调整 JSX Studio 宽度'
    });
    const wrapper = container.querySelector<HTMLElement>(
      '.ant-drawer-content-wrapper'
    );

    expect(wrapper).toHaveStyle({ width: '840px' });
    fireEvent.mouseDown(handle, { clientX: 500 });
    fireEvent.mouseMove(document, { clientX: 420 });
    await act(async () => animationFrameCallback?.(performance.now()));
    fireEvent.mouseUp(document);
    expect(wrapper).toHaveStyle({ width: '920px' });

    fireEvent.keyDown(handle, { key: 'Home' });
    expect(handle).toHaveAttribute('aria-valuenow', '640');
    fireEvent.keyDown(handle, { key: 'End' });
    expect(handle).toHaveAttribute('aria-valuenow', '1200');
  });
});
