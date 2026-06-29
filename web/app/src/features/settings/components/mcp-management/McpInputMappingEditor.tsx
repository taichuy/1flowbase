import { DeleteOutlined, PlusOutlined } from '@ant-design/icons';
import {
  Button,
  Checkbox,
  Empty,
  Flex,
  Input,
  Select,
  Space,
  Tabs,
  Typography
} from 'antd';
import { useMemo, useState } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { InlineJsonCodeEditor } from '../../../agent-flow/components/detail/fields/json-schema/JsonSchemaSettingsPanel';
import {
  type McpInputInterfaceParameter,
  type McpInputMappingValue,
  type McpInputParameterMapping,
  normalizeInputMapping
} from './mcp-input-mapping-model';

const DES_ID_PARAMETER_NAME = 'des_id';

function stringifyMapping(value: McpInputMappingValue) {
  return JSON.stringify(value, null, 2);
}

function mappingFromInterfaceParameter(
  parameter: McpInputInterfaceParameter
): McpInputParameterMapping {
  return {
    interface_param: parameter.name,
    mcp_param: parameter.name,
    description: parameter.description,
    required: parameter.required
  };
}

function desIdInterfaceParameter(): McpInputInterfaceParameter {
  return {
    name: DES_ID_PARAMETER_NAME,
    field_type: 'string',
    parameter_type: 'json_body',
    description: DES_ID_PARAMETER_NAME,
    required: true
  };
}

function nextInterfaceParameterName(parameters: McpInputInterfaceParameter[]) {
  const names = new Set(parameters.map((parameter) => parameter.name));
  let index = parameters.length + 1;
  let name = `param_${index}`;

  while (names.has(name)) {
    index += 1;
    name = `param_${index}`;
  }

  return name;
}

function emptyInterfaceParameter(
  parameters: McpInputInterfaceParameter[]
): McpInputInterfaceParameter {
  return {
    name: nextInterfaceParameterName(parameters),
    field_type: 'string',
    parameter_type: 'json_body',
    description: '',
    required: false
  };
}

function parameterTypeOptions() {
  return [
    { label: 'URL', value: 'url' },
    { label: 'form', value: 'form' },
    {
      label: i18nText('settings', 'auto.json_request_body'),
      value: 'json_body'
    }
  ];
}

type JsonDraftState = {
  resetKey: string | number | null | undefined;
  serializedMapping: string;
  text: string;
  error: string;
};

function jsonDraftState(
  resetKey: string | number | null | undefined,
  serializedMapping: string
): JsonDraftState {
  return {
    resetKey,
    serializedMapping,
    text: serializedMapping,
    error: ''
  };
}

function InputMappingInterfaceSection({
  mapping,
  onAddInterfaceParameter,
  onUpdateInterfaceParameter,
  onRemoveInterfaceParameter
}: {
  mapping: McpInputMappingValue;
  onAddInterfaceParameter: () => void;
  onUpdateInterfaceParameter: (
    index: number,
    patch: Partial<McpInputInterfaceParameter>
  ) => void;
  onRemoveInterfaceParameter: (index: number) => void;
}) {
  return (
    <Space
      className="mcp-input-mapping-editor__stack"
      direction="vertical"
      size="middle"
    >
      <Flex justify="flex-end">
        <Button icon={<PlusOutlined />} onClick={onAddInterfaceParameter}>
          {i18nText('settings', 'auto.add_new_field')}
        </Button>
      </Flex>
      {mapping.interface_parameters.length > 0 ? (
        <div className="mcp-input-mapping-editor__table">
          <div className="mcp-input-mapping-editor__head">
            <span>{i18nText('settings', 'auto.field_name')}</span>
            <span>{i18nText('settings', 'auto.field_type')}</span>
            <span>{i18nText('settings', 'auto.parameter_type')}</span>
            <span>{i18nText('settings', 'auto.required')}</span>
            <span />
          </div>
          {mapping.interface_parameters.map((parameter, index) => (
            <div
              className="mcp-input-mapping-editor__row"
              key={`${parameter.name}:${index}`}
            >
              <Input
                aria-label={`field_name ${index + 1}`}
                value={parameter.name}
                onChange={(event) =>
                  onUpdateInterfaceParameter(index, {
                    name: event.target.value
                  })
                }
              />
              <Input
                aria-label={`field_type ${parameter.name || index + 1}`}
                value={parameter.field_type}
                onChange={(event) =>
                  onUpdateInterfaceParameter(index, {
                    field_type: event.target.value
                  })
                }
              />
              <Select
                aria-label={`parameter_type ${parameter.name || index + 1}`}
                options={parameterTypeOptions()}
                value={parameter.parameter_type}
                onChange={(nextParameterType) =>
                  onUpdateInterfaceParameter(index, {
                    parameter_type: nextParameterType
                  })
                }
              />
              <Checkbox
                aria-label={`required ${parameter.name || index + 1}`}
                checked={parameter.required}
                onChange={(event) =>
                  onUpdateInterfaceParameter(index, {
                    required: event.target.checked
                  })
                }
              />
              <Button
                aria-label={`delete_field ${parameter.name || index + 1}`}
                icon={<DeleteOutlined />}
                onClick={() => onRemoveInterfaceParameter(index)}
              />
            </div>
          ))}
        </div>
      ) : (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
      )}
    </Space>
  );
}

