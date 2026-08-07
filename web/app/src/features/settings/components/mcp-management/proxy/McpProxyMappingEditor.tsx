import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import { Button, Checkbox, Flex, Input, Space, Typography } from 'antd';
import { useEffect } from 'react';
import type {
  ConsoleMcpProxyInputMapping,
  ConsoleMcpProxyOutputMapping
} from '@1flowbase/api-client';

import { i18nText } from '../../../../../shared/i18n/text';

const MCP_PROXY_PATH_PATTERN =
  /^[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)*$/;

type ProxyMapping = ConsoleMcpProxyInputMapping | ConsoleMcpProxyOutputMapping;

export function mcpProxyMappingIsValid(value: ProxyMapping) {
  return value.mappings.every(
    (entry) =>
      MCP_PROXY_PATH_PATTERN.test(entry.local_path) &&
      MCP_PROXY_PATH_PATTERN.test(entry.remote_path)
  );
}

const textByKey = {
  proxy_add_mapping: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_add_mapping'),
  proxy_delete_mapping: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_delete_mapping'),
  proxy_input_mapping_title: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_input_mapping_title'),
  proxy_output_mapping_title: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_output_mapping_title'),
  proxy_path_format_hint: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_path_format_hint'),
  proxy_path_invalid: () =>
    i18nText('settingsMcpManagement', 'auto.proxy_path_invalid'),
  proxy_required: () => i18nText('settingsMcpManagement', 'auto.proxy_required')
};

function text(key: keyof typeof textByKey) {
  return textByKey[key]();
}

export function McpProxyMappingEditor({
  direction,
  value,
  onChange,
  onValidityChange
}: {
  direction: 'input' | 'output';
  value: ProxyMapping;
  onChange: (value: ProxyMapping) => void;
  onValidityChange?: (valid: boolean) => void;
}) {
  const valid = mcpProxyMappingIsValid(value);

  useEffect(() => {
    onValidityChange?.(valid);
  }, [onValidityChange, valid]);

  const updateEntry = (
    index: number,
    patch: Partial<ProxyMapping['mappings'][number]>
  ) => {
    onChange({
      mappings: value.mappings.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, ...patch } : entry
      )
    });
  };

  return (
    <Space orientation="vertical" size="small" className="mcp-management__stack">
      <Flex gap={8} align="center">
        <Typography.Text strong>
          {direction === 'input'
            ? text('proxy_input_mapping_title')
            : text('proxy_output_mapping_title')}
        </Typography.Text>
        <Typography.Text type="secondary">
          {text('proxy_path_format_hint')}
        </Typography.Text>
      </Flex>
      {value.mappings.map((entry, index) => (
        <Flex key={index} gap={8} align="start" wrap="wrap">
          <Input
            aria-label={`${direction === 'input' ? 'local_path' : 'remote_path'} ${index + 1}`}
            placeholder={direction === 'input' ? 'local_path' : 'remote_path'}
            status={
              (direction === 'input' ? entry.local_path : entry.remote_path) &&
              !MCP_PROXY_PATH_PATTERN.test(
                direction === 'input' ? entry.local_path : entry.remote_path
              )
                ? 'error'
                : undefined
            }
            value={direction === 'input' ? entry.local_path : entry.remote_path}
            onChange={(event) =>
              updateEntry(
                index,
                direction === 'input'
                  ? { local_path: event.target.value }
                  : { remote_path: event.target.value }
              )
            }
          />
          <Typography.Text aria-hidden>→</Typography.Text>
          <Input
            aria-label={`${direction === 'input' ? 'remote_path' : 'local_path'} ${index + 1}`}
            placeholder={direction === 'input' ? 'remote_path' : 'local_path'}
            status={
              (direction === 'input' ? entry.remote_path : entry.local_path) &&
              !MCP_PROXY_PATH_PATTERN.test(
                direction === 'input' ? entry.remote_path : entry.local_path
              )
                ? 'error'
                : undefined
            }
            value={direction === 'input' ? entry.remote_path : entry.local_path}
            onChange={(event) =>
              updateEntry(
                index,
                direction === 'input'
                  ? { remote_path: event.target.value }
                  : { local_path: event.target.value }
              )
            }
          />
          <Checkbox
            aria-label={`${text('proxy_required')} ${index + 1}`}
            checked={entry.required}
            onChange={(event) =>
              updateEntry(index, { required: event.target.checked })
            }
          >
            {text('proxy_required')}
          </Checkbox>
          <Button
            aria-label={`${text('proxy_delete_mapping')} ${index + 1}`}
            danger
            icon={<DeleteOutlined />}
            onClick={() =>
              onChange({
                mappings: value.mappings.filter(
                  (_, entryIndex) => entryIndex !== index
                )
              })
            }
          />
        </Flex>
      ))}
      <Button
        icon={<PlusOutlined />}
        onClick={() =>
          onChange({
            mappings: [
              ...value.mappings,
              { local_path: '', remote_path: '', required: false }
            ]
          })
        }
      >
        {text('proxy_add_mapping')}
      </Button>
      {!valid ? (
        <Typography.Text type="danger">
          {text('proxy_path_invalid')}
        </Typography.Text>
      ) : null}
    </Space>
  );
}
