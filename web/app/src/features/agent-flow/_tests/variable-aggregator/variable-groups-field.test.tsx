import { fireEvent, render, screen } from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, test } from 'vitest';

import type { FlowVariableGroupDocument } from '@1flowbase/flow-schema';

import { VariableGroupsField } from '../../components/bindings/VariableGroupsField';
import type { FlowSelectorOption } from '../../lib/selector-options';
import { i18nText } from '../../../../shared/i18n/text';

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
  test('AC-016 edits a legal group key and allocates max groupN plus one', () => {
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

    fireEvent.change(
      screen.getByRole('textbox', {
        name: i18nText('agentFlow', 'auto.variable_group_key', { value1: 1 })
      }),
      { target: { value: 'renamed_group' } }
    );
    expect(screen.getByTestId('groups-value')).toHaveTextContent(
      'renamed_group'
    );
    fireEvent.click(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.add_variable_group')
      })
    );

    expect(screen.getByTestId('groups-value')).toHaveTextContent('group4');
  });

  test.each([
    ['', 'auto.variable_group_key_required'],
    ['bad-key', 'auto.variable_group_key_format_message'],
    ['__reserved', 'auto.variable_group_key_reserved_message']
  ] as const)(
    'AC-016 keeps invalid intermediate key %j visible with an inline error',
    (key, messageKey) => {
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

      fireEvent.change(
        screen.getByRole('textbox', {
          name: i18nText('agentFlow', 'auto.variable_group_key', { value1: 1 })
        }),
        { target: { value: key } }
      );

      expect(screen.getByTestId('groups-value')).toHaveTextContent(
        `\"key\":\"${key}\"`
      );
      expect(
        screen.getByText(i18nText('agentFlow', messageKey))
      ).toBeInTheDocument();
    }
  );

  test('AC-016 reports duplicate group keys on both edited rows', () => {
    render(
      <Harness
        initial={[
          {
            key: 'group1',
            valueType: 'string',
            candidates: [['node-source', 'text']]
          },
          {
            key: 'group2',
            valueType: 'string',
            candidates: [['node-source', 'text']]
          }
        ]}
      />
    );

    fireEvent.change(
      screen.getByRole('textbox', {
        name: i18nText('agentFlow', 'auto.variable_group_key', { value1: 2 })
      }),
      { target: { value: 'group1' } }
    );

    expect(
      screen.getAllByText(
        i18nText('agentFlow', 'auto.variable_group_keys_must_unique')
      )
    ).toHaveLength(2);
  });

  test('AC-011 preserves an incompatible selector on type change and reports its row error', () => {
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

    fireEvent.mouseDown(
      screen.getByLabelText(
        `Groups-group1-${i18nText('agentFlow', 'auto.type')}`
      )
    );
    expect(
      screen.getByText(i18nText('agentFlow', 'auto.boolean'))
    ).toBeInTheDocument();
    expect(
      screen.getByText(i18nText('agentFlow', 'auto.object'))
    ).toBeInTheDocument();
    expect(
      screen.getByText(i18nText('agentFlow', 'auto.array'))
    ).toBeInTheDocument();
    fireEvent.click(screen.getByText(i18nText('agentFlow', 'auto.number')));

    expect(screen.getByTestId('groups-value')).toHaveTextContent(
      'node-source","text'
    );
    expect(
      screen.getByText(
        i18nText('agentFlow', 'auto.variable_group_candidate_type_mismatch', {
          value1: 'number'
        })
      )
    ).toBeInTheDocument();
  });

  test('AC-011 filters selector menus by declared type and maps array wildcard declarations', async () => {
    const { rerender } = render(
      <Harness
        initial={[{ key: 'group1', valueType: 'string', candidates: [[]] }]}
      />
    );

    const stringSelector = screen.getByRole('combobox', {
      name: 'Groups-group1-1'
    });
    fireEvent.mouseDown(stringSelector);
    expect(stringSelector).toHaveAttribute('aria-expanded', 'true');
    fireEvent.click(await screen.findByText('Source'));
    expect(await screen.findByText('text')).toBeInTheDocument();
    expect(screen.queryByText('count')).toBeNull();
    expect(screen.queryByText('payload')).toBeNull();

    rerender(
      <Harness
        key="array-groups"
        initial={[{ key: 'group1', valueType: 'array', candidates: [[]] }]}
      />
    );
    const arraySelector = screen.getByRole('combobox', {
      name: 'Groups-group1-1'
    });
    fireEvent.mouseDown(arraySelector);
    expect(arraySelector).toHaveAttribute('aria-expanded', 'true');
    fireEvent.click(await screen.findByText('Source'));
    expect(await screen.findByText('items')).toBeInTheDocument();
    expect(screen.queryByText('count')).toBeNull();
    expect(screen.queryByText('payload')).toBeNull();
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
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.delete_variable_group', {
          value1: 'group1'
        })
      })
    ).toBeDisabled();
    fireEvent.click(
      screen.getAllByRole('button', {
        name: i18nText('agentFlow', 'auto.move_candidate_down')
      })[0]!
    );
    fireEvent.click(
      screen.getAllByRole('button', {
        name: i18nText('agentFlow', 'auto.delete_candidate')
      })[0]!
    );
    expect(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.delete_candidate')
      })
    ).toBeDisabled();
  });
});
