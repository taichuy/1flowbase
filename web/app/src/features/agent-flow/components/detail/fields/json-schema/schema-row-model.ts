import type { editor } from 'monaco-editor';

import type { JsonSchemaRootType } from './json-schema-utils';

export type SchemaEditorTab = 'fields' | 'json';
export type SchemaFieldType =
  | 'string'
  | 'number'
  | 'boolean'
  | 'object'
  | 'array';
export type SchemaArrayItemType = 'string' | 'number' | 'object';
export type SchemaEnumValueType = 'string' | 'number';
export type SchemaEnumArrayTypeOptionValue = `array_${SchemaEnumValueType}`;
export type SchemaFieldTypeOptionValue =
  | Exclude<SchemaFieldType, 'array'>
  | `array_${SchemaArrayItemType}`;

export const schemaFieldTypeOptions = [
  { value: 'string', label: 'String' },
  { value: 'number', label: 'Number' },
  { value: 'boolean', label: 'Boolean' },
  { value: 'object', label: 'Object' },
  { value: 'array_string', label: 'Array<String>' },
  { value: 'array_number', label: 'Array<Number>' },
  { value: 'array_object', label: 'Array<Object>' }
] satisfies Array<{ value: SchemaFieldTypeOptionValue; label: string }>;

export const schemaEnumValueTypeOptions = [
  { value: 'string', label: 'String' },
  { value: 'number', label: 'Number' }
] satisfies Array<{ value: SchemaEnumValueType; label: string }>;

export const schemaEnumArrayTypeOptions = [
  { value: 'array_string', label: 'Array<String>' },
  { value: 'array_number', label: 'Array<Number>' }
] satisfies Array<{ value: SchemaEnumArrayTypeOptionValue; label: string }>;

export interface SchemaFieldRow {
  id: string;
  key: string;
  type: SchemaFieldType;
  arrayItemType?: SchemaArrayItemType;
  description: string;
  required: boolean;
  children?: SchemaFieldRow[];
  enumValues?: string[];
  baseSchema?: Record<string, unknown>;
}

let schemaFieldRowIdSeed = 0;

export function createSchemaFieldRowId() {
  schemaFieldRowIdSeed += 1;
  return `schema-field-row-${schemaFieldRowIdSeed}`;
}

export const JSON_SCHEMA_EDITOR_OPTIONS = {
  automaticLayout: true,
  minimap: { enabled: false },
  fontSize: 13,
  lineHeight: 20,
  lineNumbersMinChars: 3,
  scrollBeyondLastLine: false,
  tabSize: 2,
  wordWrap: 'on',
  padding: {
    top: 12,
    bottom: 12
  },
  scrollbar: {
    verticalScrollbarSize: 8,
    horizontalScrollbarSize: 8
  }
} satisfies editor.IStandaloneEditorConstructionOptions;

export function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function stringifySchema(schema: Record<string, unknown>) {
  return JSON.stringify(schema, null, 2);
}

function schemaFieldType(value: unknown): SchemaFieldType {
  return ['string', 'number', 'boolean', 'object', 'array'].includes(
    String(value)
  )
    ? (value as SchemaFieldType)
    : 'string';
}

function schemaArrayItemType(
  schema: Record<string, unknown> | undefined
): SchemaArrayItemType {
  if (schema?.type === 'string' || schema?.type === 'number') {
    return schema.type;
  }

  return 'object';
}

function schemaEnumValues(
  schema: Record<string, unknown> | undefined,
  valueType: SchemaEnumValueType
) {
  if (!Array.isArray(schema?.enum)) {
    return undefined;
  }

  return schema.enum.every((value) => typeof value === valueType)
    ? schema.enum.map((value) => String(value))
    : undefined;
}

export function schemaFieldTypeOptionValue(
  row: SchemaFieldRow
): SchemaFieldTypeOptionValue {
  if (row.type === 'array') {
    return `array_${row.arrayItemType ?? 'object'}`;
  }

  return row.type;
}

