import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { type ReactNode } from 'react';
import { describe, expect, test, vi } from 'vitest';

import { SchemaFormDrawer } from '../form-drawer/SchemaFormDrawer';
import type { PluginFormSchema } from '../contracts/plugin-form-schema';

const antdMocks = vi.hoisted(() => ({
  Drawer: vi.fn(),
  modalConfirm: vi.fn()
}));

vi.mock('antd', async () => {
  const actual = await vi.importActual<typeof import('antd')>('antd');

  antdMocks.Drawer.mockImplementation(
    ({
      children,
      extra,
      footer,
      onClose,
      rootClassName,
      title,
      width
    }: {
      children?: ReactNode;
      extra?: ReactNode;
      footer?: ReactNode;
      onClose?: () => void;
      rootClassName?: string;
      title?: ReactNode;
      width?: number | string;
    }) => (
      <section className={rootClassName} data-testid="mock-drawer-root">
        <div
          className="ant-drawer-content-wrapper"
          data-testid="mock-drawer"
          style={{ width }}
        >
          <div data-testid="mock-drawer-title">{title}</div>
          <button type="button" onClick={onClose}>
            close drawer
          </button>
          {extra ? <div data-testid="mock-drawer-extra">{extra}</div> : null}
          <div>{children}</div>
          {footer ? <div data-testid="mock-drawer-footer">{footer}</div> : null}
        </div>
      </section>
    )
  );

  return {
    ...actual,
    Drawer: antdMocks.Drawer,
    Modal: {
      ...actual.Modal,
      confirm: antdMocks.modalConfirm
    }
  };
});

const schema: PluginFormSchema = {
  schema_version: '1.0.0',
  fields: [
    {
      key: 'name',
      label: '标识',
      type: 'string',
      required: true
    },
    {
      key: 'description',
      label: '说明',
      type: 'string',
      control: 'textarea'
    },
    {
      key: 'enabled',
      label: '启用',
      type: 'boolean'
    }
  ]
};

describe('SchemaFormDrawer', () => {
  test('validates required fields before submit', async () => {
    const onSubmit = vi.fn();

    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ enabled: true }}
        onCancel={vi.fn()}
        onSubmit={onSubmit}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

    expect(onSubmit).not.toHaveBeenCalled();
    expect(await screen.findByText('标识不能为空')).toBeInTheDocument();
  });

  test('blocks submit when onBeforeSubmit returns false', async () => {
    const onBeforeSubmit = vi.fn().mockResolvedValue(false);
    const onSubmit = vi.fn();

    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ name: 'password-local', enabled: true }}
        onBeforeSubmit={onBeforeSubmit}
        onCancel={vi.fn()}
        onSubmit={onSubmit}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));

    await waitFor(() => expect(onBeforeSubmit).toHaveBeenCalled());
    expect(onSubmit).not.toHaveBeenCalled();
  });

  test('emits submit success and failure lifecycle events', async () => {
    const onSubmitSuccess = vi.fn();
    const onSubmitError = vi.fn();
    const onSubmit = vi
      .fn()
      .mockResolvedValueOnce({ saved: true })
      .mockRejectedValueOnce(new Error('保存失败'));

    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ name: 'password-local', enabled: true }}
        onCancel={vi.fn()}
        onSubmit={onSubmit}
        onSubmitError={onSubmitError}
        onSubmitSuccess={onSubmitSuccess}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => expect(onSubmitSuccess).toHaveBeenCalledWith({ saved: true }, expect.any(Object)));

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => expect(onSubmitError).toHaveBeenCalledWith(expect.any(Error), expect.any(Object)));
    expect(await screen.findByText('保存失败')).toBeInTheDocument();
  });

  test('confirms close when the form has unsaved changes', () => {
    const onCancel = vi.fn();
    antdMocks.modalConfirm.mockImplementation(({ onOk }: { onOk: () => void }) => {
      onOk();
    });

    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ name: 'password-local', enabled: true }}
        onCancel={onCancel}
        onSubmit={vi.fn()}
      />
    );

    fireEvent.change(screen.getByLabelText('标识'), {
      target: { value: 'password-updated' }
    });
    fireEvent.click(screen.getByRole('button', { name: /取\s*消/ }));

    expect(antdMocks.modalConfirm).toHaveBeenCalledWith(
      expect.objectContaining({
        title: '放弃未保存的更改？'
      })
    );
    expect(onCancel).toHaveBeenCalled();
  });

  test('exposes controlled extra action context without replacing primary submit', async () => {
    const extraAction = vi.fn(async (context) => {
      expect(context.getValues()).toMatchObject({ name: 'password-local' });
      context.setFieldValue('description', '连接正常');
      await context.submit();
    });
    const onSubmit = vi.fn();

    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ name: 'password-local', enabled: true }}
        extraActions={[
          {
            key: 'test',
            label: '测试连接',
            onClick: extraAction,
            placement: 'left'
          }
        ]}
        onCancel={vi.fn()}
        onSubmit={onSubmit}
      />
    );

    fireEvent.click(screen.getByRole('button', { name: '测试连接' }));

    await waitFor(() => expect(extraAction).toHaveBeenCalled());
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith(
      expect.objectContaining({
        description: '连接正常',
        name: 'password-local'
      }),
      expect.any(Object)
    ));
  });

  test('supports resizable drawer width without rerendering on every mouse move', async () => {
    let animationFrameCallback: FrameRequestCallback | null = null;
    const requestAnimationFrameSpy = vi
      .spyOn(window, 'requestAnimationFrame')
      .mockImplementation((callback) => {
        animationFrameCallback = callback;
        return 123;
      });
    const cancelAnimationFrameSpy = vi
      .spyOn(window, 'cancelAnimationFrame')
      .mockImplementation(() => undefined);

    try {
      render(
        <SchemaFormDrawer
          open
          resizable
          defaultWidth={520}
          minWidth={480}
          maxWidth={960}
          resizeLabel="调整配置抽屉宽度"
          title="Password 配置"
          schema={schema}
          initialValues={{ name: 'password-local', enabled: true }}
          onCancel={vi.fn()}
          onSubmit={vi.fn()}
        />
      );

      const drawerWrapper = screen.getByTestId('mock-drawer');
      const resizeHandle = screen.getByRole('separator', {
        name: '调整配置抽屉宽度'
      });

      expect(drawerWrapper).toHaveStyle({ width: '520px' });
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');

      fireEvent.mouseDown(resizeHandle, { clientX: 500 });
      fireEvent.mouseMove(document, { clientX: 460 });
      fireEvent.mouseMove(document, { clientX: 450 });

      expect(document.body).toHaveClass('resizable-drawer--resizing');
      expect(requestAnimationFrameSpy).toHaveBeenCalledTimes(1);
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');

      await act(async () => {
        animationFrameCallback?.(performance.now());
      });

      expect(drawerWrapper).toHaveStyle({ width: '570px' });
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '520');

      fireEvent.mouseUp(document);
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '570');
      expect(document.body).not.toHaveClass('resizable-drawer--resizing');

      fireEvent.keyDown(resizeHandle, { key: 'Home' });
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '480');
      fireEvent.keyDown(resizeHandle, { key: 'End' });
      expect(resizeHandle).toHaveAttribute('aria-valuenow', '960');
    } finally {
      requestAnimationFrameSpy.mockRestore();
      cancelAnimationFrameSpy.mockRestore();
    }
  });
});
