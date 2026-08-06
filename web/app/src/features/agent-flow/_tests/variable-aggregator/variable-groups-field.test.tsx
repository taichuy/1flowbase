import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, test } from 'vitest';

import type { FlowVariableGroupDocument } from '@1flowbase/flow-schema';

import { VariableGroupsField } from '../../components/bindings/VariableGroupsField';
import type { FlowSelectorOption } from '../../lib/selector-options';

const OPTIONS: FlowSelectorOption[] = [
  {
    nodeId: 'node-source',
    nodeLabel: 'Source',
    outputKey: 'text',
    outputLabel: 'text',
    valueType: 'string',
    value: ['node-source', 'text'],
    displayLabel: 'Source/text'
  },
  {
    nodeId: 'node-source',
    nodeLabel: 'Source',
    outputKey: 'count',
    outputLabel: 'count',
    valueType: 'number',
    value: ['node-source', 'count'],
    displayLabel: 'Source/count'
  },
  {
    nodeId: 'node-source',
    nodeLabel: 'Source',
    outputKey: 'items',
    outputLabel: 'items',
    valueType: 'array[object]',
    value: ['node-source', 'items'],
    displayLabel: 'Source/items'
  },
  {
    nodeId: 'node-source',
    nodeLabel: 'Source',
    outputKey: 'payload',
    outputLabel: 'payload',
    valueType: 'json',
    value: ['node-source', 'payload'],
    displayLabel: 'Source/payload'
  }
];

function Harness({ initial }: { initial: FlowVariableGroupDocument[] }) {
  const [groups, setGroups] = useState(initial);

  return (
    <>
      <VariableGroupsField
        ariaLabel="Groups"
        options={OPTIONS}
        value={groups}
        onChange={setGroups}
      />
      <output data-testid="groups-value">{JSON.stringify(groups)}</output>
    </>
  );
}

describe('VariableGroupsField', () => {
  test('AC-008 AC-009 keeps immutable group keys and allocates max groupN plus one', () => {
    render(
      <Harness
        initial={[
          {
            key: 'group1',
            valueType: 'string',
            candidates: [['node-source', 'text']]
          },
          { key: 'group3', valueType: 'array', candidates: [[]] }
        ]}
      />
    );

    expect(screen.queryByRole('textbox', { name: /group1/i })).toBeNull();
    fireEvent.click(screen.getByRole('button', { name: '添加变量组' }));

    expect(screen.getByTestId('groups-value')).toHaveTextContent('group4');
  });

  test('AC-010 preserves an incompatible selector on type change and reports its row error', () => {
    render(
      <Harness
        initial={[
          {
            key: 'group1',
            valueType: 'string',
            candidates: [['node-source', 'text']]
          }
        ]}
      />
    );

    fireEvent.mouseDown(screen.getByLabelText('Groups-group1-类型'));
    expect(screen.getByText('boolean')).toBeInTheDocument();
    expect(screen.getByText('object')).toBeInTheDocument();
    expect(screen.getByText('array')).toBeInTheDocument();
    fireEvent.click(screen.getByText('number'));

    expect(screen.getByTestId('groups-value')).toHaveTextContent(
      'node-source","text'
    );
    expect(
      screen.getByText(/现有候选变量会保留，直到你替换它/)
    ).toBeInTheDocument();
  });

  test('AC-010 filters selector menus by declared type and maps array wildcard declarations', () => {
    const { rerender } = render(
      <Harness
        initial={[{ key: 'group1', valueType: 'string', candidates: [[]] }]}
      />
    );

    fireEvent.mouseDown(screen.getByLabelText('Groups-group1-1'));
    expect(screen.getByText('text')).toBeInTheDocument();
    expect(screen.queryByText('count')).toBeNull();
    expect(screen.queryByText('payload')).toBeNull();

    rerender(
      <Harness
        key="array-groups"
        initial={[{ key: 'group1', valueType: 'array', candidates: [[]] }]}
      />
    );
    fireEvent.mouseDown(screen.getByLabelText('Groups-group1-1'));
    expect(screen.getByText('items')).toBeInTheDocument();
  });

  test('AC-011 keeps at least one group and one candidate while exposing ordered row controls', () => {
    render(
      <Harness
        initial={[
          {
            key: 'group1',
            valueType: 'string',
            candidates: [
              ['node-source', 'text'],
              ['node-source', 'text']
            ]
          }
        ]}
      />
    );

    expect(
      screen.getByRole('button', { name: '删除变量组 group1' })
    ).toBeDisabled();
    fireEvent.click(
      screen.getAllByRole('button', { name: '下移候选变量' })[0]!
    );
    fireEvent.click(
      screen.getAllByRole('button', { name: '删除候选变量' })[0]!
    );
    expect(screen.getByRole('button', { name: '删除候选变量' })).toBeDisabled();
  });
});