export function schemaFieldTypePatch(
  value: SchemaFieldTypeOptionValue
): Pick<SchemaFieldRow, 'type'> &
  Partial<Pick<SchemaFieldRow, 'arrayItemType'>> {
  if (value === 'array_string') {
    return { type: 'array', arrayItemType: 'string' };
  }

  if (value === 'array_number') {
    return { type: 'array', arrayItemType: 'number' };
  }

  if (value === 'array_object') {
    return { type: 'array', arrayItemType: 'object' };
  }

  return { type: value };
}

export function schemaEnumValueTypeForRow(
  row: SchemaFieldRow
): SchemaEnumValueType | undefined {
  if (row.type === 'string' || row.type === 'number') {
    return row.type;
  }

  if (
    row.type === 'array' &&
    (row.arrayItemType === 'string' || row.arrayItemType === 'number')
  ) {
    return row.arrayItemType;
  }

  return undefined;
}

export function schemaEnumArrayTypeOptionValue(
  valueType: SchemaEnumValueType
): SchemaEnumArrayTypeOptionValue {
  return `array_${valueType}`;
}

export function schemaEnumValueTypeFromArrayOption(
  value: SchemaEnumArrayTypeOptionValue
): SchemaEnumValueType {
  return value === 'array_number' ? 'number' : 'string';
}

function propertySchemaForType(
  type: Exclude<SchemaFieldType, 'array'>
): Record<string, unknown> {
  if (type === 'object') {
    return { type: 'object', properties: {} };
  }

  return { type };
}

function schemaRowsFromObjectSchema(schema: unknown): SchemaFieldRow[] {
  if (!isRecord(schema)) {
    return [];
  }

  const properties = isRecord(schema.properties) ? schema.properties : {};
  const required = Array.isArray(schema.required)
    ? schema.required.filter(
        (entry): entry is string => typeof entry === 'string'
      )
    : [];

  return Object.entries(properties).map(([key, property]) =>
    schemaRowFromProperty(key, property, required.includes(key))
  );
}

function schemaRowFromProperty(
  key: string,
  property: unknown,
  required: boolean
): SchemaFieldRow {
  const baseSchema = isRecord(property) ? property : undefined;
  const type = schemaFieldType(baseSchema?.type);
  const description =
    baseSchema && typeof baseSchema.description === 'string'
      ? baseSchema.description
      : '';

  if (type === 'object') {
    return {
      id: createSchemaFieldRowId(),
      key,
      type,
      description,
      required,
      baseSchema,
      children: schemaRowsFromObjectSchema(property)
    };
  }

  if (type === 'array') {
    const items = isRecord(baseSchema?.items) ? baseSchema.items : undefined;
    const arrayItemType = schemaArrayItemType(items);
    const enumValueType =
      arrayItemType === 'string' || arrayItemType === 'number'
        ? arrayItemType
        : undefined;

    return {
      id: createSchemaFieldRowId(),
      key,
      type,
      arrayItemType,
      description,
      required,
      baseSchema,
      enumValues: enumValueType
        ? schemaEnumValues(items, enumValueType)
        : undefined,
      children:
        arrayItemType === 'object'
          ? schemaRowsFromObjectSchema(items)
          : undefined
    };
  }

  return {
    id: createSchemaFieldRowId(),
    key,
    type,
    description,
    required,
    enumValues:
      type === 'string' || type === 'number'
        ? schemaEnumValues(baseSchema, type)
        : undefined,
    baseSchema
  };
}

export function schemaRowsFromSchema(schema: unknown): SchemaFieldRow[] {
  if (!isRecord(schema)) {
    return [];
  }

  const root =
    schema.type === 'array' && isRecord(schema.items) ? schema.items : schema;
  if (!isRecord(root)) {
    return [];
  }

  return schemaRowsFromObjectSchema(root);
}

