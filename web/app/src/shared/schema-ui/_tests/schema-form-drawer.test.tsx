import { fireEvent, render, screen, waitFor } from '@testing-library/react';
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
      title
    }: {
      children?: ReactNode;
      extra?: ReactNode;
      footer?: ReactNode;
      onClose?: () => void;
      title?: ReactNode;
    }) => (
      <section data-testid="mock-drawer">
        <div data-testid="mock-drawer-title">{title}</div>
        <button type="button" onClick={onClose}>
          close drawer
        </button>
        {extra ? <div data-testid="mock-drawer-extra">{extra}</div> : null}
        <div>{children}</div>
        {footer ? <div data-testid="mock-drawer-footer">{footer}</div> : null}
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

  test('confirms close when the form has unsaved changes', async () => {
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
});
