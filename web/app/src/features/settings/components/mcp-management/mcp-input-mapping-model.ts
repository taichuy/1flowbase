import type {
  ConsoleMcpInterfaceCapability,
  ConsoleMcpParameterDescriptor,
  ConsoleMcpParameterType
} from '@1flowbase/api-client';

export type McpInputInterfaceParameter = {
  name: string;
  field_type: string;
  parameter_type: ConsoleMcpParameterType;
  description: string;
  required: boolean;
  source?: { kind: 'mcp_call' };
};

export type McpInputParameterMapping = {
  interface_param: string;
  mcp_param: string;
  description: string;
  required: boolean;
  source?: { kind: 'mcp_call'; path: string };
};

export type McpInputMappingValue = {
  interface_parameters: McpInputInterfaceParameter[];
  mappings: McpInputParameterMapping[];
};

export const MCP_CALL_PARAMETERS: McpInputInterfaceParameter[] = [
  {
    name: 'des_id',
    field_type: 'string',
    parameter_type: 'json_body',
    description: '描述 ID；留空使用 Tool 默认值，例如 report_detail。',
    required: false,
    source: { kind: 'mcp_call' }
  },
  {
    name: 'max_inline_chars',
    field_type: 'integer',
    parameter_type: 'json_body',
    description: '返回字符预算；正整数，留空使用 Tool 默认值，例如 4000。',
    required: false,
    source: { kind: 'mcp_call' }
  },
  {
    name: 'response_fields',
    field_type: 'array<string>',
    parameter_type: 'json_body',
    description:
      '返回字段；JSON Pointer 数组，例如 ["/title", "/items/0/body"]。留空使用 Tool 默认值（未设置则沿用映射输出）；[] 不返回业务字段。',
    required: false,
    source: { kind: 'mcp_call' }
  }
];

