import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import FileTextOutlined from '@ant-design/icons/es/icons/FileTextOutlined';
import FormOutlined from '@ant-design/icons/es/icons/FormOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
import {
  Button,
  Input,
  InputNumber,
  Select,
  Segmented,
  Typography
} from 'antd';
import { useEffect, useRef, useState } from 'react';
import { i18nText } from '../../../../../shared/i18n/text';

const scalarObjectValueTypes = ['string', 'number', 'boolean'] as const;

type ScalarObjectValueType = (typeof scalarObjectValueTypes)[number];

interface ObjectValueRow {
  key: string;
  type: ScalarObjectValueType;
  value: string | number | boolean | null;
}

interface EnvironmentVariableValueEditorProps {
  value?: unknown;
  valueType: string;
  onChange?: (value: unknown) => void;
  onValueErrorChange?: (message: string | null) => void;
}

interface ObjectValueEditorProps extends EnvironmentVariableValueEditorProps {
  addButtonLabel?: string;
  ariaLabelPrefix?: string;
}

function inferScalarType(value: unknown): ScalarObjectValueType {
  if (typeof value === 'number') {
    return 'number';
  }

  if (typeof value === 'boolean') {
    return 'boolean';
  }

  return 'string';
}

function createEmptyObjectRow(): ObjectValueRow {
  return {
    key: '',
    type: 'string',
    value: ''
  };
}

function createObjectRows(value: unknown): ObjectValueRow[] {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    return [createEmptyObjectRow()];
  }

  const rows = Object.entries(value).map(([key, itemValue]) => ({
    key,
    type: inferScalarType(itemValue),
    value:
      typeof itemValue === 'string' ||
      typeof itemValue === 'number' ||
      typeof itemValue === 'boolean'
        ? itemValue
        : JSON.stringify(itemValue)
  }));

  return rows.length > 0 ? rows : [createEmptyObjectRow()];
}

function createObjectFromRows(rows: ObjectValueRow[]) {
  return rows.reduce<Record<string, string | number | boolean | null>>(
    (acc, row) => {
      if (row.key.trim().length === 0) {
        return acc;
      }

      acc[row.key.trim()] = row.value ?? null;
      return acc;
    },
    {}
  );
}

function formatJson(value: unknown) {
  return JSON.stringify(value, null, 2) ?? '';
}

function areJsonValuesEqual(left: unknown, right: unknown) {
  try {
    return JSON.stringify(left) === JSON.stringify(right);
  } catch {
    return Object.is(left, right);
  }
}

function createDefaultItem(valueType: string) {
  if (valueType === 'array[number]') {
    return 0;
  }

  if (valueType === 'array[boolean]') {
    return false;
  }

  if (valueType === 'array[object]') {
    return {};
  }

  return '';
}

function normalizeArrayValue(valueType: string, value: unknown) {
  const items = Array.isArray(value) ? value : [];

  if (items.length > 0) {
    return items;
  }

  return [createDefaultItem(valueType)];
}

function getJsonPlaceholder(valueType: string) {
  if (valueType === 'object') {
    return '{\n  "key": "value"\n}';
  }

  if (valueType === 'array[string]') {
    return '[\n  "item"\n]';
  }

  if (valueType === 'array[number]') {
    return '[\n  1\n]';
  }

  if (valueType === 'array[boolean]') {
    return '[\n  true\n]';
  }

  if (valueType === 'array[object]') {
    return '[\n  { "key": "value" }\n]';
  }

  return '';
}

