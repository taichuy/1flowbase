import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { Input, Select, Modal, Button } from 'antd';
import { useState, type ComponentType } from 'react';
import { createPortal } from 'react-dom';
import { describe, expect, test, vi } from 'vitest';
import type { BlockContext } from '@1flowbase/page-protocol';
import type { FrontstagePageContent } from '../../../api/page-content';
import { PageCanvas } from '../../../components/PageCanvas';
import type { FrontstageBlockInstance } from '../../../lib/page-document';
import type {
  FrontstageNativePreparationSnapshot,
  FrontstageNativePreparedRuntime
} from '../../../lib/page-canvas/native-runtime-preparation';
import { createFrontstagePageContentFixture } from '../../frontstage-page-content-fixtures';
import { createNativePreparationSource } from '../fixtures/native-preparation-source';

async function mount(
  Block: Parameters<typeof preparation>[2],
  isDesignMode = true
) {
  const block = runtimeBlock('Keyboard');
  const select = vi.fn();
  const view = render(
    <PageCanvas
      content={pageContentWithBlocks([block])}
      isDesignMode={isDesignMode}
      runtimeBlocks={[block]}
      onSelectBlock={select}
      runtimePreparations={createNativePreparationSource([
        preparation('keyboard', 1, Block)
      ])}
    />
  );
  const host = await screen.findByTestId(
    'frontstage-native-block-root-block-1'
  );
  const root = host.shadowRoot!;
  const queries = within(root as unknown as HTMLElement);
  const slot = screen.getByTestId('block-slot-block-1');
  return { ...view, root, queries, slot, select };
}

describe('I2019 canvas keyboard ownership', () => {
  test.each([true, false])(
    'AC-001 preserves native textarea key behavior in design=%s',
    async (design) => {
      const { queries, select } = await mount(
        () => <Input.TextArea aria-label="prompt" />,
        design
      );
      const field = await queries.findByRole('textbox');
      for (const key of ['a', 'Enter', ' ']) {
        expect(fireEvent.keyDown(field, { key, composed: true })).toBe(true);
      }
      expect(select).not.toHaveBeenCalled();
    }
  );

  test.each([
    'input',
    'textarea',
    'select',
    'button',
    'contenteditable',
    'custom'
  ])('AC-002 leaves %s inside Shadow DOM to its owner', async (kind) => {
    const { queries, select } = await mount(() => (
      <div>
        <input data-testid="input" />
        <textarea data-testid="textarea" />
        <select data-testid="select">
          <option>one</option>
        </select>
        <button data-testid="button">action</button>
        <div
          data-testid="contenteditable"
          contentEditable
          suppressContentEditableWarning
        >
          edit
        </div>
        <div data-testid="custom" role="slider" tabIndex={0}>
          custom widget
        </div>
      </div>
    ));
    const target = await queries.findByTestId(kind);
    for (const key of ['Enter', ' ']) {
      const event = new KeyboardEvent('keydown', {
        key,
        bubbles: true,
        composed: true,
        cancelable: true
      });
      act(() => target.dispatchEvent(event));
      expect(event.defaultPrevented).toBe(false);
    }
    expect(select).not.toHaveBeenCalled();
  });

  test('AC-002 keyboard activation of a nested button does not select the canvas', async () => {
    const { queries, select } = await mount(() => {
      const [count, setCount] = useState(0);
      return <Button onClick={() => setCount(count + 1)}>count:{count}</Button>;
    });
    const button = await queries.findByRole('button');
    button.focus();
    fireEvent.keyDown(button, { key: 'Enter', composed: true });
    fireEvent.click(button, { detail: 0, composed: true });
    expect(button).toHaveTextContent('count:1');
    expect(select).not.toHaveBeenCalled();
  });

  test('AC-002 Select keyboard navigation stays inside the control', async () => {
    const { queries, select } = await mount(() => (
      <Select
        aria-label="model"
        defaultValue="one"
        options={[{ value: 'one' }, { value: 'two' }]}
      />
    ));
    const combo = await queries.findByRole('combobox');
    combo.focus();
    fireEvent.keyDown(combo, { key: 'ArrowDown', keyCode: 40 });
    fireEvent.keyDown(combo, { key: 'ArrowDown', keyCode: 40 });
    fireEvent.keyDown(combo, { key: 'Enter', keyCode: 13 });
    expect(
      queries.getByText('two', { selector: '[title=two]' })
    ).toBeInTheDocument();
    expect(select).not.toHaveBeenCalled();
  });

  test('AC-002/004 Portal events do not leak to the background canvas', async () => {
    const { select } = await mount(() =>
      createPortal(<textarea aria-label="portal prompt" />, document.body)
    );
    const field = await screen.findByRole('textbox', { name: 'portal prompt' });
    field.focus();
    expect(fireEvent.keyDown(field, { key: 'Enter', composed: true })).toBe(
      true
    );
    expect(fireEvent.keyDown(field, { key: ' ', composed: true })).toBe(true);
    expect(select).not.toHaveBeenCalled();
  });

  test.each([
    { conditional: false, shadow: true },
    { conditional: true, shadow: true },
    { conditional: false, shadow: false },
    { conditional: true, shadow: false }
  ])(
    'AC-004 Modal restores trigger focus %j',
    async ({ conditional, shadow }) => {
      const Block = () => {
        const [open, setOpen] = useState(false);
        return (
          <>
            <Button onClick={() => setOpen(true)}>open dialog</Button>
            {(!conditional || open) && (
              <Modal open={open} title="dialog" onCancel={() => setOpen(false)}>
                <Input.TextArea aria-label="dialog prompt" />
              </Modal>
            )}
          </>
        );
      };
      const { queries, select } = shadow
        ? await mount(Block)
        : (() => {
            render(<Block />);
            return { queries: screen, select: vi.fn() };
          })();
      const button = await queries.findByRole('button', {
        name: 'open dialog'
      });
      button.focus();
      fireEvent.click(button, { detail: 0, composed: true });
      const field = await queries.findByRole('textbox', {
        name: 'dialog prompt'
      });
      field.focus();
      expect(fireEvent.keyDown(field, { key: 'Enter', composed: true })).toBe(
        true
      );
      fireEvent.keyDown(field, { key: 'Escape', keyCode: 27, composed: true });
      await waitFor(() =>
        expect(queries.queryByRole('dialog')).not.toBeInTheDocument()
      );
      await waitFor(() =>
        expect((button.getRootNode() as ShadowRoot).activeElement).toBe(button)
      );
      expect(select).not.toHaveBeenCalled();
    }
  );

  test.each(['Enter', ' '])(
    'AC-003 selects only the focused canvas entrance with %j',
    async (key) => {
      const { slot, select } = await mount(() => null);
      slot.focus();
      expect(fireEvent.keyDown(slot, { key })).toBe(false);
      expect(select).toHaveBeenCalledExactlyOnceWith('block-1');
    }
  );

  test.each([
    { isComposing: true },
    { keyCode: 229 },
    { ctrlKey: true },
    { metaKey: true },
    { altKey: true },
    { shiftKey: true }
  ])(
    'AC-003 does not consume composition or modified keys %j',
    async (modifiers) => {
      const { slot, select } = await mount(() => null);
      slot.focus();
      expect(fireEvent.keyDown(slot, { key: 'Enter', ...modifiers })).toBe(
        true
      );
      expect(select).not.toHaveBeenCalled();
    }
  );

  test('AC-003 does not re-consume a prevented event', async () => {
    const { slot, select } = await mount(() => null);
    slot.focus();
    const event = new KeyboardEvent('keydown', {
      key: 'Enter',
      bubbles: true,
      cancelable: true
    });
    event.preventDefault();
    fireEvent(slot, event);
    expect(select).not.toHaveBeenCalled();
  });
});

