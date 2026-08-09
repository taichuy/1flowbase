import { fireEvent, render, screen, within } from '@testing-library/react';
import { readFile } from 'node:fs/promises';
import path from 'node:path';
import { describe, expect, test, vi } from 'vitest';

import {
  DataTable,
  DataTableColumnSettings,
  type DataTableColumn,
  type DataTableConfiguration
} from '../DataTable';
import { normalizeDataTableState } from '../data-table-state';

type SampleRow = {
  id: string;
  name: string;
  owner: string;
};

const columns: Array<DataTableColumn<SampleRow>> = [
  {
    key: 'name',
    title: '名称',
    dataIndex: 'name',
    width: 180,
    ellipsis: true
  },
  {
    key: 'owner',
    title: '负责人',
    dataIndex: 'owner',
    width: 140
  }
];

function createConfiguration(
  overrides?: Partial<Pick<DataTableConfiguration, 'visibleColumnKeys'>>
): DataTableConfiguration {
  return {
    visibleColumnKeys: overrides?.visibleColumnKeys ?? ['name', 'owner'],
    columnWidths: {
      name: 180,
      owner: 140
    },
    setVisibleColumnKeys: vi.fn(),
    setColumnWidths: vi.fn()
  };
}

describe('DataTable', () => {
  test('lets a fill column absorb remaining width without a resize handle', () => {
    const fillColumns = [
      {
        ...columns[0],
        sizing: 'fill' as const
      },
      columns[1]
    ] satisfies Array<DataTableColumn<SampleRow>>;

    render(
      <DataTable<SampleRow>
        columns={fillColumns}
        configuration={createConfiguration()}
        dataSource={[]}
        page={1}
        pageSize={20}
        rowKey="id"
        total={0}
        onPageChange={vi.fn()}
      />
    );

    const nameHeader = screen.getByRole('columnheader', { name: '名称' });
    const ownerHeader = screen.getByRole('columnheader', { name: '负责人' });
    const columnElements = document.querySelectorAll('colgroup col');

    expect(columnElements[0]).not.toHaveStyle({ width: '180px' });
    expect(
      nameHeader.querySelector('.data-table__header-resize-handle')
    ).toBeNull();
    expect(
      ownerHeader.querySelector('.data-table__header-resize-handle')
    ).not.toBeNull();
  });

  test('adds schema columns missing from saved widths without restoring hidden existing columns', () => {
    const state = normalizeDataTableState(
      [
        ...columns,
        {
          key: 'status',
          title: '状态',
          width: 120
        }
      ],
      {
        visibleColumnKeys: ['name'],
        columnWidths: {
          name: 180,
          owner: 140
        }
      }
    );

    expect(state.visibleColumnKeys).toEqual(['name', 'status']);
    expect(state.columnWidths).toEqual({
      name: 180,
      owner: 140,
      status: 120
    });
  });

  test('renders rows with fixed scroll shell and pagination', async () => {
    render(
      <DataTable<SampleRow>
        columns={columns}
        configuration={createConfiguration()}
        dataSource={[
          {
            id: 'row-1',
            name: '生产应用',
            owner: 'root'
          }
        ]}
        page={1}
        pageSize={20}
        rowKey="id"
        total={21}
        onPageChange={vi.fn()}
      />
    );

    expect(
      screen.getByRole('columnheader', { name: '名称' })
    ).toBeInTheDocument();
    expect(screen.getByText('生产应用')).toBeInTheDocument();
    expect(screen.getByText('共 21 条')).toBeInTheDocument();

    const cssSource = await readFile(
      path.resolve(process.cwd(), 'src/shared/ui/data-table/data-table.css'),
      'utf8'
    );

    expect(cssSource).toMatch(
      /\.data-table\s*\{[^}]*display:\s*flex;[^}]*flex-direction:\s*column;[^}]*\}/s
    );
    expect(cssSource).toMatch(
      /\.data-table__scroll-area\s*\{[^}]*overflow-x:\s*auto;[^}]*overflow-y:\s*auto;[^}]*\}/s
    );
  });

  test('supports cursor pagination without changing page-based consumers', () => {
    const onPreviousPage = vi.fn();
    const onNextPage = vi.fn();
    render(
      <DataTable<SampleRow>
        columns={columns}
        configuration={createConfiguration()}
        cursorPagination={{
          currentPage: 2,
          hasPreviousPage: true,
          hasNextPage: true,
          previousLabel: '上一页',
          nextLabel: '下一页',
          total: 45,
          onPreviousPage,
          onNextPage
        }}
        dataSource={[]}
        rowKey="id"
      />
    );

    expect(screen.getByText('共 45 条')).toBeInTheDocument();
    expect(document.querySelector('[data-current-page="2"]')).not.toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '上一页' }));
    fireEvent.click(screen.getByRole('button', { name: '下一页' }));
    expect(onPreviousPage).toHaveBeenCalledTimes(1);
    expect(onNextPage).toHaveBeenCalledTimes(1);
  });

  test('renders table actions outside the horizontally scrollable table area', () => {
    render(
      <DataTable<SampleRow>
        columns={columns}
        configuration={createConfiguration()}
        dataSource={[]}
        page={1}
        pageSize={20}
        rowKey="id"
        toolbar={<button type="button">刷新</button>}
        total={0}
        onPageChange={vi.fn()}
      />
    );

    const toolbar = document.querySelector('.data-table__toolbar');
    const scrollArea = document.querySelector('.data-table__scroll-area');

    expect(toolbar).not.toBeNull();
    expect(scrollArea).not.toBeNull();
    expect(scrollArea?.contains(toolbar)).toBe(false);
    expect(document.querySelector('.ant-table-title')).toBeNull();
    expect(
      within(toolbar as HTMLElement).getByRole('button', { name: '刷新' })
    ).toBeInTheDocument();
  });

  test('renders a shared column settings select and keeps column order from the table schema', async () => {
    const configuration = createConfiguration();

    render(
      <DataTableColumnSettings
        columns={columns}
        configuration={configuration}
      />
    );

    fireEvent.mouseDown(screen.getByRole('combobox', { name: '字段配置' }));
    fireEvent.click(
      await screen.findByText('负责人', {
        selector: '.ant-select-item-option-content'
      })
    );

    expect(configuration.setVisibleColumnKeys).toHaveBeenCalledWith(['name']);
    expect(
      within(screen.getByRole('listbox')).getByRole('option', {
        name: '名称'
      })
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: '重置默认字段' })
    ).toBeInTheDocument();
  });
});
