import { useCallback, useState } from 'react';
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor
} from '@testing-library/react';
import { ConfigProvider, Drawer, Modal } from 'antd';
import { describe, expect, it, vi } from 'vitest';
import {
  DataBlockHostContext,
  type DataBlockHost
} from '../../../../lib/data-blocks/host-context';
import type { DataFormItem } from '../../../../lib/data-blocks/types';
import { FormLayout } from '../FormLayout';

const originalItems: DataFormItem[] = [
  { id: 'name', kind: 'field', field_code: 'name', row: 'name', span: 24 },
  { id: 'status', kind: 'field', field_code: 'status', row: 'status', span: 24 }
];

function setup(presentation: 'drawer' | 'modal') {
  const closed = vi.fn();
  const changed = vi.fn();
  const activity = vi.fn();
  function Fixture() {
    const [open, setOpen] = useState(true);
    const [active, setActive] = useState(false);
    const [mounted, setMounted] = useState(true);
    const [items, setItems] = useState(originalItems);
    const [draft, setDraft] = useState('Original');
    const setDragActive = useCallback((next: boolean) => {
      activity(next);
      setActive(next);
    }, []);
    const host: DataBlockHost = {
      block_id: 'form',
      design_mode: true,
      saving: false,
      targets: [],
      setDragActive,
      saveConfig: vi.fn(),
      prepareAction: vi.fn(),
      openAction: vi.fn(),
      close: vi.fn()
    };
    const close = () => {
      closed();
      setOpen(false);
    };
    const content = (
      <>
        <button onClick={() => setMounted(false)}>Unmount form layout</button>
        {mounted && (
          <FormLayout
            items={items}
            design
            saving={false}
            onSettings={() => {}}
            onItemsChange={(next) => {
              changed(next);
              setItems(next);
            }}
            renderItem={(item) =>
              item.id === 'name' ? (
                <input
                  aria-label="Customer name"
                  value={draft}
                  onChange={(event) => setDraft(event.target.value)}
                />
              ) : (
                <span>Draft status</span>
              )
            }
          />
        )}
        <output aria-label="Stored order">
          {items.map((item) => item.id).join(',')}
        </output>
      </>
    );
    return (
      <ConfigProvider theme={{ token: { motion: false } }}>
        <DataBlockHostContext.Provider value={host}>
          {presentation === 'drawer' ? (
            <Drawer
              title="Edit customer"
              open={open}
              keyboard={!active}
              onClose={close}
              getContainer={false}
            >
              {content}
            </Drawer>
          ) : (
            <Modal
              title="Edit customer"
              open={open}
              keyboard={!active}
              onCancel={close}
              footer={null}
              getContainer={false}
            >
              {content}
            </Modal>
          )}
        </DataBlockHostContext.Provider>
      </ConfigProvider>
    );
  }
  const view = render(<Fixture />);
  return { ...view, closed, changed, activity };
}

async function startKeyboardDrag(container: HTMLElement) {
  const handle = screen.getAllByRole('button', { name: '拖动字段' })[0];
  handle.focus();
  fireEvent.keyDown(handle, { key: ' ', code: 'Space', keyCode: 32 });
  await waitFor(() =>
    expect(container.querySelector('.data-form-grid')).toHaveClass(
      'is-dragging'
    )
  );
  // The public KeyboardSensor installs its document key listener on the next task.
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
  return handle;
}
const escape = (target: HTMLElement) =>
  fireEvent.keyDown(target, { key: 'Escape', code: 'Escape', keyCode: 27 });

describe('form drag Escape ownership', () => {
  it.each(['drawer', 'modal'] as const)(
    'cancels actual DnD inside %s while keeping the draft, then lets ordinary Escape close',
    async (presentation) => {
      const fixture = setup(presentation);
      const input = screen.getByLabelText('Customer name');
      fireEvent.change(input, { target: { value: 'Unsaved customer draft' } });
      const handle = await startKeyboardDrag(fixture.container);
      expect(fixture.activity).toHaveBeenLastCalledWith(true);
      escape(handle);
      await waitFor(() =>
        expect(
          fixture.container.querySelector('.data-form-grid')
        ).not.toHaveClass('is-dragging')
      );
      expect(fixture.closed).not.toHaveBeenCalled();
      expect(fixture.changed).not.toHaveBeenCalled();
      expect(input).toHaveValue('Unsaved customer draft');
      expect(screen.getByLabelText('Stored order')).toHaveTextContent(
        'name,status'
      );
      expect(fixture.activity).toHaveBeenLastCalledWith(false);
      escape(input);
      expect(fixture.closed).toHaveBeenCalledOnce();
    }
  );

  it('releases container keyboard ownership if the form unmounts during an active drag', async () => {
    const fixture = setup('drawer');
    await startKeyboardDrag(fixture.container);
    expect(fixture.activity).toHaveBeenLastCalledWith(true);
    fireEvent.click(
      screen.getByRole('button', { name: 'Unmount form layout' })
    );
    await waitFor(() =>
      expect(fixture.activity).toHaveBeenLastCalledWith(false)
    );
    expect(fixture.changed).not.toHaveBeenCalled();
    escape(screen.getByRole('button', { name: 'Unmount form layout' }));
    expect(fixture.closed).toHaveBeenCalledOnce();
  });
});
