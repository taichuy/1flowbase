import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { type ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, test, vi } from 'vitest';

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

// Keep real component delays inside this fixture's lifetime, including callbacks
// scheduled by validation. Automatic RTL cleanup still owns mounted components.
beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true });
});

afterEach(async () => {
  try {
    await act(async () => {
      await vi.runOnlyPendingTimersAsync();
    });
  } finally {
    vi.useRealTimers();
  }
});

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

  test('clears omitted optional values when switching records without remounting controls', async () => {
    const onSubmit = vi.fn().mockResolvedValue(undefined);
    const props = { open: true, title: 'Edit record', schema, onSubmit };
    const { rerender } = render(
      <SchemaFormDrawer
        {...props}
        initialValues={{ name: 'first', description: 'private-old-value' }}
      />
    );
    const description = screen.getByLabelText('说明');
    expect(description).toHaveValue('private-old-value');

    rerender(
      <SchemaFormDrawer {...props} initialValues={{ name: 'second' }} />
    );

    await waitFor(() => expect(description).toHaveValue(''));
    expect(screen.getByLabelText('说明')).toBe(description);
    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() => expect(onSubmit).toHaveBeenCalled());
    expect(onSubmit.mock.calls[0][0]).toMatchObject({ name: 'second' });
    expect(onSubmit.mock.calls[0][0].description).toBeUndefined();
  });

  test('replaces nested record values without retaining omitted keys or remounting controls', async () => {
    const inspectValues = vi.fn();
    const props = {
      open: true,
      title: 'Nested record',
      schema,
      onSubmit: vi.fn(),
      extraActions: [
        {
          key: 'inspect',
          label: 'Inspect values',
          onClick: inspectValues
        }
      ]
    };
    const { rerender } = render(
      <SchemaFormDrawer
        {...props}
        initialValues={{
          name: 'first',
          profile: { name: 'old', optional: 'secret' }
        }}
      />
    );
    const name = screen.getByLabelText('标识');
    fireEvent.click(screen.getByRole('button', { name: 'Inspect values' }));
    expect(inspectValues.mock.calls[0][0].getValues().profile).toEqual({
      name: 'old',
      optional: 'secret'
    });

    rerender(
      <SchemaFormDrawer
        {...props}
        initialValues={{
          name: 'second',
          profile: { name: 'new' }
        }}
      />
    );

    await waitFor(() => expect(name).toHaveValue('second'));
    expect(screen.getByLabelText('标识')).toBe(name);
    fireEvent.click(screen.getByRole('button', { name: 'Inspect values' }));
    expect(inspectValues.mock.calls[1][0].getValues().profile).toEqual({
      name: 'new'
    });
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

  test('disables only the primary action when submit is unavailable', () => {
    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ name: 'password-local', enabled: true }}
        submitDisabled
        onCancel={vi.fn()}
        onSubmit={vi.fn()}
      />
    );

    expect(screen.getByRole('button', { name: /保\s*存/ })).toBeDisabled();
    expect(screen.getByLabelText('标识')).toBeEnabled();
    expect(screen.getByRole('button', { name: /取\s*消/ })).toBeEnabled();
  });

  test('renders supplementary content on both sides of the schema form', () => {
    render(
      <SchemaFormDrawer
        open
        title="Password 配置"
        schema={schema}
        initialValues={{ name: 'password-local', enabled: true }}
        leadingContent={<div>表单之前</div>}
        trailingContent={<div>表单之后</div>}
        onCancel={vi.fn()}
        onSubmit={vi.fn()}
      />
    );

    const leading = screen.getByText('表单之前');
    const field = screen.getByLabelText('标识');
    const trailing = screen.getByText('表单之后');
    expect(
      leading.compareDocumentPosition(field) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(
      field.compareDocumentPosition(trailing) & Node.DOCUMENT_POSITION_FOLLOWING
    ).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
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
    await waitFor(() =>
      expect(onSubmitSuccess).toHaveBeenCalledWith(
        { saved: true },
        expect.any(Object)
      )
    );

    fireEvent.click(screen.getByRole('button', { name: /保\s*存/ }));
    await waitFor(() =>
      expect(onSubmitError).toHaveBeenCalledWith(
        expect.any(Error),
        expect.any(Object)
      )
    );
    expect(await screen.findByText('保存失败')).toBeInTheDocument();
  });

  test('confirms close when the form has unsaved changes', () => {
    const onCancel = vi.fn();
    antdMocks.modalConfirm.mockImplementation(
      ({ onOk }: { onOk: () => void }) => {
        onOk();
      }
    );

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
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith(
        expect.objectContaining({
          description: '连接正常',
          name: 'password-local'
        }),
        expect.any(Object)
      )
    );
  });
});

test('shared password and multi-select controls keep scalar and array values', async () => {
  const onSubmit = vi.fn();
  render(
    <SchemaFormDrawer
      open
      title="Controls"
      schema={{
        schema_version: '1.0.0',
        fields: [
          {
            key: 'token',
            label: 'Secret token',
            type: 'string',
            control: 'password'
          },
          {
            key: 'members',
            label: 'Members',
            type: 'array',
            control: 'multi-select',
            options: [
              { label: 'Alice', value: 'alice' },
              { label: 'Bob', value: 'bob' }
            ]
          }
        ]
      }}
      initialValues={{ members: ['alice'] }}
      onSubmit={onSubmit}
    />
  );
  const token = document.querySelector('input[type="password"]')!;
  fireEvent.change(token, { target: { value: 'new-token' } });
  fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Members' }));
  fireEvent.click(await screen.findByText('Bob'));
  fireEvent.click(screen.getByRole('button', { name: /^保\s*存$/u }));
  await waitFor(() =>
    expect(onSubmit).toHaveBeenCalledWith(
      { token: 'new-token', members: ['alice', 'bob'] },
      expect.anything()
    )
  );
});
