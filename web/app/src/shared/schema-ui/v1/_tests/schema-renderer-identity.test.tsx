import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, test } from 'vitest';

import type { SchemaBlock, SchemaFieldBlock } from '../contracts/canvas-node-schema';
import {
  createRendererRegistry,
  type SchemaAdapter,
  type SchemaFieldRendererProps,
  type SchemaViewRendererProps,
  type SchemaDynamicFormRendererProps
} from '../registry/create-renderer-registry';
import { SchemaRenderer } from '../runtime/SchemaRenderer';

const adapter: SchemaAdapter = {
  getValue: (path) => `initial:${path}`,
  setValue: () => {},
  getDerived: () => null,
  dispatch: () => {}
};

function StatefulField({ block, adapter: source }: SchemaFieldRendererProps) {
  const [draft, setDraft] = useState(() => String(source.getValue(block.path)));
  return <input aria-label={block.label} value={draft}
    onChange={(event) => setDraft(event.target.value)} />;
}

function StatefulView({ block }: SchemaViewRendererProps) {
  const [draft, setDraft] = useState('view draft');
  return <input aria-label={block.title} value={draft}
    onChange={(event) => setDraft(event.target.value)} />;
}

function StatefulDynamicForm({ block }: SchemaDynamicFormRendererProps) {
  const [draft, setDraft] = useState('form draft');
  return <input aria-label={block.title} value={draft}
    onChange={(event) => setDraft(event.target.value)} />;
}

const registry = createRendererRegistry({
  fields: { text: StatefulField, alternate: StatefulField },
  views: { summary: StatefulView },
  dynamicForms: { first: StatefulDynamicForm, second: StatefulDynamicForm },
  shells: {}
});

function field(path: string, label = path): SchemaFieldBlock {
  return { kind: 'field', renderer: 'text', path, label };
}

function setup(blocks: SchemaBlock[]) {
  const view = render(<SchemaRenderer blocks={blocks} adapter={adapter} registry={registry} />);
  return (next: SchemaBlock[], source = adapter) => view.rerender(
    <SchemaRenderer blocks={next} adapter={source} registry={registry} />
  );
}