function preparation(
  sourceSha256: string,
  priority: 0 | 1 | 2 | 3,
  component: ComponentType<{
    ctx: BlockContext;
    props: Record<string, unknown>;
  }>,
  present = true,
  identityOverrides: Partial<{
    compilerAbi: string;
    runtimeAbi: string;
  }> = {},
  blockId = 'block-1',
  slotIndex = 0
): Extract<FrontstageNativePreparationSnapshot, { status: 'ready' }> {
  const identityInput = {
    sourceSha256: sourceSha256.padEnd(64, '0'),
    compilerAbi: identityOverrides.compilerAbi ?? 'compiler-a',
    runtimeAbi: identityOverrides.runtimeAbi ?? 'runtime-a'
  };
  return {
    status: 'ready',
    blockId,
    slotIndex,
    priority,
    generation: 0,
    mountIntent: present ? { blockId, slotIndex, identityInput } : null,
    prepared: {
      artifact: {} as FrontstageNativePreparedRuntime['artifact'],
      component: component as FrontstageNativePreparedRuntime['component'],
      identityInput,
      artifactCacheTier: 'l2',
      moduleAssets: [],
      moduleSources: []
    }
  };
}

function runtimeBlock(
  title: string,
  blockId = 'block-1',
  order = 0
): FrontstageBlockInstance {
  return {
    id: blockId,
    rendererVersion: 'v1',
    sourceId: 'block-1',
    codeRef: 'block-1-code',
    sourceCodeRef: 'block-1',
    catalog: {
      providerCode: 'official',
      installationId: 'installation-1'
    },
    contribution: {
      pluginId: 'official.blocks',
      pluginVersion: '1.0.0',
      code: 'block-1'
    },
    runtime: {
      kind: 'native_react',
      entry: 'blocks/block-1.js',
      hint: 'native_react'
    },
    layout: { order, region: 'main' },
    presentation: { heightMode: 'auto', height: null },
    order,
    props: { title },
    ports: { inputs: [], outputs: [] }
  };
}

function pageContentWithBlocks(
  blocks: readonly FrontstageBlockInstance[]
): FrontstagePageContent {
  return createFrontstagePageContentFixture({
    root: {
      uid: 'root-1',
      payload: {
        blocks: blocks.map((block) => ({
          id: block.id,
          renderer_version: block.rendererVersion,
          codeRef: block.codeRef,
          catalog: block.catalog,
          contribution: block.contribution,
          runtime: block.runtime,
          layout: block.layout,
          presentation: block.presentation,
          props: block.props,
          ports: block.ports
        }))
      }
    }
  });
}
