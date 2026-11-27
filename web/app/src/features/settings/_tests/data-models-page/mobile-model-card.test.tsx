import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { fireEvent, render, screen, within } from '@testing-library/react';
import { expect, test, vi } from 'vitest';

import { DataModelTable } from '../../components/data-models/DataModelTable';
import './data-models-page.fixtures';
import { contactsModel, setDataModelsPageBreakpoint } from './support';

test('mobile card keyboard selection stays separate from the nested edit action click', () => {
  const mobile = { xs: true, md: false };
  setDataModelsPageBreakpoint(mobile);
  const select = vi.fn();
  const edit = vi.fn();
  render(
    <QueryClientProvider client={new QueryClient()}>
      <DataModelTable
        models={[contactsModel]}
        selectedSource={null}
        selectedModelId={null}
        loading={false}
        saving={false}
        onSelectModel={select}
        onEditModel={edit}
        onDeleteModel={vi.fn()}
        onCreateModel={vi.fn()}
        onUpdateModel={vi.fn()}
      />
    </QueryClientProvider>
  );
  const card = screen
    .getByText('Contacts')
    .closest<HTMLElement>('[role="button"]')!;
  fireEvent.click(within(card).getByRole('button', { name: '编辑' }));
  expect(edit).toHaveBeenCalledExactlyOnceWith(contactsModel);
  expect(select).not.toHaveBeenCalled();
  for (const key of ['Enter', ' ']) {
    select.mockClear();
    fireEvent.keyDown(card, { key });
    expect(select).toHaveBeenCalledExactlyOnceWith(contactsModel);
  }
});