function withDescription(
  schema: Record<string, unknown>,
  description: string
) {
  const trimmed = description.trim();
  const nextSchema = { ...schema };

  if (trimmed) {
    nextSchema.description = trimmed;
  } else {
    delete nextSchema.description;
  }

  return nextSchema;
}

function compatibleBaseSchema(row: SchemaFieldRow) {
  if (row.type === 'array') {
    return row.baseSchema?.type === row.type &&
      schemaArrayItemType(
        isRecord(row.baseSchema.items) ? row.baseSchema.items : undefined
      ) === (row.arrayItemType ?? 'object')
      ? row.baseSchema
      : undefined;
  }

  return row.baseSchema?.type === row.type ? row.baseSchema : undefined;
}

export function typedEnumValues(
  enumValues: string[],
  valueType: SchemaEnumValueType
) {
  if (valueType === 'number') {
    return enumValues
      .map((value) => Number(value))
      .filter((value) => Number.isFinite(value));
  }

  return enumValues;
}

function withTypedEnum(
  schema: Record<string, unknown>,
  enumValues: string[] | undefined,
  valueType: SchemaEnumValueType | undefined
) {
  const nextSchema = { ...schema };

  if (enumValues === undefined || valueType === undefined) {
    return nextSchema;
  }

  const schemaValues = typedEnumValues(enumValues, valueType);

  if (schemaValues.length > 0) {
    nextSchema.enum = schemaValues;
  } else {
    delete nextSchema.enum;
  }

  return nextSchema;
}

function propertySchemaFromRow(
  row: SchemaFieldRow
): Record<string, unknown> {
  const baseSchema = compatibleBaseSchema(row);

  if (row.type === 'object') {
    return withDescription(
      objectSchemaFromRows(row.children ?? [], baseSchema),
      row.description
    );
  }

  if (row.type === 'array') {
    const arrayItemType = row.arrayItemType ?? 'object';
    const baseItemsSchema = isRecord(baseSchema?.items)
      ? baseSchema.items
      : undefined;
    const items =
      arrayItemType === 'object'
        ? objectSchemaFromRows(row.children ?? [], baseItemsSchema)
        : withTypedEnum(
            {
              ...(baseItemsSchema ?? {}),
              type: arrayItemType
            },
            row.enumValues,
            arrayItemType
          );

    return withDescription(
      {
        ...(baseSchema ?? {}),
        type: 'array',
        items
      },
      row.description
    );
  }

  return withDescription(
    withTypedEnum(
      {
        ...(baseSchema ?? {}),
        ...propertySchemaForType(row.type)
      },
      row.enumValues,
      schemaEnumValueTypeForRow(row)
    ),
    row.description
  );
}

export function objectSchemaFromRows(
  rows: SchemaFieldRow[],
  baseSchema?: Record<string, unknown>
) {
  const visibleRows = rows
    .map((row) => ({
      ...row,
      key: row.key.trim()
    }))
    .filter((row) => row.key.length > 0);
  const properties = Object.fromEntries(
    visibleRows.map((row) => [row.key, propertySchemaFromRow(row)])
  );
  const required = visibleRows
    .filter((row) => row.required)
    .map((row) => row.key);

  return {
    ...(baseSchema ?? {}),
    type: 'object',
    required,
    properties
  };
}

export function schemaFromRows(
  rootType: JsonSchemaRootType,
  rows: SchemaFieldRow[],
  baseSchema?: Record<string, unknown>
): Record<string, unknown> {
  const objectSchema =
    rootType === 'array'
      ? objectSchemaFromRows(
          rows,
          isRecord(baseSchema?.items) ? baseSchema.items : undefined
        )
      : objectSchemaFromRows(rows, baseSchema);

  if (rootType === 'array') {
    return {
      ...(baseSchema ?? {}),
      type: 'array',
      items: objectSchema
    };
  }

  return objectSchema;
}