function StructuredJsonEditor({
  value,
  valueType,
  onApply,
  onCancel,
  onValueErrorChange
}: {
  value: unknown;
  valueType: string;
  onApply: (value: unknown) => void;
  onCancel: () => void;
  onValueErrorChange?: (message: string | null) => void;
}) {
  const [content, setContent] = useState(() => formatJson(value));

  useEffect(() => {
    setContent(formatJson(value));
  }, [value]);

  function applyJson() {
    try {
      onApply(JSON.parse(content));
      onValueErrorChange?.(null);
    } catch {
      onValueErrorChange?.(i18nText('agentFlow', 'auto.json_format_invalid'));
    }
  }

  return (
    <div className="agent-flow-editor__env-value-json-editor">
      <Input.TextArea
        autoSize={{ minRows: 7, maxRows: 12 }}
        placeholder={getJsonPlaceholder(valueType)}
        value={content}
        onChange={(event) => {
          setContent(event.target.value);
          onValueErrorChange?.(null);
        }}
      />
      <div className="agent-flow-editor__env-value-json-actions">
        <Button size="small" onClick={onCancel}>
          {i18nText('agentFlow', 'auto.return_to_form')}
        </Button>
        <Button size="small" type="primary" onClick={applyJson}>
          {i18nText('agentFlow', 'auto.apply_json')}
        </Button>
      </div>
    </div>
  );
}

function ObjectValueEditor({
  value,
  onChange,
  onValueErrorChange,
  addButtonLabel = i18nText('agentFlow', 'auto.add_field'),
  ariaLabelPrefix = i18nText('agentFlow', 'auto.object')
}: ObjectValueEditorProps) {
  const [rows, setRows] = useState(() => createObjectRows(value));
  const lastEmittedValueRef = useRef<unknown>(value);

  useEffect(() => {
    if (areJsonValuesEqual(value, lastEmittedValueRef.current)) {
      return;
    }

    lastEmittedValueRef.current = value;
    setRows(createObjectRows(value));
  }, [value]);

  function updateRows(nextRows: ObjectValueRow[]) {
    const nextValue = createObjectFromRows(nextRows);

    setRows(nextRows);
    lastEmittedValueRef.current = nextValue;
    onValueErrorChange?.(null);
    onChange?.(nextValue);
  }

  return (
    <div className="agent-flow-editor__env-value-rows">
      {rows.map((row, index) => (
        <div className="agent-flow-editor__env-object-row" key={index}>
          <Input
            aria-label={i18nText('agentFlow', 'auto.key', {
              value1: ariaLabelPrefix,
              value2: index + 1
            })}
            placeholder="key"
            value={row.key}
            onChange={(event) =>
              updateRows(
                rows.map((candidate, candidateIndex) =>
                  candidateIndex === index
                    ? { ...candidate, key: event.target.value }
                    : candidate
                )
              )
            }
          />
          <Select
            aria-label={i18nText('agentFlow', 'auto.value_type', {
              value1: ariaLabelPrefix,
              value2: index + 1
            })}
            className="agent-flow-editor__env-object-type-select"
            options={scalarObjectValueTypes.map((type) => ({
              label: type,
              value: type
            }))}
            value={row.type}
            onChange={(nextType: ScalarObjectValueType) =>
              updateRows(
                rows.map((candidate, candidateIndex) =>
                  candidateIndex === index
                    ? {
                        ...candidate,
                        type: nextType,
                        value:
                          nextType === 'number'
                            ? 0
                            : nextType === 'boolean'
                              ? false
                              : ''
                      }
                    : candidate
                )
              )
            }
          />
          {row.type === 'number' ? (
            <InputNumber
              aria-label={i18nText('agentFlow', 'auto.value_alt', {
                value1: ariaLabelPrefix,
                value2: index + 1
              })}
              className="agent-flow-editor__env-object-value"
              value={typeof row.value === 'number' ? row.value : null}
              onChange={(nextValue) =>
                updateRows(
                  rows.map((candidate, candidateIndex) =>
                    candidateIndex === index
                      ? { ...candidate, value: nextValue }
                      : candidate
                  )
                )
              }
            />
          ) : row.type === 'boolean' ? (
            <Segmented
              block
              className="agent-flow-editor__env-object-value"
              options={[
                { label: 'true', value: true },
                { label: 'false', value: false }
              ]}
              value={row.value === true}
              onChange={(nextValue) =>
                updateRows(
                  rows.map((candidate, candidateIndex) =>
                    candidateIndex === index
                      ? { ...candidate, value: nextValue }
                      : candidate
                  )
                )
              }
            />
          ) : (
            <Input
              aria-label={i18nText('agentFlow', 'auto.value_alt', {
                value1: ariaLabelPrefix,
                value2: index + 1
              })}
              className="agent-flow-editor__env-object-value"
              placeholder="value"
              value={typeof row.value === 'string' ? row.value : ''}
              onChange={(event) =>
                updateRows(
                  rows.map((candidate, candidateIndex) =>
                    candidateIndex === index
                      ? { ...candidate, value: event.target.value }
                      : candidate
                  )
                )
              }
            />
          )}
          <Button
            aria-label={i18nText('agentFlow', 'auto.delete_field', {
              value1: ariaLabelPrefix,
              value2: index + 1
            })}
            disabled={rows.length === 1}
            icon={<DeleteOutlined />}
            type="text"
            onClick={() =>
              updateRows(
                rows.filter((_, candidateIndex) => candidateIndex !== index)
              )
            }
          />
        </div>
      ))}
      <Button
        aria-label={addButtonLabel}
        icon={<PlusOutlined />}
        size="small"
        onClick={() => updateRows([...rows, createEmptyObjectRow()])}
      >
        {addButtonLabel}
      </Button>
    </div>
  );
}

