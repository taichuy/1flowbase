import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Checkbox, Input, Select, Tabs, Typography } from 'antd';
import {
  Suspense,
  forwardRef,
  lazy,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
  type ReactNode,
  type RefObject
} from 'react';

import { parseJsonSchemaInput } from '../../../../lib/output-contract/schema';
import { FloatingSettingsPanel } from '../../FloatingSettingsPanel';
import { i18nText } from '../../../../../../shared/i18n/text';
import {
  jsonSchemaRootType,
  type JsonSchemaRootType
} from './json-schema-utils';
import {
  JSON_SCHEMA_EDITOR_OPTIONS,
  createSchemaFieldRowId,
  isRecord,
  objectSchemaFromRows,
  schemaEnumArrayTypeOptionValue,
  schemaEnumArrayTypeOptions,
  schemaEnumValueTypeForRow,
  schemaEnumValueTypeFromArrayOption,
  schemaEnumValueTypeOptions,
  schemaFieldTypeOptionValue,
  schemaFieldTypeOptions,
  schemaFieldTypePatch,
  schemaFromRows,
  schemaRowsFromSchema,
  stringifySchema,
  typedEnumValues,
  type SchemaEditorTab,
  type SchemaEnumValueType,
  type SchemaFieldRow,
  type SchemaFieldTypeOptionValue
} from './schema-row-model';

const MonacoEditor = lazy(() => import('@monaco-editor/react'));

function JsonSchemaEditorFallback() {
  return (
    <div className="agent-flow-json-schema-settings__editor-loading">
      {i18nText('agentFlow', 'auto.loading_json_schema_editor')}
    </div>
  );
}

function JsonSchemaCodeEditor({
  value,
  onChange,
  ariaLabel = i18nText('agentFlow', 'auto.json_schema_content')
}: {
  value: string;
  onChange: (value: string) => void;
  ariaLabel?: string;
}) {
  const options = useMemo(
    () => ({
      ...JSON_SCHEMA_EDITOR_OPTIONS,
      ariaLabel
    }),
    [ariaLabel]
  );

  return (
    <div className="agent-flow-json-schema-settings__editor">
      <Suspense fallback={<JsonSchemaEditorFallback />}>
        <MonacoEditor
          aria-label={ariaLabel}
          defaultLanguage="json"
          height="100%"
          language="json"
          options={options}
          theme="vs"
          value={value}
          onChange={(nextValue) => onChange(nextValue ?? '')}
        />
      </Suspense>
    </div>
  );
}

export function InlineJsonCodeEditor({
  ariaLabel,
  className,
  testId,
  value,
  onChange
}: {
  ariaLabel: string;
  className?: string;
  testId?: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className={className} data-testid={testId}>
      <JsonSchemaCodeEditor
        ariaLabel={ariaLabel}
        value={value}
        onChange={onChange}
      />
    </div>
  );
}

function readonlySchemaValue(value: unknown) {
  if (value === undefined) {
    return '';
  }

  if (typeof value === 'string') {
    return JSON.stringify(value);
  }

  if (
    typeof value === 'number' ||
    typeof value === 'boolean' ||
    value === null
  ) {
    return String(value);
  }

  return JSON.stringify(value);
}

function readonlySchemaValueType(value: unknown) {
  if (Array.isArray(value)) {
    return 'Array';
  }

  if (value === null) {
    return 'Null';
  }

  if (typeof value === 'object') {
    return 'Object';
  }

  if (typeof value === 'string') {
    return 'String';
  }

  if (typeof value === 'number') {
    return 'Number';
  }

  if (typeof value === 'boolean') {
    return 'Boolean';
  }

  return 'Unknown';
}

interface JsonSchemaSettingsPanelProps {
  open: boolean;
  schema: Record<string, unknown>;
  fallbackRootType?: JsonSchemaRootType;
  className?: string;
  title?: string;
  triggerRef: RefObject<HTMLElement | null>;
  onClose: () => void;
  onSave: (schema: Record<string, unknown>) => void;
}

export type JsonSchemaEditorResult =
  | { ok: true; schema: Record<string, unknown> }
  | { ok: false; message: string };

export interface JsonSchemaEditorContentHandle {
  getSchema: () => JsonSchemaEditorResult;
}

