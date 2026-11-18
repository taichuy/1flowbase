import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, test, vi } from 'vitest';

import {
  DataTableFilterField,
  DataTableFilterForm,
  DataTableLayout
} from '../DataTableLayout';
import { DataTableRowActions } from '../DataTableRowActions';

describe('DataTableLayout', () => {
  test('AC-001 renders the shared filter and table regions with responsive columns', async () => {
    render(
      <DataTableLayout
        filters={
          <DataTableFilterForm
            ariaLabel="列表筛选"
            resetLabel="重置"
            submitLabel="筛选"
            onReset={vi.fn()}
            onSubmit={vi.fn()}
          >
            <DataTableFilterField label="类型">
              <input aria-label="类型" />
            </DataTableFilterField>
            <DataTableFilterField label="关键词">
              <input aria-label="关键词" />
            </DataTableFilterField>
            <DataTableFilterField label="排序">
              <input aria-label="排序" />
            </DataTableFilterField>
          </DataTableFilterForm>
        }
      >
        <div role="table">列表</div>
      </DataTableLayout>
    );

    expect(screen.getByRole('form')).toBeInTheDocument();
    expect(
      screen.queryByRole('button', { name: '展开' })
    ).not.toBeInTheDocument();
    expect(screen.getByRole('table')).toHaveTextContent('列表');

    const cssSource = await readFile(
      path.resolve(
        process.cwd(),
        'src/shared/ui/data-table/data-table-layout.css'
      ),
      'utf8'
    );

    expect(cssSource).toMatch(
      /\.data-table-filter-form\s*\{[^}]*grid-template-columns:\s*repeat\(3,\s*minmax\(0,\s*1fr\)\)/s
    );
    expect(cssSource).toMatch(
      /@media\s*\(max-width:\s*1199px\)[\s\S]*?grid-template-columns:\s*repeat\(2,\s*minmax\(0,\s*1fr\)\)/
    );
    expect(cssSource).toMatch(
      /@media\s*\(max-width:\s*767px\)[\s\S]*?grid-template-columns:\s*minmax\(0,\s*1fr\)/
    );
  });

  test('AC-002 owns expandable fields and standard filter actions', () => {
    const onReset = vi.fn();
    const onSubmit = vi.fn();

    render(
      <DataTableFilterForm
        ariaLabel="列表筛选"
        collapseLabel="收起"
        expandLabel="展开"
        expandedFields={
          <DataTableFilterField label="状态">
            <input aria-label="状态" />
          </DataTableFilterField>
        }
        resetLabel="重置"
        submitLabel="筛选"
        onReset={onReset}
        onSubmit={onSubmit}
      >
        <DataTableFilterField label="关键词">
          <input aria-label="关键词" />
        </DataTableFilterField>
      </DataTableFilterForm>
    );

    expect(
      screen.queryByRole('textbox', { name: '状态' })
    ).not.toBeInTheDocument();
    const form = screen.getByRole('form');
    expect(
      within(form)
        .getAllByRole('button')
        .map((button) => button.textContent?.replaceAll(' ', ''))
    ).toEqual(['展开', '重置', '筛选']);

    const expandButton = screen.getByRole('button', { name: /展\s*开/ });
    expect(expandButton).toHaveAttribute('aria-expanded', 'false');
    fireEvent.click(expandButton);
    expect(screen.getByRole('textbox', { name: '状态' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /收\s*起/ })).toHaveAttribute(
      'aria-expanded',
      'true'
    );

    fireEvent.click(screen.getByRole('button', { name: /重\s*置/ }));
    fireEvent.click(screen.getByRole('button', { name: /筛\s*选/ }));
    expect(onReset).toHaveBeenCalledTimes(1);
    expect(onSubmit).toHaveBeenCalledTimes(1);
  });

  test.each([0, 1, 2, 3, 4])(
    'shows at most two actions inline with %i actions',
    async (count) => {
      const handlers = Array.from({ length: count }, () => vi.fn());
      render(
        <DataTableRowActions
          moreAriaLabel="More actions"
          actions={handlers.map((onClick, index) => ({
            key: String(index),
            label: `Action ${index + 1}`,
            onClick
          }))}
        />
      );
      for (let index = 0; index < Math.min(count, 2); index++) {
        fireEvent.click(
          screen.getByRole('button', { name: `Action ${index + 1}` })
        );
        expect(handlers[index]).toHaveBeenCalledTimes(1);
      }
      if (count <= 2) {
        expect(
          screen.queryByRole('button', { name: 'More actions' })
        ).not.toBeInTheDocument();
      } else {
        fireEvent.mouseEnter(
          screen.getByRole('button', { name: 'More actions' })
        );
        await act(async () => {
          await new Promise((resolve) => setTimeout(resolve, 250));
        });
        expect(screen.queryByText('Action 3')).not.toBeInTheDocument();
        fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
        fireEvent.click(
          await screen.findByRole('menuitem', { name: 'Action 3' })
        );
        expect(handlers[2]).toHaveBeenCalledTimes(1);
        expect(handlers[0]).toHaveBeenCalledTimes(1);
      }
    }
  );

  test('preserves links and disabled actions inline and in the overflow menu', async () => {
    const disabledAction = vi.fn();
    render(
      <DataTableRowActions
        moreAriaLabel="More actions"
        actions={[
          {
            key: 'view',
            label: 'View',
            href: '/applications/example/orchestration'
          },
          {
            key: 'edit',
            label: 'Edit',
            disabled: true,
            onClick: disabledAction
          },
          {
            key: 'delete',
            label: 'Delete',
            disabled: true,
            danger: true,
            onClick: disabledAction
          }
        ]}
      />
    );
    expect(screen.getByRole('link', { name: 'View' })).toHaveAttribute(
      'href',
      '/applications/example/orchestration'
    );
    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    fireEvent.click(screen.getByRole('button', { name: 'More actions' }));
    const item = await screen.findByRole('menuitem', { name: 'Delete' });
    expect(item).toHaveAttribute('aria-disabled', 'true');
    fireEvent.click(item);
    expect(disabledAction).not.toHaveBeenCalled();
  });
});
