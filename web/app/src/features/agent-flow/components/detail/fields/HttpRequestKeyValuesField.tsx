/* eslint-disable react-refresh/only-export-components */

import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button } from 'antd';
import type { FlowBinding, NamedBindingEntry } from '@1flowbase/flow-schema';
import { useRef } from 'react';

import type { FlowSelectorOption } from '../../../lib/selector-options';
import { HttpRequestTemplateInput } from './HttpRequestTemplateInput';
import { i18nText } from '../../../../../shared/i18n/text';

export type HttpRequestKeyValueEntry = Pick<
  NamedBindingEntry,
  'name' | 'value' | 'valueType'
>;

export function namedBindingEntriesFromValue(
  value: unknown
): HttpRequestKeyValueEntry[] {
  if (
    typeof value === 'object' &&
    value !== null &&
    (value as { kind?: unknown }).kind === 'named_bindings' &&
    Array.isArray((value as { value?: unknown }).value)
  ) {
    return ((value as { value: unknown[] }).value as HttpRequestKeyValueEntry[]);
  }

  return [];
}

export function toNamedBinding(entries: HttpRequestKeyValueEntry[]): FlowBinding {
  return {
    kind: 'named_bindings',
    value: entries.map((entry) => ({
      name: entry.name,
      valueType: entry.valueType,
      value: entry.value ?? { kind: 'templated_text', value: '' }
    }))
  };
}

function getTemplatedValue(entry: HttpRequestKeyValueEntry) {
  return entry.value?.kind === 'templated_text' ? entry.value.value : '';
}

export function HttpRequestKeyValuesField({
  ariaLabel,
  value,
  options,
  addButtonLabel,
  onChange
}: {
  ariaLabel: string;
  value: unknown;
  options: FlowSelectorOption[];
  addButtonLabel?: string;
  onChange: (value: FlowBinding) => void;
}) {
  const entries = namedBindingEntriesFromValue(value);
  const rowKeysRef = useRef<string[]>([]);
  const nextRowKeyRef = useRef(0);

  while (rowKeysRef.current.length < entries.length) {
    rowKeysRef.current.push(`http-request-key-value-${nextRowKeyRef.current}`);
    nextRowKeyRef.current += 1;
  }

  if (rowKeysRef.current.length > entries.length) {
    rowKeysRef.current.length = entries.length;
  }

  function emit(nextEntries: HttpRequestKeyValueEntry[]) {
    onChange(toNamedBinding(nextEntries));
  }

  function deleteEntry(index: number) {
    rowKeysRef.current.splice(index, 1);
    emit(entries.filter((_, candidateIndex) => candidateIndex !== index));
  }

  return (
    <div className="agent-flow-http-request-key-values">
      {entries.map((entry, index) => (
        <div
          key={rowKeysRef.current[index]}
          className="agent-flow-http-request-key-values__row"
        >
          <HttpRequestTemplateInput
            ariaLabel={`${ariaLabel}-${index}-key`}
            label={`${ariaLabel}-${index}-key`}
            options={options}
            placeholder={i18nText('agentFlow', 'auto.field_key')}
            value={entry.name}
            onChange={(nextValue) =>
              emit(
                entries.map((candidate, candidateIndex) =>
                  candidateIndex === index
                    ? { ...candidate, name: nextValue }
                    : candidate
                )
              )
            }
          />
          <HttpRequestTemplateInput
            ariaLabel={`${ariaLabel}-${index}-value`}
            label={`${ariaLabel}-${index}-value`}
            options={options}
            value={getTemplatedValue(entry)}
            onChange={(nextValue) =>
              emit(
                entries.map((candidate, candidateIndex) =>
                  candidateIndex === index
                    ? {
                        ...candidate,
                        value: { kind: 'templated_text', value: nextValue }
                      }
                    : candidate
                )
              )
            }
          />
          <Button
            aria-label={i18nText('agentFlow', 'auto.delete_variable', {
              value1: entry.name || index + 1
            })}
            danger
            icon={<DeleteOutlined />}
            type="text"
            onClick={() => deleteEntry(index)}
          />
        </div>
      ))}
      {addButtonLabel ? (
        <Button
          icon={<PlusOutlined />}
          type="dashed"
          onClick={() =>
            emit([
              ...entries,
              { name: '', value: { kind: 'templated_text', value: '' } }
            ])
          }
        >
          {addButtonLabel}
        </Button>
      ) : null}
    </div>
  );
}
