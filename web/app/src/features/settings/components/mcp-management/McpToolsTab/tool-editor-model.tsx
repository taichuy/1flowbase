import type {
  ConsoleMcpInterfaceCapability,
  ConsoleMcpProxyInputMapping,
  ConsoleMcpProxyOutputMapping,
  ConsoleMcpTool,
  SaveConsoleMcpToolBody,
  UpdateConsoleMcpToolBody
} from '@1flowbase/api-client';
import { Typography } from 'antd';
import { i18nText } from '../../../../../shared/i18n/text';
import type { McpInputMappingValue } from '../mcp-input-mapping-model';

export type ToolFormValues = {
  tool_id: string;
  des_id: string;
  name: string;
  short_description: string;
  full_description: string;
  execution_target_kind: 'interface_wrapper' | 'mcp_proxy';
  interface_id?: string;
  upstream_connection_id?: string;
  remote_tool_name?: string;
  source_schema_hash?: string;
  input_mapping: McpInputMappingValue | ConsoleMcpProxyInputMapping;
  output_mapping: Record<string, unknown> | ConsoleMcpProxyOutputMapping;
  parameter_schema: Record<string, unknown>;
  result_schema: Record<string, unknown>;
  risk_level: string;
  status: string;
};

export const TOOL_FORM_STEPS = [
  { title: 'basic', label: 'basic', value: 'basic' },
  { title: 'interface', label: 'interface', value: 'interface' },
  { title: 'input', label: 'input_mapping', value: 'input' },
  { title: 'output', label: 'output_mapping', value: 'output' },
  { title: 'debug', label: 'debug', value: 'debug' }
];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

export function emptyObjectSchema(): Record<string, unknown> {
  return {
    type: 'object',
    properties: {},
    additionalProperties: false
  };
}

export function schemaRecord(value: unknown): Record<string, unknown> {
  return isRecord(value) ? value : emptyObjectSchema();
}

export function toUpdateToolBody(
  body: SaveConsoleMcpToolBody
): UpdateConsoleMcpToolBody {
  const updateBody = { ...body } as Partial<SaveConsoleMcpToolBody>;
  delete updateBody.tool_id;
  return updateBody as UpdateConsoleMcpToolBody;
}

export function interfaceOptionLabel(entry: ConsoleMcpInterfaceCapability) {
  return `${entry.method} ${entry.path}`;
}

export function toolTypeLabel(tool: ConsoleMcpTool) {
  return tool.execution_target.kind === 'mcp_proxy'
    ? i18nText('settingsMcpManagement', 'auto.tool_type_mcp_proxy')
    : i18nText('settingsMcpManagement', 'auto.tool_type_interface_wrapper');
}

export function toolSourceLabel(tool: ConsoleMcpTool) {
  return tool.execution_target.kind === 'mcp_proxy'
    ? `${tool.execution_target.upstream_connection_id} / ${tool.execution_target.remote_tool_name}`
    : tool.execution_target.interface_id;
}

export function SelectedInterfaceOperationTitle({
  selectedInterface
}: {
  selectedInterface: ConsoleMcpInterfaceCapability | undefined;
}) {
  if (!selectedInterface) return null;
  return (
    <Typography.Text>{interfaceOptionLabel(selectedInterface)}</Typography.Text>
  );
}

export function schemaMappingHasContent(value: unknown): boolean {
  if (!isRecord(value)) return false;

  const properties = value.properties;
  if (isRecord(properties) && Object.keys(properties).length > 0) return true;
  if (Array.isArray(value.required) && value.required.length > 0) return true;
  if (isRecord(value.items) && schemaMappingHasContent(value.items))
    return true;

  return Object.entries(value).some(([key, entry]) => {
    if (key === 'type' && (entry === 'object' || entry === 'array')) {
      return false;
    }
    if (
      key === 'properties' &&
      isRecord(entry) &&
      Object.keys(entry).length === 0
    ) {
      return false;
    }
    if (key === 'additionalProperties' && entry === false) return false;
    if (Array.isArray(entry) && entry.length === 0) return false;
    return entry !== undefined;
  });
}
