import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import { Button, Input, Radio, Select } from 'antd';
import type { FlowBinding } from '@1flowbase/flow-schema';
import { useRef } from 'react';

import {
  HTTP_REQUEST_BODY_TYPE_OPTIONS,
  isHttpRequestBodyType,
  type HttpRequestBodyType
} from '../../../lib/http-request/contract';
import type { FlowSelectorOption } from '../../../lib/selector-options';
import { SelectorField } from '../../bindings/SelectorField';
import { HttpRequestTemplateInput } from './HttpRequestTemplateInput';
import {
  HttpRequestKeyValuesField,
  namedBindingEntriesFromValue,
  toNamedBinding,
  type HttpRequestKeyValueEntry
} from './HttpRequestKeyValuesField';
import { i18nText } from '../../../../../shared/i18n/text';

type HttpRequestFormDataKind = 'text' | 'file';

function getTemplatedBindingValue(value: unknown) {
  if (
    typeof value === 'object' &&
    value !== null &&
    (value as { kind?: unknown }).kind === 'templated_text'
  ) {
    return (value as { value?: unknown }).value as string;
  }

  return '';
}

function toTemplatedBinding(value: string): FlowBinding {
  return { kind: 'templated_text', value };
}

function getSelectorBindingValue(value: unknown) {
  if (
    typeof value === 'object' &&
    value !== null &&
    (value as { kind?: unknown }).kind === 'selector' &&
    Array.isArray((value as { value?: unknown }).value)
  ) {
    return (value as { value: string[] }).value;
  }

  return [];
}

function toSelectorBinding(value: string[]): FlowBinding {
  return { kind: 'selector', value };
}

function getBodyType(value: unknown): HttpRequestBodyType {
  return isHttpRequestBodyType(value) ? value : 'none';
}

function getFormDataKind(
  entry: HttpRequestKeyValueEntry
): HttpRequestFormDataKind {
  return entry.valueType === 'file' ? 'file' : 'text';
}

function toDefaultFormDataValue(kind: HttpRequestFormDataKind) {
  return kind === 'file'
    ? { kind: 'selector' as const, selector: [] }
    : { kind: 'templated_text' as const, value: '' };
}

function getFormDataTextValue(entry: HttpRequestKeyValueEntry) {
  return entry.value?.kind === 'templated_text' ? entry.value.value : '';
}

function getFormDataSelectorValue(entry: HttpRequestKeyValueEntry) {
  return entry.value?.kind === 'selector' ? entry.value.selector : [];
}

function isFileSelectorOption(option: FlowSelectorOption) {
  const normalizedValueType = option.valueType.toLowerCase();

  return (
    normalizedValueType === 'json' ||
    normalizedValueType.startsWith('array') ||
    normalizedValueType.includes('file')
  );
}

function formDataBinding(entries: HttpRequestKeyValueEntry[]): FlowBinding {
  return toNamedBinding(
    entries.map((entry) => ({
      ...entry,
      valueType: getFormDataKind(entry),
      value: entry.value ?? toDefaultFormDataValue(getFormDataKind(entry))
    }))
  );
}