interface JsonSchemaEditorContentProps {
  schema: Record<string, unknown>;
  fallbackRootType?: JsonSchemaRootType;
  active?: boolean;
  className?: string;
  live?: boolean;
  resetKey?: string | number | null;
  structureMode?: 'schema' | 'fields';
  parseSchemaInput?: (input: string) => JsonSchemaEditorResult;
  onChange?: (schema: Record<string, unknown>) => void;
  onValidityChange?: (valid: boolean) => void;
}

export const JsonSchemaEditorContent = forwardRef<
  JsonSchemaEditorContentHandle,
  JsonSchemaEditorContentProps
>(function JsonSchemaEditorContent(
  {
    active = true,
    schema,
    fallbackRootType = 'object',
    className,
    live = false,
    resetKey,
    structureMode = 'schema',
    parseSchemaInput = parseJsonSchemaInput,
    onChange,
    onValidityChange
  }: JsonSchemaEditorContentProps,
  ref
) {
  const [schemaText, setSchemaText] = useState('');
  const [schemaRows, setSchemaRows] = useState<SchemaFieldRow[]>([]);
  const [schemaBase, setSchemaBase] = useState<Record<string, unknown>>(schema);
  const [schemaRoot, setSchemaRoot] =
    useState<JsonSchemaRootType>(fallbackRootType);
  const [schemaTab, setSchemaTab] = useState<SchemaEditorTab>('fields');
  const [schemaError, setSchemaError] = useState<string | null>(null);
  const lastResetKeyRef = useRef<string | number | null | undefined>(undefined);

  useEffect(() => {
    if (!active) {
      return;
    }

    if (resetKey !== undefined && lastResetKeyRef.current === resetKey) {
      return;
    }

    lastResetKeyRef.current = resetKey;
    setSchemaBase(schema);
    setSchemaText(stringifySchema(schema));
    setSchemaRows(schemaRowsFromSchema(schema));
    setSchemaRoot(jsonSchemaRootType(schema, fallbackRootType));
    setSchemaTab('fields');
    setSchemaError(null);
    onValidityChange?.(true);
  }, [active, fallbackRootType, onValidityChange, resetKey, schema]);

  function notifyValidSchema(nextSchema: Record<string, unknown>) {
    if (!live) {
      return;
    }

    setSchemaError(null);
    onValidityChange?.(true);
    onChange?.(nextSchema);
  }

  function notifyInvalidSchema(message: string) {
    if (!live) {
      return;
    }

    setSchemaError(message);
    onValidityChange?.(false);
  }

  function readCurrentSchema(): JsonSchemaEditorResult {
    if (schemaTab === 'fields') {
      return {
        ok: true,
        schema: schemaFromRows(schemaRoot, schemaRows, schemaBase)
      };
    }

    return parseSchemaInput(schemaText);
  }

  function getSchema() {
    const parsed = readCurrentSchema();

    if (!parsed.ok) {
      setSchemaError(parsed.message);
      onValidityChange?.(false);
      return parsed;
    }

    setSchemaError(null);
    onValidityChange?.(true);
    return parsed;
  }

  useImperativeHandle(ref, () => ({ getSchema }));

  function updateSchemaRowsAtPath(
    rows: SchemaFieldRow[],
    path: number[],
    updater: (row: SchemaFieldRow) => SchemaFieldRow
  ): SchemaFieldRow[] {
    const [head, ...tail] = path;

    if (head === undefined) {
      return rows;
    }

    return rows.map((row, rowIndex) => {
      if (rowIndex !== head) {
        return row;
      }

      if (tail.length === 0) {
        return updater(row);
      }

      return {
        ...row,
        children: updateSchemaRowsAtPath(row.children ?? [], tail, updater)
      };
    });
  }

  function applySchemaRowPatch(
    row: SchemaFieldRow,
    patch: Partial<SchemaFieldRow>
  ): SchemaFieldRow {
    const nextType = patch.type ?? row.type;
    const nextArrayItemType =
      nextType === 'array'
        ? (patch.arrayItemType ?? row.arrayItemType ?? 'object')
        : undefined;
    const typeChanged = patch.type !== undefined && patch.type !== row.type;
    const arrayItemTypeChanged =
      patch.arrayItemType !== undefined &&
      patch.arrayItemType !== row.arrayItemType;
    const nextSupportsChildren =
      nextType === 'object' ||
      (nextType === 'array' && nextArrayItemType === 'object');
    const nextSupportsEnum =
      nextType === 'string' ||
      nextType === 'number' ||
      (nextType === 'array' &&
        (nextArrayItemType === 'string' || nextArrayItemType === 'number'));

    return {
      ...row,
      ...patch,
      arrayItemType: nextArrayItemType,
      baseSchema:
        typeChanged || arrayItemTypeChanged
          ? undefined
          : (patch.baseSchema ?? row.baseSchema),
      enumValues: nextSupportsEnum
        ? (patch.enumValues ?? row.enumValues)
        : undefined,
      children: nextSupportsChildren
        ? (patch.children ?? row.children ?? [])
        : undefined
    };
  }

  function updateSchemaRow(path: number[], patch: Partial<SchemaFieldRow>) {
    const nextRows = updateSchemaRowsAtPath(schemaRows, path, (row) =>
      applySchemaRowPatch(row, patch)
    );

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function updateSchemaRowType(
    path: number[],
    typeValue: SchemaFieldTypeOptionValue
  ) {
    updateSchemaRow(path, schemaFieldTypePatch(typeValue));
  }

  function updateSchemaRowEnumValueType(
    path: number[],
    valueType: SchemaEnumValueType
  ) {
    const nextRows = updateSchemaRowsAtPath(schemaRows, path, (row) =>
      applySchemaRowPatch(
        row,
        row.type === 'array'
          ? { type: 'array', arrayItemType: valueType }
          : { type: valueType }
      )
    );

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function addSchemaRowEnumValue(path: number[]) {
    const nextRows = updateSchemaRowsAtPath(schemaRows, path, (row) => {
      const enumValues = row.enumValues ?? [];
      const enumValueType = schemaEnumValueTypeForRow(row);
      const nextValue =
        enumValueType === 'number'
          ? String(enumValues.length + 1)
          : `value_${enumValues.length + 1}`;

      return {
        ...row,
        enumValues: [...enumValues, nextValue]
      };
    });

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function updateSchemaRowEnumValue(
    path: number[],
    enumIndex: number,
    value: string
  ) {
    const nextRows = updateSchemaRowsAtPath(schemaRows, path, (row) => {
      const enumValues = [...(row.enumValues ?? [])];
      enumValues[enumIndex] = value;

      return {
        ...row,
        enumValues
      };
    });

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function removeSchemaRowEnumValue(path: number[], enumIndex: number) {
    const nextRows = updateSchemaRowsAtPath(schemaRows, path, (row) => ({
      ...row,
      enumValues: (row.enumValues ?? []).filter(
        (_, index) => index !== enumIndex
      )
    }));

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function createSchemaRow(index: number): SchemaFieldRow {
    return {
      id: createSchemaFieldRowId(),
      key: `field_${index + 1}`,
      type: 'string',
      description: '',
      required: true
    };
  }

  function addSchemaRow(parentPath?: number[]) {
    if (!parentPath) {
      const nextRows = [...schemaRows, createSchemaRow(schemaRows.length)];

      setSchemaRows(nextRows);
      notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
      return;
    }

    const nextRows = updateSchemaRowsAtPath(schemaRows, parentPath, (row) => {
      const children = row.children ?? [];

      return {
        ...row,
        children: [...children, createSchemaRow(children.length)]
      };
    });

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function removeSchemaRow(path: number[]) {
    const removeAtPath = (
      rows: SchemaFieldRow[],
      targetPath: number[]
    ): SchemaFieldRow[] => {
      const [head, ...tail] = targetPath;

      if (head === undefined) {
        return rows;
      }

      if (tail.length === 0) {
        return rows.filter((_, rowIndex) => rowIndex !== head);
      }

      return rows.map((row, rowIndex) =>
        rowIndex === head
          ? { ...row, children: removeAtPath(row.children ?? [], tail) }
          : row
      );
    };

    const nextRows = removeAtPath(schemaRows, path);

    setSchemaRows(nextRows);
    notifyValidSchema(schemaFromRows(schemaRoot, nextRows, schemaBase));
  }

  function switchSchemaTab(nextTab: string) {
    if (nextTab === 'json') {
      const nextSchema = schemaFromRows(schemaRoot, schemaRows, schemaBase);

      setSchemaBase(nextSchema);
      setSchemaText(stringifySchema(nextSchema));
      setSchemaTab('json');
      setSchemaError(null);
      notifyValidSchema(nextSchema);
      return;
    }

    const parsed = parseSchemaInput(schemaText);
    if (!parsed.ok) {
      setSchemaError(parsed.message);
      onValidityChange?.(false);
      return;
    }

    setSchemaRows(schemaRowsFromSchema(parsed.schema));
    setSchemaBase(parsed.schema);
    setSchemaRoot(jsonSchemaRootType(parsed.schema, schemaRoot));
    setSchemaTab('fields');
    setSchemaError(null);
    notifyValidSchema(parsed.schema);
  }

  function updateSchemaText(nextValue: string) {
    setSchemaText(nextValue);

    if (!live) {
      setSchemaError(null);
      return;
    }

    const parsed = parseSchemaInput(nextValue);

    if (!parsed.ok) {
      notifyInvalidSchema(parsed.message);
      return;
    }

    setSchemaBase(parsed.schema);
    notifyValidSchema(parsed.schema);
  }

  function renderReadonlySchemaRow(
    key: string,
    value: unknown,
    indent = 0,
    operation?: ReactNode
  ) {
    return (
      <div
        className="agent-flow-json-schema-settings__field-row agent-flow-json-schema-settings__field-row--readonly"
        style={{ paddingLeft: indent }}
      >
        <Input
          aria-label={i18nText('agentFlow', 'auto.schema_field_name', {
            value1: key
          })}
          disabled
          value={key}
        />
        <Input
          aria-label={i18nText('agentFlow', 'auto.schema_field_value', {
            value1: key
          })}
          disabled
          value={readonlySchemaValue(value)}
        />
        <Input disabled value={readonlySchemaValueType(value)} />
        <Checkbox disabled checked={false} />
        {operation ?? (
          <Typography.Text type="secondary">
            {i18nText('agentFlow', 'auto.read_only')}
          </Typography.Text>
        )}
      </div>
    );
  }

  function renderReadonlySchemaNode(key: string, value: unknown, indent = 0) {
    return (
      <div
        className="agent-flow-json-schema-settings__field-node"
        key={`readonly-${indent}-${key}`}
      >
        {renderReadonlySchemaRow(key, value, indent)}
      </div>
    );
  }

  function renderReadonlySchemaNodes(
    schema: Record<string, unknown>,
    excludedKeys: string[],
    indent = 0
  ) {
    const excluded = new Set(excludedKeys);

    return Object.entries(schema)
      .filter(([key]) => !excluded.has(key))
      .map(([key, value]) => renderReadonlySchemaNode(key, value, indent));
  }

  function renderPropertiesSchemaNode(
    objectSchema: Record<string, unknown>,
    indent = 0
  ) {
    const properties = isRecord(objectSchema.properties)
      ? objectSchema.properties
      : {};

    return (
      <div
        className="agent-flow-json-schema-settings__field-node"
        key={`properties-${indent}`}
      >
        {renderReadonlySchemaRow(
          'properties',
          properties,
          indent,
          <Button
            aria-label={i18nText('agentFlow', 'auto.add_schema_field')}
            icon={<PlusOutlined />}
            size="small"
            type="text"
            onClick={() => addSchemaRow()}
          />
        )}
        <div
          className="agent-flow-json-schema-settings__field-children"
          style={{ paddingLeft: indent + 18 }}
        >
          {renderSchemaRows(schemaRows)}
        </div>
      </div>
    );
  }

  function renderSchemaStructureRows() {
    const currentSchema = schemaFromRows(schemaRoot, schemaRows, schemaBase);

    if (structureMode === 'fields') {
      return (
        <>
          {renderSchemaRows(schemaRows)}
          <Button
            aria-label={i18nText('agentFlow', 'auto.add_schema_field')}
            icon={<PlusOutlined />}
            type="dashed"
            onClick={() => addSchemaRow()}
          >
            {i18nText('agentFlow', 'auto.add_schema_field')}
          </Button>
        </>
      );
    }

    if (schemaRoot === 'array') {
      const itemSchema = isRecord(currentSchema.items)
        ? currentSchema.items
        : objectSchemaFromRows(schemaRows);

      return (
        <>
          {renderReadonlySchemaNode('type', currentSchema.type ?? 'array')}
          <div
            className="agent-flow-json-schema-settings__field-node"
            key="items"
          >
            {renderReadonlySchemaRow('items', itemSchema)}
            <div
              className="agent-flow-json-schema-settings__field-children"
              style={{ paddingLeft: 18 }}
            >
              {renderReadonlySchemaNode('type', itemSchema.type ?? 'object')}
              {renderReadonlySchemaNode(
                'required',
                Array.isArray(itemSchema.required) ? itemSchema.required : []
              )}
              {renderPropertiesSchemaNode(itemSchema)}
              {renderReadonlySchemaNodes(itemSchema, [
                'type',
                'required',
                'properties'
              ])}
            </div>
          </div>
          {renderReadonlySchemaNodes(currentSchema, ['type', 'items'])}
        </>
      );
    }

    return (
      <>
        {renderReadonlySchemaNode('type', currentSchema.type ?? 'object')}
        {renderReadonlySchemaNode(
          'required',
          Array.isArray(currentSchema.required) ? currentSchema.required : []
        )}
        {renderPropertiesSchemaNode(currentSchema)}
        {renderReadonlySchemaNodes(currentSchema, [
          'type',
          'required',
          'properties'
        ])}
      </>
    );
  }

  function renderSchemaEnumRows(
    row: SchemaFieldRow,
    path: number[],
    pathLabel: string,
    parentPath: number[]
  ) {
    const enumValueType = schemaEnumValueTypeForRow(row);

    if (!enumValueType) {
      return null;
    }
    const enumValues = row.enumValues ?? [];

    if (enumValues.length === 0) {
      return null;
    }
    const enumArrayValue = readonlySchemaValue(
      typedEnumValues(enumValues, enumValueType)
    );

    return (
      <div
        className="agent-flow-json-schema-settings__field-children"
        style={{ paddingLeft: (parentPath.length + 1) * 18 }}
      >
        <div className="agent-flow-json-schema-settings__field-node">
          <div className="agent-flow-json-schema-settings__field-row">
            <Input
              aria-label={i18nText('agentFlow', 'auto.schema_enum_field_name', {
                value1: pathLabel
              })}
              disabled
              value="enum"
            />
            <Input
              aria-label={i18nText(
                'agentFlow',
                'auto.schema_enum_field_value',
                {
                  value1: pathLabel
                }
              )}
              disabled
              value={enumArrayValue}
            />
            <Select
              aria-label={i18nText('agentFlow', 'auto.schema_enum_field_type', {
                value1: pathLabel
              })}
              options={schemaEnumArrayTypeOptions}
              value={schemaEnumArrayTypeOptionValue(enumValueType)}
              onChange={(valueType) =>
                updateSchemaRowEnumValueType(
                  path,
                  schemaEnumValueTypeFromArrayOption(valueType)
                )
              }
            />
            <Checkbox disabled checked={false} />
            <div className="agent-flow-json-schema-settings__field-actions" />
          </div>
          <div
            className="agent-flow-json-schema-settings__field-children"
            style={{ paddingLeft: 18 }}
          >
            {enumValues.map((enumValue, enumIndex) => {
              const enumLabel = `${pathLabel}.${enumIndex + 1}`;

              return (
                <div
                  className="agent-flow-json-schema-settings__field-row"
                  key={`${row.id}-enum-${enumIndex}`}
                >
                  <Input
                    aria-label={i18nText('agentFlow', 'auto.schema_enum_name', {
                      value1: enumLabel
                    })}
                    disabled
                    value={`enum[${enumIndex + 1}]`}
                  />
                  <Input
                    aria-label={i18nText(
                      'agentFlow',
                      'auto.schema_enum_value',
                      {
                        value1: enumLabel
                      }
                    )}
                    value={enumValue}
                    onChange={(event) =>
                      updateSchemaRowEnumValue(
                        path,
                        enumIndex,
                        event.target.value
                      )
                    }
                  />
                  <Select
                    aria-label={i18nText('agentFlow', 'auto.schema_enum_type', {
                      value1: enumLabel
                    })}
                    options={schemaEnumValueTypeOptions}
                    value={enumValueType}
                    onChange={(valueType) =>
                      updateSchemaRowEnumValueType(path, valueType)
                    }
                  />
                  <Checkbox disabled checked={false} />
                  <Button
                    aria-label={i18nText(
                      'agentFlow',
                      'auto.delete_schema_enum_value',
                      { value1: enumLabel }
                    )}
                    danger
                    icon={<DeleteOutlined />}
                    size="small"
                    type="text"
                    onClick={() => removeSchemaRowEnumValue(path, enumIndex)}
                  />
                </div>
              );
            })}
          </div>
        </div>
      </div>
    );
  }

  function renderSchemaRows(rows: SchemaFieldRow[], parentPath: number[] = []) {
    return rows.map((row, index) => {
      const path = [...parentPath, index];
      const pathLabel = path.map((item) => item + 1).join('.');
      const hasChildren =
        row.type === 'object' ||
        (row.type === 'array' && (row.arrayItemType ?? 'object') === 'object');
      const canAddEnum = schemaEnumValueTypeForRow(row) !== undefined;
      const childRows = row.children ?? [];

      return (
        <div
          className="agent-flow-json-schema-settings__field-node"
          key={row.id}
        >
          <div
            className="agent-flow-json-schema-settings__field-row"
            style={{ paddingLeft: parentPath.length * 18 }}
          >
            <Input
              aria-label={i18nText('agentFlow', 'auto.schema_field_name', {
                value1: pathLabel
              })}
              value={row.key}
              onChange={(event) =>
                updateSchemaRow(path, {
                  key: event.target.value
                })
              }
            />
            <Input
              aria-label={i18nText(
                'agentFlow',
                'auto.schema_field_description',
                {
                  value1: pathLabel
                }
              )}
              value={row.description}
              onChange={(event) =>
                updateSchemaRow(path, {
                  description: event.target.value
                })
              }
            />
            <Select
              aria-label={i18nText('agentFlow', 'auto.schema_field_type', {
                value1: pathLabel
              })}
              options={schemaFieldTypeOptions}
              value={schemaFieldTypeOptionValue(row)}
              onChange={(type) => updateSchemaRowType(path, type)}
            />
            <Checkbox
              aria-label={i18nText('agentFlow', 'auto.schema_field_required', {
                value1: pathLabel
              })}
              checked={row.required}
              onChange={(event) =>
                updateSchemaRow(path, {
                  required: event.target.checked
                })
              }
            />
            <div
              aria-label={i18nText('agentFlow', 'auto.schema_field_actions', {
                value1: row.key || pathLabel
              })}
              className="agent-flow-json-schema-settings__field-actions"
              role="group"
            >
              {canAddEnum ? (
                <Button
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.add_schema_enum_value',
                    { value1: row.key || pathLabel }
                  )}
                  icon={<PlusOutlined />}
                  size="small"
                  type="text"
                  onClick={() => addSchemaRowEnumValue(path)}
                />
              ) : null}
              {hasChildren ? (
                <Button
                  aria-label={i18nText(
                    'agentFlow',
                    'auto.add_child_schema_field',
                    { value1: row.key || pathLabel }
                  )}
                  icon={<PlusOutlined />}
                  size="small"
                  type="text"
                  onClick={() => addSchemaRow(path)}
                />
              ) : null}
              <Button
                aria-label={i18nText('agentFlow', 'auto.delete_schema_field', {
                  value1: row.key || pathLabel
                })}
                danger
                icon={<DeleteOutlined />}
                size="small"
                type="text"
                onClick={() => removeSchemaRow(path)}
              />
            </div>
          </div>
          {renderSchemaEnumRows(row, path, pathLabel, parentPath)}
          {hasChildren && childRows.length > 0 ? (
            <div
              className="agent-flow-json-schema-settings__field-children"
              style={{ paddingLeft: (parentPath.length + 1) * 18 }}
            >
              {renderSchemaRows(childRows, path)}
            </div>
          ) : null}
        </div>
      );
    });
  }

  return (
    <div
      className={['agent-flow-json-schema-settings', className]
        .filter(Boolean)
        .join(' ')}
    >
      <Tabs
        activeKey={schemaTab}
        onChange={switchSchemaTab}
        items={[
          {
            key: 'fields',
            label: i18nText('agentFlow', 'auto.schema_fields'),
            children: (
              <div className="agent-flow-json-schema-settings__fields">
                <div className="agent-flow-json-schema-settings__field-head">
                  <span>{i18nText('agentFlow', 'auto.field_name')}</span>
                  <span>
                    {i18nText('agentFlow', 'auto.schema_value_or_description')}
                  </span>
                  <span>{i18nText('agentFlow', 'auto.type')}</span>
                  <span>{i18nText('agentFlow', 'auto.required')}</span>
                  <span>{i18nText('agentFlow', 'auto.operation')}</span>
                </div>
                <div className="agent-flow-json-schema-settings__field-rows">
                  {renderSchemaStructureRows()}
                </div>
              </div>
            )
          },
          {
            key: 'json',
            label: i18nText('agentFlow', 'auto.json_parse'),
            children: (
              <JsonSchemaCodeEditor
                value={schemaText}
                onChange={updateSchemaText}
              />
            )
          }
        ]}
      />
      <Typography.Text type={schemaError ? 'danger' : 'secondary'}>
        {schemaError ?? i18nText('agentFlow', 'auto.json_schema_parse_hint')}
      </Typography.Text>
    </div>
  );
});

JsonSchemaEditorContent.displayName = 'JsonSchemaEditorContent';

export function JsonSchemaInlineEditor({
  schema,
  fallbackRootType = 'object',
  resetKey,
  structureMode,
  parseSchemaInput,
  onChange,
  onValidityChange
}: {
  schema: Record<string, unknown>;
  fallbackRootType?: JsonSchemaRootType;
  resetKey?: string | number | null;
  structureMode?: 'schema' | 'fields';
  parseSchemaInput?: (input: string) => JsonSchemaEditorResult;
  onChange: (schema: Record<string, unknown>) => void;
  onValidityChange?: (valid: boolean) => void;
}) {
  return (
    <JsonSchemaEditorContent
      active
      className="agent-flow-json-schema-settings--inline"
      fallbackRootType={fallbackRootType}
      live
      parseSchemaInput={parseSchemaInput}
      resetKey={resetKey}
      schema={schema}
      structureMode={structureMode}
      onChange={onChange}
      onValidityChange={onValidityChange}
    />
  );
}

export function JsonSchemaSettingsPanel({
  open,
  schema,
  fallbackRootType = 'object',
  className,
  title = 'JSON Schema',
  triggerRef,
  onClose,
  onSave
}: JsonSchemaSettingsPanelProps) {
  const editorRef = useRef<JsonSchemaEditorContentHandle>(null);

  function saveSchema() {
    const parsed = editorRef.current?.getSchema();

    if (!parsed?.ok) {
      return;
    }

    onSave(parsed.schema);
  }

  const schemaPanelFooter = (
    <div className="agent-flow-json-schema-settings__footer">
      <Button
        aria-label={i18nText('agentFlow', 'auto.cancel')}
        onClick={onClose}
      >
        {i18nText('agentFlow', 'auto.cancel')}
      </Button>
      <Button
        aria-label={i18nText('agentFlow', 'auto.save')}
        type="primary"
        onClick={saveSchema}
      >
        {i18nText('agentFlow', 'auto.save')}
      </Button>
    </div>
  );

  return (
    <FloatingSettingsPanel
      className={['agent-flow-json-schema-settings__panel', className]
        .filter(Boolean)
        .join(' ')}
      closeLabel={i18nText('agentFlow', 'auto.close_json_schema_settings')}
      defaultWidth={720}
      initialHeight={560}
      minHeight={420}
      minWidth={620}
      open={open}
      title={title}
      triggerRef={triggerRef}
      footer={schemaPanelFooter}
      dragHandleTestId="agent-flow-json-schema-settings-drag-handle"
      leftResizeHandleTestId="agent-flow-json-schema-settings-resize-left"
      rightResizeHandleTestId="agent-flow-json-schema-settings-resize-right"
      bottomResizeHandleTestId="agent-flow-json-schema-settings-resize-bottom"
      onClose={onClose}
    >
      <JsonSchemaEditorContent
        ref={editorRef}
        active={open}
        fallbackRootType={fallbackRootType}
        schema={schema}
      />
    </FloatingSettingsPanel>
  );
}
