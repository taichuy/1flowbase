import { fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { ResizableDrawer } from '../ResizableDrawer';

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  return {
    ...actual,
    Drawer: ({
      children,
      defaultSize,
      maxSize,
      resizable,
      rootClassName,
      size,
      styles,
      title,
      zIndex
    }: {
      children?: ReactNode;
      defaultSize?: number | string;
      maxSize?: number;
      resizable?: boolean | { onResize?: (size: number) => void };
      rootClassName?: string;
      size?: number | string;
      styles?: {
        dragger?: React.CSSProperties;
        wrapper?: React.CSSProperties;
      };
      title?: ReactNode;
      zIndex?: number;
    }) => (
      <section
        className={rootClassName}
        data-default-size={defaultSize}
        data-dragger-left={styles?.dragger?.left}
        data-dragger-width={styles?.dragger?.width}
        data-max-size={maxSize}
        data-min-width={styles?.wrapper?.minWidth}
        data-resizable={Boolean(resizable)}
        data-z-index={zIndex}
      >
        <button
          type="button"
          onClick={() => {
            if (typeof resizable === 'object') {
              resizable.onResize?.(920);
            }
          }}
        >
          模拟原生拖拽
        </button>
        <div className="ant-drawer-content-wrapper" style={{ width: size }}>
          <div>{title}</div>
          {children}
        </div>
      </section>
    )
  };
});

describe('ResizableDrawer', () => {
  test('delegates pointer resizing to Ant Design while retaining shared width constraints', () => {
    const requestAnimationFrame = vi.spyOn(window, 'requestAnimationFrame');
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
    const drawer = container.querySelector<HTMLElement>('.resizable-drawer');

    expect(wrapper).toHaveStyle({ width: '840px' });
    expect(drawer).toHaveAttribute('data-resizable', 'true');
    expect(drawer).toHaveAttribute('data-default-size', '840');
    expect(drawer).toHaveAttribute('data-min-width', 'min(640px, 100vw)');
    expect(drawer).toHaveAttribute('data-max-size', '1200');
    expect(drawer).toHaveAttribute('data-dragger-width', '16');
    expect(drawer).toHaveAttribute('data-dragger-left', '-8');

    fireEvent.click(screen.getByRole('button', { name: '模拟原生拖拽' }));
    expect(wrapper).toHaveStyle({ width: '920px' });
    expect(handle).toHaveAttribute('aria-valuenow', '920');
    expect(requestAnimationFrame).not.toHaveBeenCalled();

    fireEvent.keyDown(handle, { key: 'Home' });
    expect(handle).toHaveAttribute('aria-valuenow', '640');
    fireEvent.keyDown(handle, { key: 'End' });
    expect(handle).toHaveAttribute('aria-valuenow', '1200');
  });
});