export function HttpRequestBodyField({
  bodyType,
  bodyValue,
  urlencodedValue,
  formDataValue,
  binaryValue,
  options,
  onBodyTypeChange,
  onBodyChange,
  onUrlencodedChange,
  onFormDataChange,
  onBinaryChange
}: {
  bodyType: unknown;
  bodyValue: unknown;
  urlencodedValue: unknown;
  formDataValue: unknown;
  binaryValue: unknown;
  options: FlowSelectorOption[];
  onBodyTypeChange: (value: HttpRequestBodyType) => void;
  onBodyChange: (value: FlowBinding) => void;
  onUrlencodedChange: (value: FlowBinding) => void;
  onFormDataChange: (value: FlowBinding) => void;
  onBinaryChange: (value: FlowBinding) => void;
}) {
  const selectedBodyType = getBodyType(bodyType);
  const formDataEntries = namedBindingEntriesFromValue(formDataValue);
  const fileOptions = options.filter(isFileSelectorOption);
  const formDataRowKeysRef = useRef<string[]>([]);
  const nextFormDataRowKeyRef = useRef(0);

  while (formDataRowKeysRef.current.length < formDataEntries.length) {
    formDataRowKeysRef.current.push(
      `http-request-form-data-${nextFormDataRowKeyRef.current}`
    );
    nextFormDataRowKeyRef.current += 1;
  }

  if (formDataRowKeysRef.current.length > formDataEntries.length) {
    formDataRowKeysRef.current.length = formDataEntries.length;
  }

  function emitFormData(nextEntries: HttpRequestKeyValueEntry[]) {
    onFormDataChange(formDataBinding(nextEntries));
  }

  function deleteFormDataEntry(index: number) {
    formDataRowKeysRef.current.splice(index, 1);
    emitFormData(
      formDataEntries.filter((_, candidateIndex) => candidateIndex !== index)
    );
  }

  return (
    <div className="agent-flow-http-request-body">
      <Radio.Group
        aria-label={i18nText('agentFlow', 'auto.request_body_type')}
        className="agent-flow-http-request-body__types"
        options={[...HTTP_REQUEST_BODY_TYPE_OPTIONS]}
        value={selectedBodyType}
        onChange={(event) => onBodyTypeChange(event.target.value)}
      />
      {selectedBodyType === 'json' || selectedBodyType === 'raw' ? (
        <HttpRequestTemplateInput
          ariaLabel={i18nText('agentFlow', 'auto.request_body')}
          displayMode="block"
          label={i18nText('agentFlow', 'auto.request_body')}
          options={options}
          value={getTemplatedBindingValue(bodyValue)}
          onChange={(nextValue) => onBodyChange(toTemplatedBinding(nextValue))}
        />
      ) : null}
      {selectedBodyType === 'x-www-form-urlencoded' ? (
        <HttpRequestKeyValuesField
          addButtonLabel={i18nText('agentFlow', 'auto.add_new_form_field')}
          ariaLabel={i18nText('agentFlow', 'auto.urlencoded_form')}
          options={options}
          value={urlencodedValue}
          onChange={onUrlencodedChange}
        />
      ) : null}
      {selectedBodyType === 'form-data' ? (
        <div className="agent-flow-http-request-key-values">
          {formDataEntries.map((entry, index) => {
            const kind = getFormDataKind(entry);

            return (
              <div
                key={formDataRowKeysRef.current[index]}
                className="agent-flow-http-request-key-values__row agent-flow-http-request-key-values__row--form-data"
              >
                <Input
                  aria-label={`${i18nText('agentFlow', 'auto.form_data')}-${index}-key`}
                  placeholder={i18nText('agentFlow', 'auto.field_key')}
                  value={entry.name}
                  onChange={(event) =>
                    emitFormData(
                      formDataEntries.map((candidate, candidateIndex) =>
                        candidateIndex === index
                          ? { ...candidate, name: event.target.value }
                          : candidate
                      )
                    )
                  }
                />
                <Select
                  aria-label={`${i18nText('agentFlow', 'auto.form_data')}-${index}-type`}
                  options={[
                    {
                      value: 'text',
                      label: i18nText('agentFlow', 'auto.text')
                    },
                    { value: 'file', label: i18nText('agentFlow', 'auto.file') }
                  ]}
                  value={kind}
                  onChange={(nextKind) =>
                    emitFormData(
                      formDataEntries.map((candidate, candidateIndex) =>
                        candidateIndex === index
                          ? {
                              ...candidate,
                              valueType: nextKind,
                              value: toDefaultFormDataValue(nextKind)
                            }
                          : candidate
                      )
                    )
                  }
                />
                {kind === 'file' ? (
                  <SelectorField
                    ariaLabel={`${i18nText('agentFlow', 'auto.form_data')}-${index}-file`}
                    options={fileOptions}
                    value={getFormDataSelectorValue(entry)}
                    onChange={(nextValue) =>
                      emitFormData(
                        formDataEntries.map((candidate, candidateIndex) =>
                          candidateIndex === index
                            ? {
                                ...candidate,
                                value: {
                                  kind: 'selector',
                                  selector: nextValue as string[]
                                }
                              }
                            : candidate
                        )
                      )
                    }
                  />
                ) : (
                  <HttpRequestTemplateInput
                    ariaLabel={`${i18nText('agentFlow', 'auto.form_data')}-${index}-value`}
                    label={`${i18nText('agentFlow', 'auto.form_data')}-${index}-value`}
                    options={options}
                    placeholder={i18nText(
                      'agentFlow',
                      'auto.enter_template_content'
                    )}
                    value={getFormDataTextValue(entry)}
                    onChange={(nextValue) =>
                      emitFormData(
                        formDataEntries.map((candidate, candidateIndex) =>
                          candidateIndex === index
                            ? {
                                ...candidate,
                                value: {
                                  kind: 'templated_text',
                                  value: nextValue
                                }
                              }
                            : candidate
                        )
                      )
                    }
                  />
                )}
                <Button
                  aria-label={i18nText('agentFlow', 'auto.delete_variable', {
                    value1: entry.name || index + 1
                  })}
                  danger
                  icon={<DeleteOutlined />}
                  type="text"
                  onClick={() => deleteFormDataEntry(index)}
                />
              </div>
            );
          })}
          <Button
            icon={<PlusOutlined />}
            type="dashed"
            onClick={() =>
              emitFormData([
                ...formDataEntries,
                {
                  name: '',
                  valueType: 'text',
                  value: { kind: 'templated_text', value: '' }
                }
              ])
            }
          >
            {i18nText('agentFlow', 'auto.add_new_form_field')}
          </Button>
        </div>
      ) : null}
      {selectedBodyType === 'binary' ? (
        <SelectorField
          ariaLabel={i18nText('agentFlow', 'auto.binary_file_variable')}
          options={fileOptions}
          value={getSelectorBindingValue(binaryValue)}
          onChange={(nextValue) =>
            onBinaryChange(toSelectorBinding(nextValue as string[]))
          }
        />
      ) : null}
    </div>
  );
}
