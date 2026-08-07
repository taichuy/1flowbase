import { Alert, Button, Form, Input, Space } from 'antd';
import { useState } from 'react';
import type {
  ConsoleMcpProxyInputMapping,
  ConsoleMcpProxyToolDebugResponse,
  ExecuteConsoleMcpProxyToolDebugBody
} from '@1flowbase/api-client';

import { JsonPreviewBlock } from '../../../../../shared/ui/json-preview/JsonPreviewBlock';
import { i18nText } from '../../../../../shared/i18n/text';

const textByKey = {
  proxy_local_arguments: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_local_arguments'),
  proxy_mapped_result: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_mapped_result'),
  proxy_remote_arguments: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_remote_arguments'),
  proxy_required: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_required'),
  proxy_run_debug: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_run_debug'),
  proxy_upstream_result: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_upstream_result')
};

function text(key: keyof typeof textByKey) {
  return textByKey[key]();
}

function setPath(
  target: Record<string, unknown>,
  path: string,
  value: unknown
) {
  const segments = path.split('.');
  let cursor = target;
  segments.slice(0, -1).forEach((segment) => {
    const current = cursor[segment];
    if (
      typeof current !== 'object' ||
      current === null ||
      Array.isArray(current)
    ) {
      cursor[segment] = {};
    }
    cursor = cursor[segment] as Record<string, unknown>;
  });
  cursor[segments[segments.length - 1]] = value;
}

export function McpProxyToolDebugPanel({
  toolId,
  csrfToken,
  inputMapping,
  executeDebug
}: {
  toolId: string;
  csrfToken: string;
  inputMapping: ConsoleMcpProxyInputMapping;
  executeDebug: (
    toolId: string,
    body: ExecuteConsoleMcpProxyToolDebugBody,
    csrfToken: string
  ) => Promise<ConsoleMcpProxyToolDebugResponse>;
}) {
  const [values, setValues] = useState<Record<string, string>>({});
  const [result, setResult] = useState<ConsoleMcpProxyToolDebugResponse | null>(
    null
  );
  const [error, setError] = useState<string | null>(null);
  const [running, setRunning] = useState(false);

  const run = async () => {
    const argumentsValue: Record<string, unknown> = {};
    for (const mapping of inputMapping.mappings) {
      const value = values[mapping.local_path];
      if (mapping.required && !value) {
        setError(`${mapping.local_path}: ${text('proxy_required')}`);
        return;
      }
      if (value) setPath(argumentsValue, mapping.local_path, value);
    }

    setRunning(true);
    try {
      setResult(
        await executeDebug(toolId, { arguments: argumentsValue }, csrfToken)
      );
      setError(null);
    } catch (runError) {
      setResult(null);
      setError(runError instanceof Error ? runError.message : String(runError));
    } finally {
      setRunning(false);
    }
  };

  return (
    <Space orientation="vertical" size="middle" className="mcp-management__stack">
      {inputMapping.mappings.map((mapping) => (
        <Form.Item
          key={mapping.local_path}
          label={mapping.local_path}
          required={mapping.required}
        >
          <Input
            aria-label={mapping.local_path}
            value={values[mapping.local_path] ?? ''}
            onChange={(event) =>
              setValues((current) => ({
                ...current,
                [mapping.local_path]: event.target.value
              }))
            }
          />
        </Form.Item>
      ))}
      <Button type="primary" loading={running} onClick={() => void run()}>
        {text('proxy_run_debug')}
      </Button>
      {error ? <Alert type="error" title={error} /> : null}
      {result ? (
        <div className="mcp-management__proxy-debug-results">
          <JsonPreviewBlock
            title={text('proxy_local_arguments')}
            value={result.local_arguments}
            collapsible={false}
            height="160px"
          />
          <JsonPreviewBlock
            title={text('proxy_remote_arguments')}
            value={result.remote_arguments}
            collapsible={false}
            height="160px"
          />
          <JsonPreviewBlock
            title={text('proxy_upstream_result')}
            value={result.upstream_result}
            collapsible={false}
            height="160px"
          />
          <JsonPreviewBlock
            title={text('proxy_mapped_result')}
            value={result.mapped_result}
            collapsible={false}
            height="160px"
          />
        </div>
      ) : null}
    </Space>
  );
}
