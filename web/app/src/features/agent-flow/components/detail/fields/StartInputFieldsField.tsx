import {
  DownOutlined,
  DeleteOutlined,
  HolderOutlined,
  PlusOutlined,
  RightOutlined
} from '@ant-design/icons';
import { Button, Empty, Typography } from 'antd';
import { useRef, useState } from 'react';

import type { ConsoleApplicationNodeContractField } from '@1flowbase/api-client';

import type {
  FlowStartInputField,
  FlowStartInputSource
} from '@1flowbase/flow-schema';

import { JsonPreviewBlock } from '../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { useStableListItemKeys } from '../../../hooks/interactions/use-stable-list-item-keys';
import {
  getStartInputValueType,
  normalizeStartInputField,
  startInputTypeOptions,
  startSystemVariables
} from '../../../lib/variables/start-node-variables';
import { CollectionFieldHeader } from '../collection/CollectionFieldHeader';
import { StartInputFieldSettingsPanel } from './StartInputFieldSettingsPanel';
import { i18nText } from '../../../../../shared/i18n/text';

function normalizeList(value: unknown): FlowStartInputField[] {
  return Array.isArray(value)
    ? value.map((field, index) => normalizeStartInputField(field, index))
    : [];
}

export type StartInputSourceOption = {
  value: FlowStartInputSource;
  label: string;
};

export type StartInputContractKind =
  | 'agent'
  | 'workflow_schedule'
  | 'workflow_http';

function createNextField(
  index: number,
  sourceOptions: StartInputSourceOption[],
  inputTypes: typeof startInputTypeOptions
): FlowStartInputField {
  const key = `input_${index + 1}`;
  const inputType = inputTypes[0]?.value ?? 'text';

  return {
    key,
    label: key,
    inputType,
    valueType: getStartInputValueType(inputType),
    required: false,
    source: sourceOptions[0]?.value
  };
}

function replaceAt(
  fields: FlowStartInputField[],
  index: number,
  patch: Partial<FlowStartInputField>
) {
  return fields.map((field, fieldIndex) =>
    fieldIndex === index ? { ...field, ...patch } : field
  );
}

function moveItem(fields: FlowStartInputField[], from: number, to: number) {
  if (to < 0 || to >= fields.length) {
    return fields;
  }

  const nextFields = [...fields];
  const [item] = nextFields.splice(from, 1);

  if (!item) {
    return fields;
  }

  nextFields.splice(to, 0, item);
  return nextFields;
}

function formatSystemVariableType(variable: {
  key: string;
  valueType: string;
}) {
  return formatValueType(variable.valueType);
}

function formatValueType(valueType: string) {
  if (valueType === 'string') {
    return 'String';
  }
  if (valueType === 'number') {
    return 'Number';
  }
  if (valueType === 'boolean') {
    return 'Boolean';
  }
  if (valueType === 'json') {
    return 'JSON';
  }
  return valueType;
}

function systemVariableExample(variableKey: string) {
  switch (variableKey) {
    case 'history':
      return [
        {
          role: 'user',
          content: i18nText('agentFlow', 'auto.last_round_user_messages')
        },
        {
          role: 'assistant',
          content: i18nText('agentFlow', 'auto.last_round_assistant_reply')
        }
      ];
    case 'files':
      return [
        {
          id: 'file-1',
          title: 'example.pdf',
          filename: 'example.pdf',
          extname: 'pdf',
          size: 245760,
          mimetype: 'application/pdf',
          path: 'attachments/2026/05/file-1.pdf',
          url: 'https://files.example.com/attachments/2026/05/file-1.pdf',
          storage_id: 'storage-1',
          meta: {}
        }
      ];
    default:
      return null;
  }
}

type EditingInputField = {
  index: number | null;
  field: FlowStartInputField;
};