function InputMappingLayerSection({
  mapping,
  addableOptions,
  pendingInterfaceParam,
  onPendingInterfaceParamChange,
  onAddMapping,
  onUpdateMapping,
  onRemoveMapping
}: {
  mapping: McpInputMappingValue;
  addableOptions: Array<{ label: string; value: string }>;
  pendingInterfaceParam: string | undefined;
  onPendingInterfaceParamChange: (value: string | undefined) => void;
  onAddMapping: (interfaceParam: string | undefined) => void;
  onUpdateMapping: (
    index: number,
    patch: Partial<McpInputParameterMapping>
  ) => void;
  onRemoveMapping: (index: number) => void;
}) {
  return (
    <Space
      className="mcp-input-mapping-editor__stack"
      direction="vertical"
      size="middle"
    >
      <Flex
        align="center"
        className="mcp-input-mapping-editor__mapping-action"
        gap={8}
      >
        <Select
          aria-label="interface_param"
          placeholder="interface_param"
          options={addableOptions}
          value={pendingInterfaceParam}
          onChange={onPendingInterfaceParamChange}
        />
        <Button
          icon={<PlusOutlined />}
          disabled={!pendingInterfaceParam}
          onClick={() => onAddMapping(pendingInterfaceParam)}
        >
          {i18nText('settings', 'auto.add_mapping')}
        </Button>
      </Flex>
      {mapping.mappings.length > 0 ? (
        <div className="mcp-input-mapping-editor__table">
          <div className="mcp-input-mapping-editor__mapping-head">
            <span>{i18nText('settings', 'auto.interface_param')}</span>
            <span>{i18nText('settings', 'auto.mcp_param')}</span>
            <span>{i18nText('settings', 'auto.description')}</span>
            <span>{i18nText('settings', 'auto.required')}</span>
            <span />
          </div>
          {mapping.mappings.map((entry, index) => (
            <div
              className="mcp-input-mapping-editor__mapping-row"
              key={`${entry.interface_param}:${index}`}
            >
              <Input readOnly value={entry.interface_param} />
              <Input
                aria-label={`mcp_param ${entry.interface_param}`}
                value={entry.mcp_param}
                onChange={(event) =>
                  onUpdateMapping(index, {
                    mcp_param: event.target.value
                  })
                }
              />
              <Input
                aria-label={`description ${entry.interface_param}`}
                value={entry.description}
                onChange={(event) =>
                  onUpdateMapping(index, {
                    description: event.target.value
                  })
                }
              />
              <Checkbox
                aria-label={`required ${entry.interface_param}`}
                checked={entry.required}
                onChange={(event) =>
                  onUpdateMapping(index, {
                    required: event.target.checked
                  })
                }
              />
              <Button
                aria-label={`delete_mapping ${entry.interface_param}`}
                icon={<DeleteOutlined />}
                onClick={() => onRemoveMapping(index)}
              />
            </div>
          ))}
        </div>
      ) : (
        <Empty image={Empty.PRESENTED_IMAGE_SIMPLE} />
      )}
    </Space>
  );
}

