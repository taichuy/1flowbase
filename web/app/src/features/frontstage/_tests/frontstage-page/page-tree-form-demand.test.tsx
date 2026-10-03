import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { Form } from 'antd';
import { StrictMode, useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { i18nText } from '../../../../shared/i18n/text';
import {
  PageTreeFormModal,
  type PageTreeFormDialog,
  type PageTreeFormValues
} from '../../pages/frontstage-page/page-tree-form-modal';

function Harness({
  dialog,
  pickerOpen = false,
  onSubmit = vi.fn()
}: {
  dialog: PageTreeFormDialog | null;
  pickerOpen?: boolean;
  onSubmit?: (values: PageTreeFormValues) => void;
}) {
  const [form] = Form.useForm();
  const [isPickerOpen, setPickerOpen] = useState(pickerOpen);
  return (
    <StrictMode>
      <PageTreeFormModal
        dialog={dialog}
        form={form}
        iconPickerOpen={isPickerOpen}
        isOperationPending={false}
        onCancel={vi.fn()}
        onIconPickerOpenChange={setPickerOpen}
        onSubmit={() => onSubmit(form.getFieldsValue())}
      />
    </StrictMode>
  );
}

const dialog: PageTreeFormDialog = {
  kind: 'create',
  nodeKind: 'page',
  parentId: null,
  rank: 'a',
  title: 'Create',
  initialTitle: '',
  initialIcon: '',
  initialTooltip: ''
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

describe('PageTreeFormModal demand lifecycle', () => {
  it('MDP-001 MDP-002 keeps form and icon catalog dormant while hidden', () => {
    render(<Harness dialog={null} />);

    expect(
      screen.queryByRole('button', {
        name: i18nText('frontstage', 'auto.select_icon')
      })
    ).not.toBeInTheDocument();
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();
  });

  it('MDP-002 loads the icon catalog only after a real picker click', async () => {
    render(<Harness dialog={dialog} />);

    const trigger = await screen.findByRole('button', {
      name: i18nText('frontstage', 'auto.select_icon')
    });
    expect(screen.queryByRole('searchbox')).not.toBeInTheDocument();

    fireEvent.click(trigger);

    expect(await screen.findByRole('searchbox')).toBeInTheDocument();
  });

  it('prefills existing metadata on first open and discards a cancelled draft on reopen', async () => {
    const editDialog: PageTreeFormDialog = {
      kind: 'rename',
      nodeKind: 'page',
      nodeId: 'page-logs',
      title: 'Configure page',
      initialTitle: 'Agent Flow 总日志',
      initialIcon: 'FileTextOutlined',
      initialTooltip: '全部应用的执行日志'
    };
    const onSubmit = vi.fn();
    const view = render(<Harness dialog={null} onSubmit={onSubmit} />);

    view.rerender(<Harness dialog={editDialog} onSubmit={onSubmit} />);
    const name = (await screen.findAllByRole('textbox'))[0];
    expect(name).toHaveValue(editDialog.initialTitle);
    expect(screen.getAllByRole('textbox')[1]).toHaveValue(
      editDialog.initialTooltip
    );
    fireEvent.change(name, { target: { value: '未保存的草稿' } });

    view.rerender(<Harness dialog={null} onSubmit={onSubmit} />);
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    view.rerender(<Harness dialog={editDialog} onSubmit={onSubmit} />);
    expect((await screen.findAllByRole('textbox'))[0]).toHaveValue(
      editDialog.initialTitle
    );
    fireEvent.change(screen.getAllByRole('textbox')[0], {
      target: { value: '执行总日志' }
    });
    fireEvent.click(
      screen.getByRole('button', {
        name: /确\s*定/
      })
    );
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith({
        title: '执行总日志',
        icon: editDialog.initialIcon,
        tooltip: editDialog.initialTooltip
      })
    );
  });

  it('initializes description edits and clears their values before creating a new node', async () => {
    const view = render(
      <Harness
        dialog={{
          kind: 'tooltip',
          nodeId: 'page-logs',
          title: 'Edit description',
          initialTooltip: '现有说明'
        }}
      />
    );
    expect(await screen.findByRole('textbox')).toHaveValue('现有说明');

    view.rerender(<Harness dialog={null} />);
    await waitFor(() =>
      expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    );
    view.rerender(
      <Harness
        dialog={{ ...dialog, showSlug: true, initialSlug: 'new-page' }}
      />
    );
    const fields = await screen.findAllByRole('textbox');
    expect(fields[0]).toHaveValue('');
    expect(fields[1]).toHaveValue('new-page');
    expect(fields[2]).toHaveValue('');
  });
});
