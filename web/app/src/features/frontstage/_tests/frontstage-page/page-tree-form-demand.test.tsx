import { render, screen } from '@testing-library/react';
import { Form } from 'antd';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';

import {
  PageTreeFormModal,
  type PageTreeFormDialog
} from '../../pages/frontstage-page/page-tree-form-modal';

function Harness({
  dialog,
  pickerOpen = false
}: {
  dialog: PageTreeFormDialog | null;
  pickerOpen?: boolean;
}) {
  const [form] = Form.useForm();
  const [isPickerOpen, setPickerOpen] = useState(pickerOpen);
  return (
    <PageTreeFormModal
      dialog={dialog}
      form={form}
      iconPickerOpen={isPickerOpen}
      isOperationPending={false}
      onCancel={vi.fn()}
      onIconPickerOpenChange={setPickerOpen}
      onSubmit={vi.fn()}
    />
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

describe('PageTreeFormModal demand lifecycle', () => {
  it('MDP-001 MDP-002 keeps form and icon catalog dormant while hidden', () => {
    render(<Harness dialog={null} />);

    expect(
      screen.queryByRole('button', { name: 'auto.select_icon' })
    ).toBeNull();
    expect(screen.queryByRole('searchbox')).toBeNull();
  });

  it('MDP-002 loads the icon catalog only after explicit picker intent', async () => {
    render(<Harness dialog={dialog} />);

    await screen.findByRole('button', { name: 'auto.select_icon' });
    expect(screen.queryByRole('searchbox')).toBeNull();

    render(<Harness dialog={dialog} pickerOpen />);

    expect(await screen.findByRole('searchbox')).toBeTruthy();
  });
});
