import type {
  FlowNodeDocument,
  FlowNodeOutputDocument,
  FlowStartInputField,
  FlowStartInputType
} from '@1flowbase/flow-schema';

export const startInputTypes = [
  { value: 'text', valueType: 'string' },
  { value: 'paragraph', valueType: 'string' },
  { value: 'select', valueType: 'string' },
  { value: 'number', valueType: 'number' },
  { value: 'checkbox', valueType: 'boolean' },
  { value: 'file', valueType: 'json' },
  { value: 'file_list', valueType: 'array[object]' },
  { value: 'url', valueType: 'string' }
] satisfies Array<{
  value: FlowStartInputType;
  valueType: FlowNodeOutputDocument['valueType'];
}>;

function isStartInputType(value: unknown): value is FlowStartInputType {
  return startInputTypes.some((option) => option.value === value);
}

export function getStartInputValueType(inputType: FlowStartInputType) {
  return (
    startInputTypes.find((option) => option.value === inputType)?.valueType ??
    'string'
  );
}

function normalizeString(value: unknown, fallback: string) {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : fallback;
}

function normalizeOptionalString(value: unknown) {
  return typeof value === 'string' && value.trim().length > 0
    ? value
    : undefined;
}

function normalizeDefaultValue(value: unknown, inputType: FlowStartInputType) {
  switch (inputType) {
    case 'number':
      return typeof value === 'number' && Number.isFinite(value)
        ? value
        : undefined;
    case 'checkbox':
      return typeof value === 'boolean' ? value : undefined;
    case 'file':
    case 'file_list':
      return undefined;
    case 'text':
    case 'paragraph':
    case 'select':
    case 'url':
      return typeof value === 'string' && value.length > 0 ? value : undefined;
  }
}

function normalizeMaxLength(value: unknown) {
  return typeof value === 'number' && Number.isInteger(value) && value > 0
    ? value
    : undefined;
}

function normalizeOptions(value: unknown) {
  return Array.isArray(value)
    ? value
        .filter((option): option is string => typeof option === 'string')
        .map((option) => option.trim())
        .filter(Boolean)
    : undefined;
}

function normalizeStartInputSource(value: unknown) {
  return value === 'path' ||
    value === 'query' ||
    value === 'body' ||
    value === 'form'
    ? value
    : undefined;
}

export function normalizeStartInputField(
  value: unknown,
  index: number
): FlowStartInputField {
  const source =
    typeof value === 'object' && value !== null
      ? (value as Record<string, unknown>)
      : {};
  const inputType = isStartInputType(source.inputType)
    ? source.inputType
    : 'text';
  const key = normalizeString(source.key, `input_${index + 1}`);

  return {
    key,
    label: normalizeString(source.label, key),
    inputType,
    valueType: getStartInputValueType(inputType),
    required: Boolean(source.required),
    placeholder: normalizeOptionalString(source.placeholder),
    defaultValue: normalizeDefaultValue(source.defaultValue, inputType),
    maxLength: normalizeMaxLength(source.maxLength),
    hidden: Boolean(source.hidden),
    options: normalizeOptions(source.options),
    source: normalizeStartInputSource(source.source)
  };
}

export function getStartInputFields(
  node: Pick<FlowNodeDocument, 'config'> | null | undefined
) {
  const rawFields = node?.config.input_fields;

  return Array.isArray(rawFields)
    ? rawFields.map((field, index) => normalizeStartInputField(field, index))
    : [];
}
