import { Alert, Button, Form, Input, Space } from 'antd';
import { useState } from 'react';

import { JsonPreviewBlock } from '../../../../shared/ui/json-preview/JsonPreviewBlock';
import {
  normalizeInputMapping,
  type McpInputMappingValue
} from './mcp-input-mapping-model';

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function formatJson(value: unknown) {
  return JSON.stringify(value ?? {}, null, 2);
}

function parseMcpArguments(text: string) {
  const value = JSON.parse(text || '{}') as unknown;
  if (!isRecord(value)) {
    throw new Error('请输入 JSON 对象');
  }

  return value;
}

function buildInterfaceArguments(
  inputMapping: McpInputMappingValue,
  mcpArguments: Record<string, unknown>
) {
  const interfaceArguments: Record<string, unknown> = {};
  for (const mapping of inputMapping.mappings) {
    if (Object.prototype.hasOwnProperty.call(mcpArguments, mapping.mcp_param)) {
      interfaceArguments[mapping.interface_param] = mcpArguments[mapping.mcp_param];
    }
  }

  return interfaceArguments;
}

export function McpToolDebugPanel({
  inputMapping,
  outputMapping
}: {
  inputMapping: unknown;
  outputMapping: Record<string, unknown>;
}) {
  const [mcpArgumentsText, setMcpArgumentsText] = useState('{}');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [debugResult, setDebugResult] = useState<unknown>(null);

  const runDebug = () => {
    try {
      const mcpArguments = parseMcpArguments(mcpArgumentsText);
      const normalizedInputMapping = normalizeInputMapping(inputMapping);
      setDebugResult({
        mcp_arguments: mcpArguments,
        interface_arguments: buildInterfaceArguments(
          normalizedInputMapping,
          mcpArguments
        ),
        output_mapping: outputMapping
      });
      setErrorMessage(null);
    } catch (error) {
      setDebugResult(null);
      setErrorMessage(error instanceof Error ? error.message : String(error));
    }
  };

  return (
    <Space className="mcp-tool-debug-panel" direction="vertical" size={12}>
      <Form.Item label="MCP 参数 JSON">
        <Input.TextArea
          aria-label="MCP 参数 JSON"
          rows={5}
          value={mcpArgumentsText}
          onChange={(event) => setMcpArgumentsText(event.target.value)}
        />
      </Form.Item>
      <Space>
        <Button aria-label="运行" type="primary" onClick={runDebug}>
          运行
        </Button>
      </Space>
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
