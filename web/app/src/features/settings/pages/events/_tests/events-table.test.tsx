import { fireEvent, render, screen, within } from '@testing-library/react';
import { ConfigProvider, Grid } from 'antd';
import enUS from 'antd/locale/en_US';
import { describe, expect, test, vi } from 'vitest';
import { EventsTable } from '../EventsTable';

const rows = Array.from({ length: 53 }, (_, index) => ({
  id: String(index + 1),
  name: `Event ${index + 1}`
}));
const columns = [
  { key: 'name', dataIndex: 'name' as const, title: 'Name', width: 180 }
];

describe('event table shared pagination', () => {
  test('shows total, changes pages and changes page size using the shared footer', async () => {
    render(
      <ConfigProvider locale={enUS}>
        <EventsTable
          columns={columns}
          rows={rows}
          rowKey="id"
          tableKey="pagination-test"
        />
      </ConfigProvider>
    );
    const table = screen.getByRole('table');
    expect(within(table).getAllByRole('row')).toHaveLength(21);
    expect(screen.getByText('Event 20')).toBeInTheDocument();
    expect(screen.queryByText('Event 21')).not.toBeInTheDocument();
    expect(
      document.querySelector('.data-table__pagination-total')
    ).toHaveTextContent('53');
    fireEvent.click(screen.getByTitle('2'));
    expect(screen.getByText('Event 21')).toBeInTheDocument();
    expect(screen.queryByText('Event 1')).not.toBeInTheDocument();
    fireEvent.mouseDown(screen.getByRole('combobox'));
    for (const size of [10, 20, 50, 100]) {
      expect(await screen.findAllByText(`${size} / page`)).not.toHaveLength(0);
    }
    fireEvent.click(screen.getByText('50 / page'));
    // Ant pagination preserves page 2 while it is valid: only the final 3 rows remain.
    expect(within(table).getAllByRole('row')).toHaveLength(4);
    expect(screen.getByText('Event 51')).toBeInTheDocument();
    expect(screen.getByText('Event 53')).toBeInTheDocument();
    fireEvent.click(screen.getByTitle('1'));
    expect(within(table).getAllByRole('row')).toHaveLength(51);
    expect(screen.getByText('Event 50')).toBeInTheDocument();
    expect(screen.queryByText('Event 51')).not.toBeInTheDocument();
  });
});

test('runtime pagination is restored from and written to the URL without changing the original tab or scope', async () => {
  window.history.replaceState(
    {},
    '',
    '/settings/events?tab=activity&scope=system&activity_page=2&activity_size=20'
  );
  const view = render(
    <ConfigProvider locale={enUS}>
      <EventsTable
        urlState
        columns={columns}
        rows={rows}
        rowKey="id"
        tableKey="activity"
      />
    </ConfigProvider>
  );
  expect(screen.getByText('Event 21')).toBeInTheDocument();
  fireEvent.click(screen.getByTitle('3'));
  expect(window.location.search).toContain('activity_page=3');
  expect(window.location.search).toContain('scope=system');
  view.unmount();
  render(
    <ConfigProvider locale={enUS}>
      <EventsTable
        urlState
        columns={columns}
        rows={rows}
        rowKey="id"
        tableKey="activity"
      />
    </ConfigProvider>
  );
  expect(screen.getByText('Event 41')).toBeInTheDocument();
  expect(screen.queryByText('Event 21')).not.toBeInTheDocument();
});

test('small-screen activity reuses shared records and loads the next slice from the same snapshot', async () => {
  const breakpoint = vi
    .spyOn(Grid, 'useBreakpoint')
    .mockReturnValue({ xs: true, sm: false, md: false });
  try {
    window.history.replaceState(
      {},
      '',
      '/settings/events?tab=activity&activity_size=10'
    );
    render(
      <ConfigProvider locale={enUS}>
        <EventsTable
          urlState
          mobileRecords="activity"
          columns={[{ ...columns[0], mobileRole: 'title' }]}
          rows={rows}
          rowKey="id"
          tableKey="activity"
        />
      </ConfigProvider>
    );
    expect(screen.queryByRole('table')).not.toBeInTheDocument();
    expect(document.querySelector('.data-table-mobile__record')).not.toBeNull();
    expect(screen.getByText('Event 1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /Load more|加载更多/ }));
    expect(window.location.search).toContain('activity_page=2');
    expect(screen.getByText('Event 1')).toBeInTheDocument();
  } finally {
    breakpoint.mockRestore();
  }
});