export const emptyInputMapping: McpInputMappingValue = {
  interface_parameters: [],
  mappings: []
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function stringValue(value: unknown) {
  return typeof value === 'string' ? value : '';
}

function booleanValue(value: unknown) {
  return typeof value === 'boolean' ? value : false;
}

function parameterTypeValue(value: unknown): ConsoleMcpParameterType {
  return value === 'url' || value === 'form' || value === 'json_body'
    ? value
    : 'json_body';
}

function normalizeInterfaceParameter(
  value: unknown
): McpInputInterfaceParameter | null {
  if (!isRecord(value)) {
    return null;
  }
  const name = stringValue(value.name);
  if (!name) {
    return null;
  }

  return {
    name,
    field_type: stringValue(value.field_type),
    parameter_type: parameterTypeValue(value.parameter_type),
    description: stringValue(value.description),
    required: booleanValue(value.required),
    ...(isRecord(value.source) &&
    value.source.kind === 'mcp_call' &&
    MCP_CALL_PARAMETERS.some((parameter) => parameter.name === name)
      ? { source: { kind: 'mcp_call' as const } }
      : {})
  };
}

function normalizeMapping(value: unknown): McpInputParameterMapping | null {
  if (!isRecord(value)) {
    return null;
  }
  const interfaceParam = stringValue(value.interface_param);
  if (!interfaceParam) {
    return null;
  }

  return {
    interface_param: interfaceParam,
    mcp_param: stringValue(value.mcp_param) || interfaceParam,
    description: stringValue(value.description),
    required: booleanValue(value.required),
    ...(isRecord(value.source) &&
    value.source.kind === 'mcp_call' &&
    value.source.path === interfaceParam &&
    MCP_CALL_PARAMETERS.some((parameter) => parameter.name === interfaceParam)
      ? { source: { kind: 'mcp_call' as const, path: interfaceParam } }
      : {})
  };
}

export function normalizeInputMapping(value: unknown): McpInputMappingValue {
  if (!isRecord(value)) {
    return emptyInputMapping;
  }

  return {
    interface_parameters: Array.isArray(value.interface_parameters)
      ? value.interface_parameters
          .map(normalizeInterfaceParameter)
          .filter((parameter): parameter is McpInputInterfaceParameter =>
            Boolean(parameter)
          )
      : [],
    mappings: Array.isArray(value.mappings)
      ? value.mappings
          .map(normalizeMapping)
          .filter((mapping): mapping is McpInputParameterMapping =>
            Boolean(mapping)
          )
      : []
  };
}

export function buildInputMappingFromParameterDescriptors(
  descriptors: ConsoleMcpParameterDescriptor[]
): McpInputMappingValue {
  const interfaceParameters = descriptors
    .filter(
      (descriptor) =>
        !MCP_CALL_PARAMETERS.some(
          (parameter) => parameter.name === descriptor.name
        )
    )
    .map((descriptor) => ({
      name: descriptor.name,
      field_type: descriptor.field_type,
      parameter_type: descriptor.parameter_type,
      description: descriptor.description ?? '',
      required: descriptor.required
    }));

  return {
    interface_parameters: [...MCP_CALL_PARAMETERS, ...interfaceParameters],
    mappings: []
  };
}

export function buildInputMappingFromInterface(
  entry: ConsoleMcpInterfaceCapability,
  currentValue?: unknown
): McpInputMappingValue {
  const nextMapping = buildInputMappingFromParameterDescriptors(
    entry.parameter_descriptors
  );
  const current = normalizeInputMapping(currentValue);
  const currentMappings = new Map(
    current.mappings.map((mapping) => [mapping.interface_param, mapping])
  );

  return {
    ...nextMapping,
    mappings: nextMapping.interface_parameters.flatMap((parameter) => {
      const mapping = currentMappings.get(parameter.name);
      return mapping ? [withCallParameterDescription(mapping, parameter)] : [];
    })
  };
}

function withCallParameterDescription(
  mapping: McpInputParameterMapping,
  parameter: McpInputInterfaceParameter
): McpInputParameterMapping {
  return mapping.source?.kind === 'mcp_call' && !mapping.description
    ? { ...mapping, description: parameter.description }
    : mapping;
}

export function withLegacyCallMappings(value: unknown): McpInputMappingValue {
  const mapping = normalizeInputMapping(value);
  if (
    mapping.interface_parameters.some(
      (parameter) => parameter.source?.kind === 'mcp_call'
    )
  ) {
    return {
      interface_parameters: mapping.interface_parameters.map((parameter) => {
        const defaultParameter = MCP_CALL_PARAMETERS.find(
          (candidate) => candidate.name === parameter.name
        );
        return parameter.source?.kind === 'mcp_call' &&
          defaultParameter &&
          !parameter.description
          ? { ...parameter, description: defaultParameter.description }
          : parameter;
      }),
      mappings: mapping.mappings.map((entry) => {
        const defaultParameter = MCP_CALL_PARAMETERS.find(
          (candidate) => candidate.name === entry.interface_param
        );
        return defaultParameter
          ? withCallParameterDescription(entry, defaultParameter)
          : entry;
      })
    };
  }

  const controlNames = new Set(
    MCP_CALL_PARAMETERS.map((parameter) => parameter.name)
  );
  const legacySelection =
    isRecord(value) && Array.isArray(value.call_parameters)
      ? new Set(
          value.call_parameters.filter(
            (name): name is string => typeof name === 'string'
          )
        )
      : controlNames;
  return {
    interface_parameters: [
      ...MCP_CALL_PARAMETERS,
      ...mapping.interface_parameters.filter(
        (parameter) => !controlNames.has(parameter.name)
      )
    ],
    mappings: [
      ...MCP_CALL_PARAMETERS.filter((parameter) =>
        legacySelection.has(parameter.name)
      ).map((parameter) => ({
        interface_param: parameter.name,
        mcp_param: parameter.name,
        description: parameter.description,
        required: false,
        source: { kind: 'mcp_call' as const, path: parameter.name }
      })),
      ...mapping.mappings.filter(
        (entry) => !controlNames.has(entry.interface_param)
      )
    ]
  };
}

export function inputMappingHasContent(value: unknown): boolean {
  const mapping = normalizeInputMapping(value);
  return (
    mapping.interface_parameters.some(
      (parameter) => parameter.source?.kind !== 'mcp_call'
    ) ||
    mapping.mappings.some(
      (entry) =>
        entry.interface_param ||
        entry.mcp_param ||
        entry.description ||
        entry.required
    )
  );
}
