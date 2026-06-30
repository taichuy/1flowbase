import {
  Alert,
  Button,
  Checkbox,
  Flex,
  Form,
  Input,
  InputNumber,
  Space,
  Typography
} from 'antd';
import { useMemo, useState } from 'react';

import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';
import {
  normalizeInputMapping,
  type McpInputMappingValue,
  type McpInputParameterMapping
} from './mcp-input-mapping-model';
import type {
  ExecuteSettingsMcpToolDebugBody,
  SettingsMcpToolDebugExecuteResponse
} from '../../api/mcp-management';

type DebugField = McpInputParameterMapping & {
  field_type: string;
};

function formatJson(value: unknown) {
  return JSON.stringify(value ?? {}, null, 2);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function setArgumentPathValue(
  target: Record<string, unknown>,
  path: string,
  value: unknown
) {
  const segments = path.split('.').filter(Boolean);
  if (segments.length === 0) {
    return;
  }

  let cursor = target;
  for (const segment of segments.slice(0, -1)) {
    const current = cursor[segment];
    if (!isRecord(current)) {
      cursor[segment] = {};
    }
    cursor = cursor[segment] as Record<string, unknown>;
  }
  cursor[segments[segments.length - 1]] = value;
}

function buildDebugFields(inputMapping: McpInputMappingValue): DebugField[] {
  return inputMapping.mappings.map((mapping) => {
    const interfaceParameter = inputMapping.interface_parameters.find(
      (parameter) => parameter.name === mapping.interface_param
    );

    return {
      ...mapping,
      field_type: interfaceParameter?.field_type ?? ''
    };
  });
}

function debugFieldExtra(field: DebugField) {
  const description = field.description.trim();
  if (!description || description === field.mcp_param) {
    return undefined;
  }

  return description;
}

function debugFieldKind(fieldType: string) {
  const normalized = fieldType.toLowerCase();
  if (normalized.includes('bool')) {
    return 'boolean';
  }
  if (
    normalized.includes('int') ||
    normalized.includes('float') ||
    normalized.includes('double') ||
    normalized.includes('number')
  ) {
    return 'number';
  }
  if (
    normalized.includes('object') ||
    normalized.includes('array') ||
    normalized.includes('json')
  ) {
    return 'json';
  }

  return 'string';
}

function isBlankValue(value: unknown) {
  return typeof value === 'undefined' || value === null || value === '';
}

function buildMcpArguments(
  fields: DebugField[],
  argumentValues: Record<string, unknown>
) {
  const mcpArguments: Record<string, unknown> = {};
  for (const field of fields) {
    const rawValue = argumentValues[field.mcp_param];
    if (field.required && isBlankValue(rawValue)) {
      throw new Error(`${field.mcp_param} 是必填参数`);
    }

    if (isBlankValue(rawValue)) {
      continue;
    }

    if (debugFieldKind(field.field_type) === 'json') {
      try {
        setArgumentPathValue(
          mcpArguments,
          field.mcp_param,
          typeof rawValue === 'string' ? JSON.parse(rawValue) : rawValue
        );
      } catch {
        throw new Error(`${field.mcp_param} 请输入有效 JSON`);
      }
      continue;
    }

    setArgumentPathValue(mcpArguments, field.mcp_param, rawValue);
  }

  return mcpArguments;
}

export function McpToolDebugPanel({
  csrfToken = '',
  executeDebug,
  inputMapping,
  interfaceId,
  operationLabel,
  outputMapping
}: {
  csrfToken?: string;
  executeDebug?: (
    body: ExecuteSettingsMcpToolDebugBody,
    csrfToken: string
  ) => Promise<SettingsMcpToolDebugExecuteResponse>;
  inputMapping: unknown;
  interfaceId?: string | null;
  operationLabel?: string | null;
  outputMapping: Record<string, unknown>;
}) {
  const normalizedInputMapping = useMemo(
    () => normalizeInputMapping(inputMapping),
    [inputMapping]
  );
  const debugFields = useMemo(
    () => buildDebugFields(normalizedInputMapping),
    [normalizedInputMapping]
  );
  const [argumentValues, setArgumentValues] = useState<Record<string, unknown>>(
    {}
  );
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [debugResult, setDebugResult] = useState<unknown>(null);
  const [debugRunning, setDebugRunning] = useState(false);

  const setArgumentValue = (name: string, value: unknown) => {
    setArgumentValues((current) => ({
      ...current,
      [name]: value
    }));
  };

  const runDebug = async () => {
    try {
      if (!interfaceId || !executeDebug) {
        throw new Error('interface_id 是必填参数');
      }
      const mcpArguments = buildMcpArguments(debugFields, argumentValues);
      setDebugRunning(true);
      const result = await executeDebug(
        {
          interface_id: interfaceId,
          mcp_arguments: mcpArguments,
          input_mapping: normalizedInputMapping,
          output_mapping: outputMapping
        },
        csrfToken
      );
      setDebugResult(result);
      setErrorMessage(null);
    } catch (error) {
      setDebugResult(null);
      setErrorMessage(error instanceof Error ? error.message : String(error));
    } finally {
      setDebugRunning(false);
    }
  };

  const renderFieldInput = (field: DebugField) => {
    const value = argumentValues[field.mcp_param];
    const kind = debugFieldKind(field.field_type);

    if (kind === 'boolean') {
      return (
        <Checkbox
          aria-label={field.mcp_param}
          checked={Boolean(value)}
          onChange={(event) =>
            setArgumentValue(field.mcp_param, event.target.checked)
          }
        />
      );
    }

    if (kind === 'number') {
      return (
        <InputNumber
          aria-label={field.mcp_param}
          style={{ width: '100%' }}
          value={typeof value === 'number' ? value : null}
          onChange={(nextValue) => setArgumentValue(field.mcp_param, nextValue)}
        />
      );
    }

    if (kind === 'json') {
      return (
        <Input.TextArea
          aria-label={field.mcp_param}
          rows={3}
          value={typeof value === 'string' ? value : ''}
          onChange={(event) =>
            setArgumentValue(field.mcp_param, event.target.value)
          }
        />
      );
    }

    return (
      <Input
        aria-label={field.mcp_param}
        value={typeof value === 'string' ? value : ''}
        onChange={(event) =>
          setArgumentValue(field.mcp_param, event.target.value)
        }
      />
    );
  };

  return (
    <Space className="mcp-tool-debug-panel" direction="vertical" size={12}>
      <Flex
        className="mcp-tool-debug-panel__header"
        align="center"
        aria-label="调试操作"
        gap={12}
        justify={operationLabel ? 'space-between' : 'flex-end'}
        role="group"
      >
        {operationLabel ? (
          <Typography.Text className="mcp-tool-debug-panel__operation" ellipsis>
            {operationLabel}
          </Typography.Text>
        ) : null}
        <Button
          aria-label="运行"
          disabled={debugFields.length === 0 || !interfaceId || !executeDebug}
          loading={debugRunning}
          type="primary"
          onClick={() => void runDebug()}
        >
          运行
        </Button>
      </Flex>
      {debugFields.length > 0 ? (
        <div className="mcp-tool-debug-panel__fields">
          {debugFields.map((field) => (
            <Form.Item
              className="mcp-tool-debug-panel__field"
              key={`${field.interface_param}:${field.mcp_param}`}
              label={field.mcp_param}
              required={field.required}
              extra={debugFieldExtra(field)}
            >
              {renderFieldInput(field)}
            </Form.Item>
          ))}
        </div>
      ) : (
        <Alert type="info" message="先在 input_mapping 添加 MCP 参数映射" />
      )}
      {errorMessage ? <Alert type="error" message={errorMessage} /> : null}
      {debugResult ? (
        <JsonPreviewBlock
          title="返回值"
          value={debugResult}
          collapsible={false}
          height="240px"
          rawText={formatJson(debugResult)}
        />
      ) : null}
    </Space>
  );
}