function ArrayObjectValueEditor({
  item,
  index,
  onChange,
  onValueErrorChange
}: {
  item: unknown;
  index: number;
  onChange: (value: unknown) => void;
  onValueErrorChange?: (message: string | null) => void;
}) {
  return (
    <div
      aria-label={i18nText('agentFlow', 'auto.array_object', {
        value1: index + 1
      })}
      className="agent-flow-editor__env-array-object-value"
    >
      <Typography.Text
        className="agent-flow-editor__env-array-object-title"
        type="secondary"
      >
        {i18nText('agentFlow', 'auto.object_field')}
      </Typography.Text>
      <ObjectValueEditor
        addButtonLabel={i18nText('agentFlow', 'auto.add_array_object_field', {
          value1: index + 1
        })}
        ariaLabelPrefix={i18nText('agentFlow', 'auto.array_object_field', {
          value1: index + 1
        })}
        value={item}
        valueType="object"
        onChange={onChange}
        onValueErrorChange={onValueErrorChange}
      />
    </div>
  );
}

function ArrayValueEditor({
  value,
  valueType,
  onChange,
  onValueErrorChange
}: EnvironmentVariableValueEditorProps) {
  const [items, setItems] = useState(() =>
    normalizeArrayValue(valueType, value)
  );
  const lastEmittedValueRef = useRef<unknown>(value);
  const previousValueTypeRef = useRef(valueType);

  useEffect(() => {
    const valueTypeChanged = previousValueTypeRef.current !== valueType;

    previousValueTypeRef.current = valueType;

    if (
      !valueTypeChanged &&
      areJsonValuesEqual(value, lastEmittedValueRef.current)
    ) {
      return;
    }

    lastEmittedValueRef.current = value;
    setItems(normalizeArrayValue(valueType, value));
  }, [value, valueType]);

  function updateItems(nextItems: unknown[]) {
    setItems(nextItems);
    lastEmittedValueRef.current = nextItems;
    onValueErrorChange?.(null);
    onChange?.(nextItems);
  }

  return (
    <div className="agent-flow-editor__env-value-rows">
      {items.map((item, index) => (
        <div className="agent-flow-editor__env-array-row" key={index}>
          <Typography.Text
            className="agent-flow-editor__env-array-index"
            type="secondary"
          >
            {index + 1}
          </Typography.Text>
          {valueType === 'array[number]' ? (
            <InputNumber
              aria-label={i18nText('agentFlow', 'auto.array_value', {
                value1: index + 1
              })}
              className="agent-flow-editor__env-array-value"
              value={typeof item === 'number' ? item : null}
              onChange={(nextValue) =>
                updateItems(
                  items.map((candidate, candidateIndex) =>
                    candidateIndex === index ? nextValue : candidate
                  )
                )
              }
            />
          ) : valueType === 'array[boolean]' ? (
            <Segmented
              block
              className="agent-flow-editor__env-array-value"
              options={[
                { label: 'true', value: true },
                { label: 'false', value: false }
              ]}
              value={item === true}
              onChange={(nextValue) =>
                updateItems(
                  items.map((candidate, candidateIndex) =>
                    candidateIndex === index ? nextValue : candidate
                  )
                )
              }
            />
          ) : valueType === 'array[object]' ? (
            <ArrayObjectValueEditor
              index={index}
              item={item}
              onChange={(nextValue) =>
                updateItems(
                  items.map((candidate, candidateIndex) =>
                    candidateIndex === index ? nextValue : candidate
                  )
                )
              }
              onValueErrorChange={onValueErrorChange}
            />
          ) : (
            <Input
              aria-label={i18nText('agentFlow', 'auto.array_value', {
                value1: index + 1
              })}
              className="agent-flow-editor__env-array-value"
              placeholder="value"
              value={typeof item === 'string' ? item : ''}
              onChange={(event) =>
                updateItems(
                  items.map((candidate, candidateIndex) =>
                    candidateIndex === index ? event.target.value : candidate
                  )
                )
              }
            />
          )}
          <Button
            aria-label={i18nText('agentFlow', 'auto.delete_array_item', {
              value1: index + 1
            })}
            disabled={items.length === 1}
            icon={<DeleteOutlined />}
            type="text"
            onClick={() =>
              updateItems(
                items.filter((_, candidateIndex) => candidateIndex !== index)
              )
            }
          />
        </div>
      ))}
      <Button
        aria-label={i18nText('agentFlow', 'auto.add_items')}
        icon={<PlusOutlined />}
        size="small"
        onClick={() => updateItems([...items, createDefaultItem(valueType)])}
      >
        {i18nText('agentFlow', 'auto.add_items')}
      </Button>
    </div>
  );
}