export function StartInputFieldsField({
  contractKind,
  title,
  value,
  sourceOptions = [],
  inputTypeContract,
  sourceContract,
  onChange
}: {
  contractKind: StartInputContractKind;
  title: string;
  value: unknown;
  sourceOptions?: StartInputSourceOption[];
  inputTypeContract?: ConsoleApplicationNodeContractField;
  sourceContract?: ConsoleApplicationNodeContractField;
  onChange: (value: FlowStartInputField[]) => void;
}) {
  const fields = normalizeList(value);
  const variableNamespace = contractKind === 'agent' ? 'userinput' : 'input';
  const [editing, setEditing] = useState<EditingInputField | null>(null);
  const [expandedSystemKeys, setExpandedSystemKeys] = useState<Set<string>>(
    () => new Set()
  );
  const triggerRef = useRef<HTMLButtonElement | null>(null);
  const draggingIndexRef = useRef<number | null>(null);
  const { itemKeys, insertItemKey, moveItemKey, removeItemKey } =
    useStableListItemKeys('start-input-field', fields.length);
  const inputTypes =
    contractKind === 'agent'
      ? startInputTypeOptions
      : startInputTypeOptions.filter((option) =>
          inputTypeContract?.allowed_values.includes(option.value)
        );

  function openAddPanel() {
    if (inputTypes.length === 0) {
      return;
    }

    setEditing({
      index: null,
      field: createNextField(fields.length, sourceOptions, inputTypes)
    });
  }

  function openEditPanel(field: FlowStartInputField, index: number) {
    setEditing({
      index,
      field: normalizeStartInputField(field, index)
    });
  }

  function closePanel() {
    setEditing(null);
  }

  function updateDraft(patch: Partial<FlowStartInputField>) {
    setEditing((current) =>
      current
        ? {
            ...current,
            field: {
              ...current.field,
              ...patch
            }
          }
        : current
    );
  }

  function saveDraft() {
    if (!editing) {
      return;
    }

    const nextField = normalizeStartInputField(
      editing.field,
      editing.index ?? fields.length
    );

    if (editing.index === null) {
      insertItemKey(fields.length);
      onChange([...fields, nextField]);
    } else {
      onChange(replaceAt(fields, editing.index, nextField));
    }

    closePanel();
  }

  function handleDragStart(index: number) {
    draggingIndexRef.current = index;
  }

  function handleDrop(targetIndex: number) {
    const draggingIndex = draggingIndexRef.current;

    if (draggingIndex === null || draggingIndex === targetIndex) {
      draggingIndexRef.current = null;
      return;
    }

    moveItemKey(draggingIndex, targetIndex);
    onChange(moveItem(fields, draggingIndex, targetIndex));
    draggingIndexRef.current = null;
  }

  function toggleSystemVariable(key: string) {
    setExpandedSystemKeys((current) => {
      const next = new Set(current);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }

  const floatingPanel = editing ? (
    <StartInputFieldSettingsPanel
      mode={editing.index === null ? 'create' : 'edit'}
      field={editing.field}
      inputTypeContract={inputTypeContract}
      inputTypeOptions={inputTypes}
      sourceContract={sourceContract}
      sourceOptions={sourceOptions}
      triggerRef={triggerRef}
      onChange={updateDraft}
      onClose={closePanel}
      onSave={saveDraft}
    />
  ) : null;

  return (
    <div className="agent-flow-start-input-fields">
      <CollectionFieldHeader
        title={title}
        actions={
          <Button
            aria-label={i18nText('agentFlow', 'auto.add_new_input_field')}
            icon={<PlusOutlined />}
            size="small"
            type="text"
            disabled={inputTypes.length === 0}
            onClick={openAddPanel}
            ref={triggerRef}
          />
        }
      />

      {fields.length > 0 ? (
        <div className="agent-flow-start-input-fields__list">
          {fields.map((field, index) => (
            <div
              key={itemKeys[index]}
              className="agent-flow-start-input-fields__item agent-flow-node-detail__list-item"
              data-testid={`start-input-field-row-${field.key}`}
              onDragOver={(event) => event.preventDefault()}
              onDrop={() => handleDrop(index)}
            >
              <button
                aria-label={i18nText(
                  'agentFlow',
                  'auto.drag_drop_sort_input_field',
                  { value1: field.key }
                )}
                className="agent-flow-start-input-fields__drag-handle"
                draggable
                onDragStart={() => handleDragStart(index)}
                onDragEnd={() => {
                  draggingIndexRef.current = null;
                }}
                type="button"
              >
                <HolderOutlined />
              </button>
              <button
                aria-label={i18nText('agentFlow', 'auto.edit_input_field', {
                  value1: field.key
                })}
                className="agent-flow-start-input-fields__variable-main"
                type="button"
                onClick={() => openEditPanel(field, index)}
              >
                <span className="agent-flow-node-detail__list-item-left">
                  <span className="agent-flow-node-detail__list-item-icon">
                    {'{x}'}
                  </span>
                  <span className="agent-flow-start-input-fields__name-stack">
                    <span className="agent-flow-node-detail__list-item-name">
                      {variableNamespace}.{field.key}
                    </span>
                    {field.label && field.label !== field.key ? (
                      <span className="agent-flow-start-input-fields__label">
                        {field.label}
                      </span>
                    ) : null}
                  </span>
                </span>
                <span className="agent-flow-start-input-fields__item-meta">
                  {field.required ? (
                    <span className="agent-flow-start-input-fields__badge">
                      {i18nText('agentFlow', 'auto.required')}
                    </span>
                  ) : null}
                  {field.hidden ? (
                    <span className="agent-flow-start-input-fields__badge">
                      {i18nText('agentFlow', 'auto.hide')}
                    </span>
                  ) : null}
                  <span className="agent-flow-node-detail__list-item-type">
                    {formatValueType(field.valueType)}
                  </span>
                </span>
              </button>
              <Button
                aria-label={i18nText('agentFlow', 'auto.delete_input_field', {
                  value1: field.key
                })}
                className="agent-flow-start-input-fields__delete"
                danger
                icon={<DeleteOutlined />}
                size="small"
                type="text"
                onClick={() => {
                  removeItemKey(index);
                  onChange(
                    fields.filter((_, fieldIndex) => fieldIndex !== index)
                  );
                }}
              />
            </div>
          ))}
        </div>
      ) : (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={i18nText('agentFlow', 'auto.custom_input_field_yet')}
        />
      )}

      {contractKind === 'agent' ? (
        <div className="agent-flow-start-input-fields__system">
          <Typography.Text
            strong
            className="agent-flow-start-input-fields__system-title"
          >
            {i18nText('agentFlow', 'auto.system_variables')}
          </Typography.Text>
          <div className="agent-flow-node-detail__list">
            {startSystemVariables.map((variable) => {
              const example = systemVariableExample(variable.key);
              const expanded = expandedSystemKeys.has(variable.key);
              const systemRowContent = (
                <>
                  <span className="agent-flow-node-detail__list-item-left">
                    {example ? (
                      <span className="agent-flow-start-input-fields__system-chevron">
                        {expanded ? <DownOutlined /> : <RightOutlined />}
                      </span>
                    ) : null}
                    <span className="agent-flow-node-detail__list-item-icon">
                      {'{x}'}
                    </span>
                    <span className="agent-flow-node-detail__list-item-name">
                      {variable.title}
                    </span>
                  </span>
                  <span className="agent-flow-node-detail__list-item-type">
                    {formatSystemVariableType(variable)}
                  </span>
                </>
              );

              return (
                <div
                  key={variable.key}
                  data-testid={`start-system-variable-${variable.key}`}
                  className={[
                    'agent-flow-start-input-fields__system-variable',
                    expanded
                      ? 'agent-flow-start-input-fields__system-variable--expanded'
                      : null
                  ]
                    .filter(Boolean)
                    .join(' ')}
                >
                  {example ? (
                    <button
                      type="button"
                      className="agent-flow-node-detail__list-item agent-flow-start-input-fields__system-trigger"
                      aria-expanded={expanded}
                      onClick={() => {
                        toggleSystemVariable(variable.key);
                      }}
                    >
                      {systemRowContent}
                    </button>
                  ) : (
                    <div className="agent-flow-node-detail__list-item">
                      {systemRowContent}
                    </div>
                  )}
                  {example && expanded ? (
                    <JsonPreviewBlock
                      className="agent-flow-start-input-fields__system-json"
                      collapsible={false}
                      copyAriaLabel={i18nText('agentFlow', 'auto.copy_json', {
                        value1: variable.title
                      })}
                      displayTitle=""
                      fullscreenAriaLabel={i18nText(
                        'agentFlow',
                        'auto.zoom_view_json',
                        { value1: variable.title }
                      )}
                      height="180px"
                      title={variable.title}
                      value={example}
                    />
                  ) : null}
                </div>
              );
            })}
          </div>
        </div>
      ) : null}
      {floatingPanel}
    </div>
  );
}
