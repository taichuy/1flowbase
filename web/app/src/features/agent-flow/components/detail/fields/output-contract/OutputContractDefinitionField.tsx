import {
  DeleteOutlined,
  FileTextOutlined,
  PlusOutlined
} from '@ant-design/icons';
import { Button, Empty, Input, Select, Tooltip, Typography } from 'antd';
import { useRef, useState } from 'react';

import type { FlowNodeDocument } from '@1flowbase/flow-schema';
import { useStableListItemKeys } from '../../../../hooks/interactions/use-stable-list-item-keys';
import { outputTypeSupportsJsonSchema } from '../../../../lib/output-contract/schema';
import { isOutputVariableKeyAllowed } from '../../../../lib/output-contract/variable-key';
import { i18nText } from '../../../../../../shared/i18n/text';
import { CollectionFieldHeader } from '../../collection/CollectionFieldHeader';
import { JsonSchemaSettingsPanel } from '../json-schema/JsonSchemaSettingsPanel';
import {
  createDefaultJsonSchema,
  type JsonSchemaRootType
} from '../json-schema/json-schema-utils';

const valueTypeOptions = [
  { value: 'string', label: 'String' },
  { value: 'number', label: 'Number' },
  { value: 'boolean', label: 'Boolean' },
  { value: 'object', label: 'Object' },
  { value: 'array', label: 'Array' },
  { value: 'json', label: 'JSON' },
  { value: 'protocol_context', label: 'Protocol Context' },
  { value: 'unknown', label: 'Unknown' }
] satisfies Array<{
  value: FlowNodeDocument['outputs'][number]['valueType'];
  label: string;
}>;

