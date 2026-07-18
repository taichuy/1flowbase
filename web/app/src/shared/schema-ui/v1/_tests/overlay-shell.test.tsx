import { render, screen } from '@testing-library/react';
import { type ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { SchemaDockPanel } from '../overlay-shell/SchemaDockPanel';
import { SchemaDrawerPanel } from '../overlay-shell/SchemaDrawerPanel';
import { SchemaModalPanel } from '../overlay-shell/SchemaModalPanel';

const antdMocks = vi.hoisted(() => ({
  Drawer: vi.fn(),
  Modal: vi.fn()
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  antdMocks.Drawer.mockImplementation(
    ({ children, title }: { children?: ReactNode; title?: ReactNode }) => (
      <div data-testid="mock-drawer">
        <div data-testid="mock-drawer-title">{title}</div>
        {children}
      </div>
    )
  );
  antdMocks.Modal.mockImplementation(
    ({ children, title }: { children?: ReactNode; title?: ReactNode }) => (
      <div data-testid="mock-modal">
        <div data-testid="mock-modal-title">{title}</div>
        {children}
      </div>
    )
  );

  return {
    ...actual,
    Drawer: antdMocks.Drawer,
    Modal: antdMocks.Modal
  };
});

describe('overlay shell runtime', () => {
  test('keeps dock panel width controlled by the caller and avoids internal resize chrome', () => {
    const onClose = vi.fn();

    const { rerender } = render(
      <SchemaDockPanel
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'dock_panel',
          title: '节点详情'
        }}
        width={520}
        footer={<button type="button">保存</button>}
        onClose={onClose}
      >
        <div>dock body</div>
      </SchemaDockPanel>
    );

    expect(screen.getByRole('button', { name: '保存' })).toBeInTheDocument();
    expect(screen.getByLabelText('节点详情')).toHaveStyle({ width: '520px' });
    expect(screen.queryByRole('separator', { name: '调整节点详情宽度' })).not.toBeInTheDocument();

    rerender(
      <SchemaDockPanel
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'dock_panel',
          title: '节点详情'
        }}
        width={640}
        footer={<button type="button">保存</button>}
        onClose={onClose}
      >
        <div>dock body</div>
      </SchemaDockPanel>
    );

    expect(screen.getByLabelText('节点详情')).toHaveStyle({ width: '640px' });
  });

  test('preserves drawer container and destroy semantics per consumer', () => {
    const onClose = vi.fn();

    render(
      <SchemaDrawerPanel
        open
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'drawer_panel',
          title: '历史版本',
          getContainer: false
        }}
        onClose={onClose}
      >
        <div>inline drawer body</div>
      </SchemaDrawerPanel>
    );

    expect(antdMocks.Drawer.mock.calls.at(0)?.[0]).toMatchObject({
      open: true,
      placement: 'right',
      title: '历史版本',
      getContainer: false,
      destroyOnClose: undefined
    });

    render(
      <SchemaDrawerPanel
        open
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'drawer_panel',
          title: '运行详情',
          destroyOnClose: true
        }}
        onClose={onClose}
      >
        <div>portal drawer body</div>
      </SchemaDrawerPanel>
    );

    expect(antdMocks.Drawer.mock.calls.at(1)?.[0]).toMatchObject({
      open: true,
      placement: 'right',
      title: '运行详情',
      destroyOnClose: true
    });
    expect(screen.getByText('inline drawer body')).toBeInTheDocument();
    expect(screen.getByText('portal drawer body')).toBeInTheDocument();
  });

  test('preserves modal destroyOnHidden semantics', () => {
    const onClose = vi.fn();

    render(
      <SchemaModalPanel
        open
        schema={{
          schemaVersion: '1.0.0',
          shellType: 'modal_panel',
          title: '新建应用',
          destroyOnHidden: true
        }}
        onClose={onClose}
      >
        <div>modal body</div>
      </SchemaModalPanel>
    );

    expect(antdMocks.Modal.mock.calls.at(0)?.[0]).toMatchObject({
      open: true,
      title: '新建应用',
      destroyOnHidden: true
    });
    expect(screen.getByText('modal body')).toBeInTheDocument();
  });
});