describe('schema renderer descriptor identity', () => {
  test('retains the edited, focused field through insertion, removal, reorder and value/label changes', () => {
    const update = setup([field('first'), field('edited')]);
    const edited = screen.getByLabelText('edited');
    edited.focus();
    fireEvent.change(edited, { target: { value: 'unsaved draft' } });
    update([field('inserted'), field('edited'), field('first')]);
    expect(screen.getByLabelText('edited')).toBe(edited);
    expect(edited).toHaveFocus();
    update([{ ...field('edited', '翻译后的字段'), options: [{ value: 'one', label: '一' }] }], {
      ...adapter, getValue: () => 'new server value'
    });
    expect(screen.getByLabelText('翻译后的字段')).toBe(edited);
    expect(edited).toHaveValue('unsaved draft');
    expect(edited).toHaveFocus();
    update([]);
    expect(edited).not.toBeInTheDocument();
    update([field('edited')]);
    expect(screen.getByLabelText('edited')).not.toBe(edited);
    expect(screen.getByLabelText('edited')).toHaveValue('initial:edited');
  });

  test.each(['renderer', 'path'] as const)('remounts when field %s changes even if the component implementation is shared', (identityPart) => {
    const update = setup([field('original', 'Field')]);
    const original = screen.getByLabelText('Field');
    original.focus();
    fireEvent.change(original, { target: { value: 'old draft' } });
    update([{ ...field('original', 'Field'), [identityPart]: identityPart === 'renderer' ? 'alternate' : 'other' }]);
    const replacement = screen.getByLabelText('Field');
    expect(replacement).not.toBe(original);
    expect(replacement).not.toHaveValue('old draft');
    expect(replacement).not.toHaveFocus();
  });

  test('pairs duplicate descriptors by sibling occurrence, independently of unrelated insertion', () => {
    const update = setup([field('same', 'First occurrence'), field('same', 'Second occurrence')]);
    const first = screen.getByLabelText('First occurrence');
    const second = screen.getByLabelText('Second occurrence');
    fireEvent.change(first, { target: { value: 'first draft' } });
    fireEvent.change(second, { target: { value: 'second draft' } });
    second.focus();
    update([field('unrelated'), field('same', 'One'), field('same', 'Two'), field('same', 'Three')]);
    expect(screen.getByLabelText('One')).toBe(first);
    expect(screen.getByLabelText('Two')).toBe(second);
    expect(second).toHaveFocus();
    expect(screen.getByLabelText('Three')).toHaveValue('initial:same');
    // With no physical ID, reducing the equivalence class preserves its first
    // occurrence, not a guessed match based on the changed display label.
    update([field('same', 'Remaining')]);
    expect(screen.getByLabelText('Remaining')).toBe(first);
    expect(first).toHaveValue('first draft');
    expect(second).not.toBeInTheDocument();
  });

  test('uses the same key mapping at nested section/container boundaries', () => {
    const parent = (children: SchemaBlock[], title = 'Inputs'): SchemaBlock => ({
      kind: 'section', title, blocks: [{ kind: 'inline', title: 'row', blocks: children }]
    });
    const update = setup([parent([field('first'), field('nested')])]);
    const nested = screen.getByLabelText('nested');
    nested.focus();
    fireEvent.change(nested, { target: { value: 'nested draft' } });
    update([field('outside'), parent([field('new'), field('nested', '翻译'), field('first')])]);
    expect(screen.getByLabelText('翻译')).toBe(nested);
    expect(nested).toHaveFocus();
    expect(nested).toHaveValue('nested draft');
    update([parent([field('nested')], 'Different section identity')]);
    expect(screen.getByLabelText('nested')).not.toBe(nested);
    expect(screen.getByLabelText('nested')).toHaveValue('initial:nested');
  });

  test('anonymous equal containers pair by occurrence rather than object identity', () => {
    const container = (label: string): SchemaBlock => ({ kind: 'stack', blocks: [field('same', label)] });
    const update = setup([container('Left'), container('Right')]);
    const left = screen.getByLabelText('Left');
    const right = screen.getByLabelText('Right');
    fireEvent.change(left, { target: { value: 'left draft' } });
    fireEvent.change(right, { target: { value: 'right draft' } });
    update([container('Right'), container('Left')]);
    expect(screen.getByLabelText('Right')).toBe(left);
    expect(screen.getByLabelText('Right')).toHaveValue('left draft');
    expect(screen.getByLabelText('Left')).toBe(right);
    expect(screen.getByLabelText('Left')).toHaveValue('right draft');
  });

  test('ignores translated view/form titles but respects explicit view key and form key', () => {
    const view = (key: string, title: string): SchemaBlock => ({ kind: 'view', renderer: 'summary', key, title });
    const form = (form_key: string, title: string): SchemaBlock => ({ kind: 'dynamic_form', form_key, title });
    const update = setup([view('summary', 'Summary'), form('first', 'Parameters')]);
    const summary = screen.getByLabelText('Summary');
    const parameters = screen.getByLabelText('Parameters');
    fireEvent.change(summary, { target: { value: 'edited summary' } });
    fireEvent.change(parameters, { target: { value: 'edited parameters' } });
    parameters.focus();
    update([form('first', '参数'), view('summary', '摘要')]);
    expect(screen.getByLabelText('参数')).toBe(parameters);
    expect(parameters).toHaveFocus();
    expect(parameters).toHaveValue('edited parameters');
    expect(screen.getByLabelText('摘要')).toBe(summary);
    expect(summary).toHaveValue('edited summary');
    update([form('second', '参数'), view('other', '摘要')]);
    expect(screen.getByLabelText('参数')).not.toBe(parameters);
    expect(screen.getByLabelText('摘要')).not.toBe(summary);
  });
});