export function EnvironmentVariableValueEditor({
  value,
  valueType,
  onChange,
  onValueErrorChange
}: EnvironmentVariableValueEditorProps) {
  const [editInJson, setEditInJson] = useState(false);
  const structured = valueType === 'object' || valueType.startsWith('array[');

  useEffect(() => {
    setEditInJson(false);
  }, [valueType]);

  if (valueType === 'number') {
    return (
      <InputNumber
        className="agent-flow-editor__environment-variable-number-input"
        placeholder={i18nText('agentFlow', 'auto.enter_variable_value')}
        value={typeof value === 'number' ? value : null}
        onChange={(nextValue) => {
          onValueErrorChange?.(null);
          onChange?.(nextValue);
        }}
      />
    );
  }

  if (valueType === 'boolean') {
    return (
      <Segmented
        block
        options={[
          { label: 'true', value: true },
          { label: 'false', value: false }
        ]}
        value={value === true}
        onChange={(nextValue) => {
          onValueErrorChange?.(null);
          onChange?.(nextValue);
        }}
      />
    );
  }

  if (!structured) {
    return (
      <Input.TextArea
        autoSize={{ minRows: 3, maxRows: 10 }}
        placeholder={i18nText('agentFlow', 'auto.enter_variable_value')}
        value={typeof value === 'string' ? value : ''}
        onChange={(event) => {
          onValueErrorChange?.(null);
          onChange?.(event.target.value);
        }}
      />
    );
  }

  if (editInJson) {
    return (
      <StructuredJsonEditor
        value={value}
        valueType={valueType}
        onApply={(nextValue) => {
          onChange?.(nextValue);
          setEditInJson(false);
        }}
        onCancel={() => setEditInJson(false)}
        onValueErrorChange={onValueErrorChange}
      />
    );
  }

  return (
    <div className="agent-flow-editor__env-value-editor">
      <div className="agent-flow-editor__env-value-editor-toolbar">
        <Button
          icon={<FileTextOutlined />}
          size="small"
          type="text"
          onClick={() => setEditInJson(true)}
        >
          JSON
        </Button>
      </div>
      {valueType === 'object' ? (
        <ObjectValueEditor
          value={value}
          valueType={valueType}
          onChange={onChange}
          onValueErrorChange={onValueErrorChange}
        />
      ) : (
        <ArrayValueEditor
          value={value}
          valueType={valueType}
          onChange={onChange}
          onValueErrorChange={onValueErrorChange}
        />
      )}
      <Typography.Text type="secondary">
        <FormOutlined />{' '}
        {i18nText('agentFlow', 'auto.switch_json_mode_edit_complex_structures')}
      </Typography.Text>
    </div>
  );
}