function createNextOutput(
  index: number,
  selectorForKey?: (key: string) => string[] | undefined
): FlowNodeDocument['outputs'][number] {
  const key = `output_${index + 1}`;

  return {
    key,
    title: key,
    valueType: 'string',
    selector: selectorForKey?.(key)
  };
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function fallbackSchemaRootType(
  output: FlowNodeDocument['outputs'][number]
): JsonSchemaRootType {
  return output.valueType === 'array' ? 'array' : 'object';
}

function defaultSchemaForOutput(
  output: FlowNodeDocument['outputs'][number]
): Record<string, unknown> {
  if (isRecord(output.jsonSchema)) {
    return output.jsonSchema;
  }

  return createDefaultJsonSchema(fallbackSchemaRootType(output));
}

export function OutputContractDefinitionField({
  title,
  value,
  onChange,
  syncTitleWithKey = false,
  selectorForKey
}: {
  title: string;
  value: FlowNodeDocument['outputs'];
  onChange: (value: FlowNodeDocument['outputs']) => void;
  syncTitleWithKey?: boolean;
  selectorForKey?: (key: string) => string[] | undefined;
}) {
  const [editingIndex, setEditingIndex] = useState<number | null>(null);
  const schemaTriggerRef = useRef<HTMLElement | null>(null);
  const editingOutput =
    editingIndex === null ? null : (value[editingIndex] ?? null);
  const { itemKeys, insertItemKey, removeItemKey } = useStableListItemKeys(
    'output-contract',
    value.length
  );

  function openSchemaEditor(index: number, trigger: HTMLElement | null) {
    schemaTriggerRef.current = trigger;
    setEditingIndex(index);
  }

  function closeSchemaEditor() {
    setEditingIndex(null);
  }

  function emitChange(nextValue: FlowNodeDocument['outputs']) {
    onChange(
      nextValue.map((output) => ({
        ...output,
        title: syncTitleWithKey ? output.key : output.title,
        selector: selectorForKey ? selectorForKey(output.key) : output.selector
      }))
    );
  }

  function saveSchema(schema: Record<string, unknown>) {
    if (editingIndex === null) {
      return;
    }

    const nextType = schema.type;
    emitChange(
      value.map((candidate, candidateIndex) => {
        if (candidateIndex !== editingIndex) {
          return candidate;
        }

        return {
          ...candidate,
          valueType:
            nextType === 'object' || nextType === 'array'
              ? nextType
              : candidate.valueType,
          jsonSchema: schema
        };
      })
    );
    closeSchemaEditor();
  }

  return (
    <div className="agent-flow-output-contract-editor">
      <CollectionFieldHeader
        title={title}
        description={i18nText(
          'agentFlow',
          'auto.variables_produced_nodes_referenced_downstream_nodes'
        )}
        actions={
          <Button
            aria-label={i18nText('agentFlow', 'auto.add_new_output_variable')}
            icon={<PlusOutlined />}
            size="small"
            type="text"
            onClick={() => {
              insertItemKey(value.length);
              emitChange([
                ...value,
                createNextOutput(value.length, selectorForKey)
              ]);
            }}
          />
        }
      />
      {value.length > 0 ? (
        <div className="agent-flow-output-contract-editor__list">
          {value.map((output, index) => {
            const outputKeyIsValid =
              output.key.length === 0 || isOutputVariableKeyAllowed(output.key);

            return (
              <div
                key={itemKeys[index]}
                className={`agent-flow-output-contract-editor__row${
                  syncTitleWithKey
                    ? ' agent-flow-output-contract-editor__row--synced-title'
                    : ''
                }`}
              >
                <label className="agent-flow-output-contract-editor__cell">
                  <span>{i18nText('agentFlow', 'auto.variable_name')}</span>
                  <Input
                    aria-label={i18nText(
                      'agentFlow',
                      'auto.output_variable_name',
                      { value1: index + 1 }
                    )}
                    status={outputKeyIsValid ? undefined : 'error'}
                    value={output.key}
                    onChange={(event) =>
                      emitChange(
                        value.map((candidate, candidateIndex) =>
                          candidateIndex === index
                            ? {
                                ...candidate,
                                key: event.target.value,
                                title: syncTitleWithKey
                                  ? event.target.value
                                  : candidate.title
                              }
                            : candidate
                        )
                      )
                    }
                  />
                  {!outputKeyIsValid ? (
                    <Typography.Text type="danger">
                      {i18nText(
                        'agentFlow',
                        'auto.output_variable_name_format_hint'
                      )}
                    </Typography.Text>
                  ) : null}
                </label>
                {!syncTitleWithKey ? (
                  <label className="agent-flow-output-contract-editor__cell">
                    <span>{i18nText('agentFlow', 'auto.display_name')}</span>
                    <Input
                      aria-label={i18nText(
                        'agentFlow',
                        'auto.output_display_name',
                        { value1: index + 1 }
                      )}
                      value={output.title}
                      onChange={(event) =>
                        emitChange(
                          value.map((candidate, candidateIndex) =>
                            candidateIndex === index
                              ? { ...candidate, title: event.target.value }
                              : candidate
                          )
                        )
                      }
                    />
                  </label>
                ) : null}
                <label className="agent-flow-output-contract-editor__cell">
                  <span>{i18nText('agentFlow', 'auto.type')}</span>
                  <Select
                    aria-label={i18nText('agentFlow', 'auto.output_type', {
                      value1: index + 1
                    })}
                    options={valueTypeOptions}
                    value={output.valueType}
                    onChange={(valueType) =>
                      emitChange(
                        value.map((candidate, candidateIndex) =>
                          candidateIndex === index
                            ? {
                                ...candidate,
                                valueType,
                                jsonSchema: outputTypeSupportsJsonSchema(
                                  valueType
                                )
                                  ? candidate.jsonSchema
                                  : undefined
                              }
                            : candidate
                        )
                      )
                    }
                  />
                </label>
                {outputTypeSupportsJsonSchema(output.valueType) ? (
                  <Tooltip title="编辑 JSON Schema">
                    <Button
                      aria-label="编辑 JSON Schema"
                      className={
                        output.jsonSchema
                          ? 'agent-flow-output-contract-editor__schema agent-flow-output-contract-editor__schema--active'
                          : 'agent-flow-output-contract-editor__schema'
                      }
                      icon={<FileTextOutlined />}
                      size="small"
                      type="text"
                      onClick={(event) =>
                        openSchemaEditor(index, event.currentTarget)
                      }
                    />
                  </Tooltip>
                ) : null}
                <Button
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.delete_output_variable',
                    { value1: output.key || index + 1 }
                  )}
                  className="agent-flow-output-contract-editor__delete"
                  danger
                  icon={<DeleteOutlined />}
                  size="small"
                  type="text"
                  onClick={() => {
                    removeItemKey(index);
                    emitChange(
                      value.filter((_, outputIndex) => outputIndex !== index)
                    );
                  }}
                />
              </div>
            );
          })}
        </div>
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'auto.output_variables_yet')}
        />
      )}
      <JsonSchemaSettingsPanel
        fallbackRootType={
          editingOutput ? fallbackSchemaRootType(editingOutput) : 'object'
        }
        open={editingOutput !== null}
        schema={
          editingOutput
            ? defaultSchemaForOutput(editingOutput)
            : createDefaultJsonSchema()
        }
        title="JSON Schema"
        triggerRef={schemaTriggerRef}
        onClose={closeSchemaEditor}
        onSave={saveSchema}
      />
    </div>
  );
}
