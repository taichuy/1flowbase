import type { FlowNodeOutputDocument } from '@1flowbase/flow-schema';

export const LLM_CONTEXT_MESSAGES_JSON_SCHEMA = {
  type: 'array',
  items: {
    type: 'object',
    required: ['role', 'content'],
    properties: {
      role: {
        type: 'string',
        enum: ['system', 'user', 'assistant', 'tool']
      },
      content: { type: 'string' },
      name: { type: 'string' },
      tool_call_id: { type: 'string' },
      tool_calls: { type: 'array' },
      content_blocks: { type: 'array' }
    }
  }
} satisfies Record<string, unknown>;

export function outputTypeSupportsJsonSchema(valueType: string) {
  return valueType === 'object' || valueType === 'array';
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function schemaType(value: unknown) {
  return isRecord(value) && typeof value.type === 'string'
    ? value.type
    : undefined;
}

function inferJsonSchemaFromValue(value: unknown): Record<string, unknown> {
  if (Array.isArray(value)) {
    return {
      type: 'array',
      items:
        value.length > 0 ? inferJsonSchemaFromValue(value[0]) : {}
    };
  }

  if (isRecord(value)) {
    const properties = Object.fromEntries(
      Object.entries(value).map(([key, entry]) => [
        key,
        inferJsonSchemaFromValue(entry)
      ])
    );

    return {
      type: 'object',
      required: Object.keys(properties),
      properties
    };
  }

  if (typeof value === 'string') {
    return { type: 'string' };
  }

  if (typeof value === 'number') {
    return { type: 'number' };
  }

  if (typeof value === 'boolean') {
    return { type: 'boolean' };
  }

  return {};
}

export function parseJsonSchemaInput(input: string): {
  ok: true;
  schema: Record<string, unknown>;
} | {
  ok: false;
  message: string;
} {
  try {
    const parsed = JSON.parse(input);

    if (isRecord(parsed) && typeof parsed.type === 'string') {
      return { ok: true, schema: parsed };
    }

    return { ok: true, schema: inferJsonSchemaFromValue(parsed) };
  } catch {
    return { ok: false, message: '请输入合法 JSON 或 JSON Schema' };
  }
}

export function outputHasLlmContextSchema(
  output: Pick<FlowNodeOutputDocument, 'valueType' | 'jsonSchema'>
) {
  return (
    (output.valueType === 'array' || output.valueType === 'array[object]') &&
    isLlmContextJsonSchema(output.jsonSchema)
  );
}

function isLlmContextJsonSchema(schema: unknown): boolean {
  if (!isRecord(schema) || schemaType(schema) !== 'array') {
    return false;
  }

  const items = schema.items;
  if (!isRecord(items) || schemaType(items) !== 'object') {
    return false;
  }

  const required = Array.isArray(items.required)
    ? items.required.filter((entry): entry is string => typeof entry === 'string')
    : [];
  if (!required.includes('role') || !required.includes('content')) {
    return false;
  }

  const properties = isRecord(items.properties) ? items.properties : {};
  const role = properties.role;
  const content = properties.content;

  return schemaType(role) === 'string' && schemaType(content) === 'string';
}
