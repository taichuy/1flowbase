import { fireEvent, render, screen } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

import { ResizableDrawer } from '../ResizableDrawer';

const WIDTH_STORAGE_KEY_PREFIX = 'resizable-drawer:width:';

function widthStorageKey(pathname = window.location.pathname) {
  return `${WIDTH_STORAGE_KEY_PREFIX}${pathname}`;
}

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
  beforeEach(() => {
    window.localStorage.clear();
    window.history.replaceState({}, '', '/settings/model-providers/providers');
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

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

  test('AC-001 persists the last width for the current page and restores it after remount', () => {
    const { container, unmount } = render(
      <ResizableDrawer
        open
        title="供应商配置"
        defaultWidth={560}
        minWidth={480}
        maxWidth={1200}
        resizeLabel="调整供应商抽屉宽度"
        onClose={vi.fn()}
      >
        <div>provider body</div>
      </ResizableDrawer>
    );

    fireEvent.click(screen.getByRole('button', { name: '模拟原生拖拽' }));

    expect(window.localStorage.getItem(widthStorageKey())).toBe('920');

    unmount();

    const { container: remountedContainer } = render(
      <ResizableDrawer
        open
        title="供应商配置"
        defaultWidth={560}
        minWidth={480}
        maxWidth={1200}
        resizeLabel="调整供应商抽屉宽度"
        onClose={vi.fn()}
      >
        <div>provider body</div>
      </ResizableDrawer>
    );

    expect(
      remountedContainer.querySelector<HTMLElement>(
        '.ant-drawer-content-wrapper'
      )
    ).toHaveStyle({ width: '920px' });
    expect(container.querySelector('.ant-drawer-content-wrapper')).toBeNull();
  });

  test('AC-002 clamps a stored width to the current drawer range', () => {
    window.localStorage.setItem(widthStorageKey(), '1440');

    const { container } = render(
      <ResizableDrawer
        open
        title="供应商配置"
        defaultWidth={560}
        minWidth={480}
        maxWidth={1200}
        resizeLabel="调整供应商抽屉宽度"
        onClose={vi.fn()}
      >
        <div>provider body</div>
      </ResizableDrawer>
    );

    expect(
      container.querySelector<HTMLElement>('.ant-drawer-content-wrapper')
    ).toHaveStyle({ width: '1200px' });
  });

  test('AC-003 ignores a non-finite stored width', () => {
    window.localStorage.setItem(widthStorageKey(), 'Infinity');

    const { container } = render(
      <ResizableDrawer
        open
        title="供应商配置"
        defaultWidth={560}
        minWidth={480}
        maxWidth={1200}
        resizeLabel="调整供应商抽屉宽度"
        onClose={vi.fn()}
      >
        <div>provider body</div>
      </ResizableDrawer>
    );

    expect(
      container.querySelector<HTMLElement>('.ant-drawer-content-wrapper')
    ).toHaveStyle({ width: '560px' });
  });

  test('AC-004 falls back to local state when browser storage is unavailable', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('storage unavailable');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('storage unavailable');
    });

    const { container } = render(
      <ResizableDrawer
        open
        title="供应商配置"
        defaultWidth={560}
        minWidth={480}
        maxWidth={1200}
        resizeLabel="调整供应商抽屉宽度"
        onClose={vi.fn()}
      >
        <div>provider body</div>
      </ResizableDrawer>
    );

    expect(
      container.querySelector<HTMLElement>('.ant-drawer-content-wrapper')
    ).toHaveStyle({ width: '560px' });

    fireEvent.click(screen.getByRole('button', { name: '模拟原生拖拽' }));

    expect(
      container.querySelector<HTMLElement>('.ant-drawer-content-wrapper')
    ).toHaveStyle({ width: '920px' });
  });
});
