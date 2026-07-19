/**
 * @vitest-environment jsdom
 */
import '@testing-library/jest-dom/vitest';

import { fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, test, vi } from 'vitest';

import type { BlockUiSchema } from '@1flowbase/page-protocol';

import { BlockUiLoadingShell, BlockUiRenderer } from '../index';

afterEach(() => {
  document.body.textContent = '';
});

describe('Block UI host renderer', () => {
  test('renders a local loading shell without user-facing runtime text', () => {
    render(<BlockUiLoadingShell />);

    const shell = screen.getByTestId('block-ui-loading-shell');
    expect(shell).toHaveAttribute('aria-busy', 'true');
    expect(shell.querySelector('.ant-skeleton')).toBeInTheDocument();
    expect(shell).toHaveTextContent('');
  });

  test('renders representative Text and Stack schema after protocol validation', () => {
    render(
      <BlockUiRenderer
        schema={{
          primitive: 'Stack',
          key: 'root',
          style: {
            spacing: { gap: 'space.2', padding: 'space.3' },
            color: { background: 'surface.default' }
          },
          children: [
            { primitive: 'Title', props: { children: 'Orders' } },
            { primitive: 'Text', props: { children: '12 pending approvals' } },
            { primitive: 'Caption', props: { children: 'Updated today' } }
          ]
        }}
      />
    );

    expect(screen.getByRole('heading', { name: 'Orders' })).toBeInTheDocument();
    expect(screen.getByText('12 pending approvals')).toBeInTheDocument();
    expect(screen.getByText('Updated today')).toBeInTheDocument();
  });

  test('renders controlled Table and Form primitives without custom render props', () => {
    render(
      <BlockUiRenderer
        schema={{
          primitive: 'Stack',
          children: [
            {
              primitive: 'Table',
              props: {
                rowKey: 'id',
                columns: [
                  { key: 'name', title: 'Name', dataIndex: 'name' },
                  { key: 'status', title: 'Status', dataIndex: 'status' }
                ],
                dataSource: [
                  { id: 'record-1', name: 'Ada', status: 'Ready' },
                  { id: 'record-2', name: 'Grace', status: 'Queued' }
                ]
              }
            },
            {
              primitive: 'Form',
              props: { layout: 'vertical' },
              children: [
                {
                  primitive: 'FormItem',
                  props: { name: 'query', label: 'Query' },
                  children: [
                    { primitive: 'Input', props: { placeholder: 'Search records' } }
                  ]
                },
                {
                  primitive: 'FormItem',
                  props: { name: 'enabled', label: 'Enabled' },
                  children: [{ primitive: 'Switch', props: { checked: true } }]
                }
              ]
            }
          ]
        }}
      />
    );

    expect(screen.getByRole('columnheader', { name: 'Name' })).toBeInTheDocument();
    expect(screen.getByRole('cell', { name: 'Ada' })).toBeInTheDocument();
    expect(screen.getByLabelText('Query')).toHaveAttribute(
      'placeholder',
      'Search records'
    );
    expect(screen.getByRole('switch', { name: 'Enabled' })).toBeChecked();
  });

  test('renders invalid primitive and style as controlled error state', () => {
    const invalidPrimitive = {
      primitive: 'NativeTrustedBlock'
    };
    const invalidStyle = {
      primitive: 'Text',
      style: { position: 'absolute' }
    };

    const { rerender } = render(<BlockUiRenderer schema={invalidPrimitive} />);

    expect(screen.getByRole('alert')).toHaveTextContent('schema_invalid');
    expect(screen.getByRole('alert')).toHaveTextContent('root.primitive');

    rerender(<BlockUiRenderer schema={invalidStyle} />);
    expect(screen.getByRole('alert')).toHaveTextContent('schema_invalid');
    expect(screen.getByRole('alert')).toHaveTextContent('root.style.position');
  });

  test('does not execute function props or pass raw className and dangerous HTML through', () => {
    const leakedClick = vi.fn();
    const schema = {
      primitive: 'Button',
      props: {
        children: 'Unsafe button',
        actionId: 'record.save',
        className: 'raw-class',
        dangerouslySetInnerHTML: { __html: '<strong>unsafe</strong>' },
        onClick: leakedClick
      }
    };

    render(<BlockUiRenderer schema={schema} />);

    expect(screen.getByRole('alert')).toHaveTextContent('schema_invalid');
    expect(leakedClick).not.toHaveBeenCalled();
    expect(screen.queryByRole('button', { name: 'Unsafe button' })).not.toBeInTheDocument();
    expect(screen.queryByText('unsafe')).not.toBeInTheDocument();
  });

  test('reports Button and IconButton actions through the injected callback only', () => {
    const onAction = vi.fn();
    const schema: BlockUiSchema = {
      primitive: 'Inline',
      children: [
        {
          primitive: 'Button',
          key: 'save-button',
          props: {
            children: 'Save',
            actionId: 'record.save',
            actionPayload: { id: 'record-1' }
          }
        },
        {
          primitive: 'IconButton',
          key: 'refresh-button',
          props: {
            label: 'Refresh',
            icon: 'reload',
            actionId: 'record.refresh'
          }
        }
      ]
    };

    render(<BlockUiRenderer schema={schema} onAction={onAction} />);

    fireEvent.click(screen.getByRole('button', { name: 'Save' }));
    fireEvent.click(screen.getByRole('button', { name: 'Refresh' }));

    expect(onAction).toHaveBeenNthCalledWith(1, {
      type: 'action',
      primitive: 'Button',
      key: 'save-button',
      actionId: 'record.save',
      payload: { id: 'record-1' }
    });
    expect(onAction).toHaveBeenNthCalledWith(2, {
      type: 'action',
      primitive: 'IconButton',
      key: 'refresh-button',
      actionId: 'record.refresh'
    });
  });

  test('renders Modal in a controlled containment wrapper without a portal dependency', () => {
    render(
      <BlockUiRenderer
        schema={{
          primitive: 'Modal',
          key: 'details-modal',
          props: { title: 'Record details', open: true },
          children: [
            {
              primitive: 'Descriptions',
              props: {
                items: [{ key: 'name', label: 'Name', children: 'Ada' }]
              }
            }
          ]
        }}
      />
    );

    const region = screen.getByRole('dialog', { name: 'Record details' });
    expect(region).toHaveAttribute('data-block-renderer-modal', 'details-modal');
    expect(within(region).getByText('Name')).toBeInTheDocument();
    expect(within(region).getByText('Ada')).toBeInTheDocument();
  });
});
