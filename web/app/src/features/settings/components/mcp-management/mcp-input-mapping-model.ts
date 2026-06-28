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
};

export type McpInputParameterMapping = {
  interface_param: string;
  mcp_param: string;
  description: string;
  required: boolean;
};

export type McpInputMappingValue = {
  interface_parameters: McpInputInterfaceParameter[];
  mappings: McpInputParameterMapping[];
};

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
    required: booleanValue(value.required)
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
    required: booleanValue(value.required)
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
  const interfaceParameters = descriptors.map((descriptor) => ({
    name: descriptor.name,
    field_type: descriptor.field_type,
    parameter_type: descriptor.parameter_type,
    description: descriptor.description ?? '',
    required: descriptor.required
  }));

  return {
    interface_parameters: interfaceParameters,
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
  const currentMappings = new Map(
    normalizeInputMapping(currentValue).mappings.map((mapping) => [
      mapping.interface_param,
      mapping
    ])
  );

  return {
    ...nextMapping,
    mappings: nextMapping.interface_parameters.flatMap((parameter) => {
      const mapping = currentMappings.get(parameter.name);
      return mapping ? [mapping] : [];
    })
  };
}

export function inputMappingHasContent(value: unknown): boolean {
  const mapping = normalizeInputMapping(value);
  return (
    mapping.interface_parameters.length > 0 ||
    mapping.mappings.some(
      (entry) =>
        entry.interface_param ||
        entry.mcp_param ||
        entry.description ||
        entry.required
    )
  );
}
