import { fireEvent, render, screen, within } from '@testing-library/react';
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
            collapseLabel="收起"
            expandLabel="展开"
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

  test('AC-004 composes primary row actions with a business-owned more menu', async () => {
    const onMoreAction = vi.fn();

    render(
      <DataTableRowActions
        moreAriaLabel="更多操作：示例"
        moreItems={[{ key: 'delete', label: '删除', danger: true }]}
        onMoreAction={onMoreAction}
      >
        <button type="button">查看</button>
        <button type="button">编辑</button>
      </DataTableRowActions>
    );

    expect(screen.getByRole('button', { name: '查看' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '编辑' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: '更多操作：示例' }));
    fireEvent.click(await screen.findByText('删除'));
    expect(onMoreAction).toHaveBeenCalledWith('delete');
  });
});
