import {
  fireEvent,
  render,
  screen,
  waitFor,
  within
} from '@testing-library/react';
import { useState } from 'react';
import { describe, expect, test, vi } from 'vitest';

import type { FlowVariableGroupDocument } from '@1flowbase/flow-schema';

import { VariableGroupsField } from '../../components/bindings/VariableGroupsField';
import type { FlowSelectorOption } from '../../lib/selector-options';
import { i18nText } from '../../../../shared/i18n/text';

const OPTIONS: FlowSelectorOption[] = [
  {
    nodeId: 'node-source',
    nodeLabel: 'Source',
    outputKey: 'summary',
    outputLabel: 'summary',
    valueType: 'string',
    value: ['node-source', 'summary'],
    displayLabel: 'Source/summary'
  },
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

function mockCandidateRowRects() {
  document
    .querySelectorAll<HTMLElement>('.agent-flow-variable-groups__row')
    .forEach((row, index) => {
      vi.spyOn(row, 'getBoundingClientRect').mockReturnValue({
        x: 0,
        y: index * 48,
        top: index * 48,
        right: 320,
        bottom: (index + 1) * 48,
        left: 0,
        width: 320,
        height: 48,
        toJSON: () => ({})
      });
    });
}

async function moveCandidateDownByKeyboard(handle: HTMLElement) {
  handle.focus();
  fireEvent.keyDown(handle, { key: ' ', code: 'Space' });
  await waitFor(() => expect(handle).toHaveAttribute('aria-pressed', 'true'));
  fireEvent.keyDown(handle, { key: 'ArrowDown', code: 'ArrowDown' });
  fireEvent.keyDown(handle, { key: ' ', code: 'Space' });
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

  test('AC-018 persists exact candidate order through the accessible keyboard drag handle', async () => {
    render(
      <Harness
        initial={[
          {
            key: 'group1',
            valueType: 'string',
            candidates: [
              ['node-source', 'text'],
              ['node-source', 'summary'],
              []
            ]
          }
        ]}
      />
    );
    mockCandidateRowRects();

    await moveCandidateDownByKeyboard(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.reorder_candidate', { value1: 1 })
      })
    );

    await waitFor(() =>
      expect(screen.getByTestId('groups-value')).toHaveTextContent(
        '"candidates":[["node-source","summary"],["node-source","text"],[]]'
      )
    );
  });

  test('AC-018 uses editor-local row ids when duplicate selectors are reordered', async () => {
    render(
      <Harness
        initial={[
          {
            key: 'group1',
            valueType: 'string',
            candidates: [
              ['node-source', 'text'],
              ['node-source', 'text'],
              ['node-source', 'summary']
            ]
          }
        ]}
      />
    );
    mockCandidateRowRects();

    await moveCandidateDownByKeyboard(
      screen.getByRole('button', {
        name: i18nText('agentFlow', 'auto.reorder_candidate', { value1: 2 })
      })
    );

    await waitFor(() =>
      expect(screen.getByTestId('groups-value')).toHaveTextContent(
        '"candidates":[["node-source","text"],["node-source","summary"],["node-source","text"]]'
      )
    );
    expect(screen.getByTestId('groups-value')).not.toHaveTextContent(
      'candidate-'
    );
  });

  test('AC-019/020 uses semantic sections without nested cards or move buttons', () => {
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
    const field = screen.getByTestId('variable-groups-field');
    const groupSection = within(field).getByRole('region', {
      name: 'Groups 1'
    });

    expect(groupSection).toHaveClass('agent-flow-variable-groups__item');
    expect(field.querySelector('.ant-card')).toBeNull();
    expect(field.querySelector('.anticon-arrow-up')).toBeNull();
    expect(field.querySelector('.anticon-arrow-down')).toBeNull();
    expect(
      within(field).getByRole('button', {
        name: i18nText('agentFlow', 'auto.reorder_candidate', { value1: 1 })
      })
    ).toBeInTheDocument();
    expect(
      within(field).getByRole('button', {
        name: i18nText('agentFlow', 'auto.reorder_candidate', { value1: 2 })
      })
    ).toBeInTheDocument();
    fireEvent.click(
      within(field).getAllByRole('button', {
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