function InputMappingJsonSection({
  jsonDraft,
  onUpdateJsonText
}: {
  jsonDraft: JsonDraftState;
  onUpdateJsonText: (nextText: string) => void;
}) {
  return (
    <Space direction="vertical" style={{ width: '100%' }}>
      <InlineJsonCodeEditor
        ariaLabel="input_mapping JSON"
        className="mcp-input-mapping-editor__json"
        value={jsonDraft.text}
        onChange={onUpdateJsonText}
      />
      <Typography.Text type={jsonDraft.error ? 'danger' : 'secondary'}>
        {jsonDraft.error || i18nText('settings', 'auto.support_json_parse')}
      </Typography.Text>
    </Space>
  );
}

export function McpInputMappingEditor({
  value,
  resetKey,
  onChange,
  onValidityChange
}: {
  value: unknown;
  resetKey?: string | number | null;
  onChange: (value: McpInputMappingValue) => void;
  onValidityChange?: (valid: boolean) => void;
}) {
  const mapping = useMemo(() => normalizeInputMapping(value), [value]);
  const serializedMapping = useMemo(() => stringifyMapping(mapping), [mapping]);
  const [jsonDraft, setJsonDraft] = useState(() =>
    jsonDraftState(resetKey, serializedMapping)
  );
  const [pendingInterfaceParam, setPendingInterfaceParam] = useState<
    string | undefined
  >();

  if (
    jsonDraft.resetKey !== resetKey ||
    jsonDraft.serializedMapping !== serializedMapping
  ) {
    setJsonDraft(jsonDraftState(resetKey, serializedMapping));
  }

  function emit(nextMapping: McpInputMappingValue) {
    const nextSerializedMapping = stringifyMapping(nextMapping);
    setJsonDraft(jsonDraftState(resetKey, nextSerializedMapping));
    onValidityChange?.(true);
    onChange(nextMapping);
  }

  function updateMapping(
    index: number,
    patch: Partial<McpInputParameterMapping>
  ) {
    emit({
      ...mapping,
      mappings: mapping.mappings.map((entry, entryIndex) =>
        entryIndex === index ? { ...entry, ...patch } : entry
      )
    });
  }

  function addInterfaceParameter() {
    emit({
      ...mapping,
      interface_parameters: [
        ...mapping.interface_parameters,
        emptyInterfaceParameter(mapping.interface_parameters)
      ]
    });
  }

  function updateInterfaceParameter(
    index: number,
    patch: Partial<McpInputInterfaceParameter>
  ) {
    const currentParameter = mapping.interface_parameters[index];
    if (!currentParameter) {
      return;
    }

    const nextParameter = { ...currentParameter, ...patch };
    const nextMappings =
      patch.name === undefined
        ? mapping.mappings
        : mapping.mappings.map((entry) => {
            if (entry.interface_param !== currentParameter.name) {
              return entry;
            }

            return {
              ...entry,
              interface_param: nextParameter.name,
              mcp_param:
                entry.mcp_param === currentParameter.name
                  ? nextParameter.name
                  : entry.mcp_param
            };
          });

    emit({
      interface_parameters: mapping.interface_parameters.map(
        (entry, entryIndex) => (entryIndex === index ? nextParameter : entry)
      ),
      mappings: nextMappings
    });
  }

  function removeInterfaceParameter(index: number) {
    const parameter = mapping.interface_parameters[index];
    if (!parameter) {
      return;
    }

    emit({
      interface_parameters: mapping.interface_parameters.filter(
        (_, entryIndex) => entryIndex !== index
      ),
      mappings: mapping.mappings.filter(
        (entry) => entry.interface_param !== parameter.name
      )
    });
  }

  function addMapping(interfaceParam: string | undefined) {
    if (interfaceParam === DES_ID_PARAMETER_NAME) {
      addDesIdMapping();
      setPendingInterfaceParam(undefined);
      return;
    }

    const parameter = mapping.interface_parameters.find(
      (entry) => entry.name === interfaceParam
    );
    if (!parameter) {
      return;
    }

    emit({
      ...mapping,
      mappings: [...mapping.mappings, mappingFromInterfaceParameter(parameter)]
    });
    setPendingInterfaceParam(undefined);
  }

  function addDesIdMapping() {
    if (mappedParameters.has(DES_ID_PARAMETER_NAME)) {
      return;
    }

    const existingParameter = mapping.interface_parameters.find(
      (entry) => entry.name === DES_ID_PARAMETER_NAME
    );
    const parameter = existingParameter ?? desIdInterfaceParameter();

    emit({
      interface_parameters: existingParameter
        ? mapping.interface_parameters
        : [...mapping.interface_parameters, parameter],
      mappings: [...mapping.mappings, mappingFromInterfaceParameter(parameter)]
    });
  }

  function removeMapping(index: number) {
    emit({
      ...mapping,
      mappings: mapping.mappings.filter((_, entryIndex) => entryIndex !== index)
    });
  }

  function updateJsonText(nextText: string) {
    try {
      const parsed = JSON.parse(nextText) as unknown;
      const nextMapping = normalizeInputMapping(parsed);
      setJsonDraft({
        resetKey,
        serializedMapping: stringifyMapping(nextMapping),
        text: nextText,
        error: ''
      });
      onValidityChange?.(true);
      onChange(nextMapping);
    } catch {
      setJsonDraft({
        ...jsonDraft,
        text: nextText,
        error: i18nText('settings', 'auto.enter_valid_json')
      });
      onValidityChange?.(false);
    }
  }

  const mappedParameters = new Set(
    mapping.mappings.map((entry) => entry.interface_param)
  );
  const addableOptions: Array<{ label: string; value: string }> = [];
  const addableOptionNames = new Set<string>();
  for (const entry of mapping.interface_parameters) {
    if (
      entry.name &&
      !mappedParameters.has(entry.name) &&
      !addableOptionNames.has(entry.name)
    ) {
      addableOptions.push({ label: entry.name, value: entry.name });
      addableOptionNames.add(entry.name);
    }
  }
  if (
    !mappedParameters.has(DES_ID_PARAMETER_NAME) &&
    !addableOptionNames.has(DES_ID_PARAMETER_NAME)
  ) {
    addableOptions.push({
      label: DES_ID_PARAMETER_NAME,
      value: DES_ID_PARAMETER_NAME
    });
    addableOptionNames.add(DES_ID_PARAMETER_NAME);
  }

  return (
    <div className="mcp-input-mapping-editor">
      <Tabs
        items={[
          {
            key: 'interface',
            label: i18nText('settings', 'auto.mcp_input_interface_layer'),
            children: (
              <InputMappingInterfaceSection
                mapping={mapping}
                onAddInterfaceParameter={addInterfaceParameter}
                onUpdateInterfaceParameter={updateInterfaceParameter}
                onRemoveInterfaceParameter={removeInterfaceParameter}
              />
            )
          },
          {
            key: 'mapping',
            label: i18nText('settings', 'auto.mcp_input_mapping_layer'),
            children: (
              <InputMappingLayerSection
                mapping={mapping}
                addableOptions={addableOptions}
                pendingInterfaceParam={pendingInterfaceParam}
                onPendingInterfaceParamChange={setPendingInterfaceParam}
                onAddMapping={addMapping}
                onUpdateMapping={updateMapping}
                onRemoveMapping={removeMapping}
              />
            )
          },
          {
            key: 'json',
            label: i18nText('settings', 'auto.json_parse'),
            children: (
              <InputMappingJsonSection
                jsonDraft={jsonDraft}
                onUpdateJsonText={updateJsonText}
              />
            )
          }
        ]}
      />
    </div>
  );
}
