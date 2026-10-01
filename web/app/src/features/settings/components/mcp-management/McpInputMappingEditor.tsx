import DeleteOutlined from '@ant-design/icons/es/icons/DeleteOutlined';
import PlusOutlined from '@ant-design/icons/es/icons/PlusOutlined';
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
import { useMemo, useState, type CSSProperties } from 'react';

import { i18nText } from '../../../../shared/i18n/text';
import { InlineJsonCodeEditor } from '../../../agent-flow/components/detail/fields/json-schema/JsonSchemaSettingsPanel';
import {
  type McpInputInterfaceParameter,
  type McpInputMappingValue,
  type McpInputParameterMapping,
  normalizeInputMapping
} from './mcp-input-mapping-model';

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
    required: parameter.required,
    ...(parameter.source?.kind === 'mcp_call'
      ? { source: { kind: 'mcp_call' as const, path: parameter.name } }
      : {})
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

type ParameterDisplayPath = {
  parentPath: string[];
  leafName: string;
};

type NestedParameterGroupRow = {
  kind: 'group';
  key: string;
  path: string;
  label: string;
  depth: number;
};

type NestedParameterFieldRow<T> = {
  kind: 'field';
  key: string;
  item: T;
  index: number;
};

type NestedParameterRow<T> =
  | NestedParameterGroupRow
  | NestedParameterFieldRow<T>;

function parameterDisplayPath(name: string): ParameterDisplayPath {
  const parts = name.split('.').filter(Boolean);

  if (parts.length <= 1) {
    return {
      parentPath: [],
      leafName: name
    };
  }

  return {
    parentPath: parts.slice(0, -1),
    leafName: parts[parts.length - 1]
  };
}

function composeParameterName(parentPath: string[], leafName: string) {
  return parentPath.length > 0 ? [...parentPath, leafName].join('.') : leafName;
}

function parameterOptionLabel(name: string) {
  const displayPath = parameterDisplayPath(name);

  return displayPath.parentPath.length > 0
    ? [...displayPath.parentPath, displayPath.leafName].join(' / ')
    : displayPath.leafName;
}

function nestedParameterRows<T>(
  items: T[],
  nameOf: (item: T) => string
): Array<NestedParameterRow<T>> {
  let previousParentPath: string[] = [];
  const rows: Array<NestedParameterRow<T>> = [];

  items.forEach((item, index) => {
    const name = nameOf(item);
    const displayPath = parameterDisplayPath(name);
    let sharedDepth = 0;

    while (
      sharedDepth < displayPath.parentPath.length &&
      previousParentPath[sharedDepth] === displayPath.parentPath[sharedDepth]
    ) {
      sharedDepth += 1;
    }

    for (
      let depth = sharedDepth;
      depth < displayPath.parentPath.length;
      depth += 1
    ) {
      const path = displayPath.parentPath.slice(0, depth + 1).join('.');
      rows.push({
        kind: 'group',
        key: `group:${index}:${path}`,
        path,
        label: displayPath.parentPath[depth],
        depth
      });
    }

    rows.push({
      kind: 'field',
      key: `field:${index}:${name}`,
      item,
      index
    });
    previousParentPath = displayPath.parentPath;
  });

  return rows;
}

function parameterDepthStyle(depth: number): CSSProperties {
  return { '--mcp-field-depth': depth } as CSSProperties;
}

function ParameterGroupRow({ row }: { row: NestedParameterGroupRow }) {
  return (
    <div
      aria-label={`field_group ${row.path}`}
      className="mcp-input-mapping-editor__field-group"
      style={parameterDepthStyle(row.depth)}
    >
      <span>{row.label}</span>
    </div>
  );
}

function ParameterNameCell({
  name,
  ariaLabel,
  readOnly,
  onChange
}: {
  name: string;
  ariaLabel: string;
  readOnly?: boolean;
  onChange?: (name: string) => void;
}) {
  const displayPath = parameterDisplayPath(name);

  return (
    <div
      className="mcp-input-mapping-editor__field-name"
      style={parameterDepthStyle(displayPath.parentPath.length)}
    >
      {displayPath.parentPath.length > 0 ? (
        <Typography.Text className="mcp-input-mapping-editor__field-prefix">
          {displayPath.parentPath.join(' / ')}
        </Typography.Text>
      ) : null}
      <Input
        aria-label={ariaLabel}
        readOnly={readOnly}
        value={displayPath.leafName}
        onChange={(event) =>
          onChange?.(
            composeParameterName(displayPath.parentPath, event.target.value)
          )
        }
      />
    </div>
  );
}

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
  showCallParameters,
  onAddInterfaceParameter,
  onUpdateInterfaceParameter,
  onRemoveInterfaceParameter
}: {
  mapping: McpInputMappingValue;
  showCallParameters: boolean;
  onAddInterfaceParameter: () => void;
  onUpdateInterfaceParameter: (
    index: number,
    patch: Partial<McpInputInterfaceParameter>
  ) => void;
  onRemoveInterfaceParameter: (index: number) => void;
}) {
  const rows = nestedParameterRows(
    mapping.interface_parameters,
    (parameter) => parameter.name
  );

  return (
    <Space
      className="mcp-input-mapping-editor__stack"
      orientation="vertical"
      size="middle"
    >
      <Flex justify="flex-end">
        <Button icon={<PlusOutlined />} onClick={onAddInterfaceParameter}>
          {i18nText('settings', 'auto.add_new_field')}
        </Button>
      </Flex>
      {showCallParameters || mapping.interface_parameters.length > 0 ? (
        <div className="mcp-input-mapping-editor__table">
          <div className="mcp-input-mapping-editor__head">
            <span>{i18nText('settings', 'auto.field_name')}</span>
            <span>{i18nText('settings', 'auto.field_type')}</span>
            <span>{i18nText('settings', 'auto.parameter_type')}</span>
            <span>{i18nText('settings', 'auto.required')}</span>
            <span />
          </div>
          {rows.map((row) => {
            if (row.kind === 'group') {
              return <ParameterGroupRow key={row.key} row={row} />;
            }

            const parameter = row.item;
            const index = row.index;
            const isCallParameter = parameter.source?.kind === 'mcp_call';
            if (isCallParameter && !showCallParameters) {
              return null;
            }

            return (
              <div
                aria-label={
                  isCallParameter
                    ? `call_parameter ${parameter.name}`
                    : undefined
                }
                className="mcp-input-mapping-editor__row"
                key={row.key}
              >
                <ParameterNameCell
                  ariaLabel={`field_name ${index + 1}`}
                  name={parameter.name}
                  readOnly={isCallParameter}
                  onChange={(name) =>
                    onUpdateInterfaceParameter(index, {
                      name
                    })
                  }
                />
                <Input
                  aria-label={`field_type ${parameter.name || index + 1}`}
                  value={parameter.field_type}
                  readOnly={isCallParameter}
                  onChange={(event) =>
                    onUpdateInterfaceParameter(index, {
                      field_type: event.target.value
                    })
                  }
                />
                {isCallParameter ? (
                  <Input
                    aria-label={`parameter_type ${parameter.name}`}
                    readOnly
                    value={i18nText('settings', 'auto.json_request_body')}
                  />
                ) : (
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
                )}
                <Checkbox
                  aria-label={`required ${parameter.name || index + 1}`}
                  checked={parameter.required}
                  disabled={isCallParameter}
                  onChange={(event) =>
                    onUpdateInterfaceParameter(index, {
                      required: event.target.checked
                    })
                  }
                />
                {isCallParameter ? (
                  <span />
                ) : (
                  <Button
                    aria-label={`delete_field ${parameter.name || index + 1}`}
                    icon={<DeleteOutlined />}
                    onClick={() => onRemoveInterfaceParameter(index)}
                  />
                )}
              </div>
            );
          })}
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
  onAddAllMappings,
  onUpdateMapping,
  onRemoveMapping
}: {
  mapping: McpInputMappingValue;
  addableOptions: Array<{ label: string; value: string }>;
  pendingInterfaceParam: string | undefined;
  onPendingInterfaceParamChange: (value: string | undefined) => void;
  onAddMapping: (interfaceParam: string | undefined) => void;
  onAddAllMappings: () => void;
  onUpdateMapping: (
    index: number,
    patch: Partial<McpInputParameterMapping>
  ) => void;
  onRemoveMapping: (index: number) => void;
}) {
  const rows = nestedParameterRows(
    mapping.mappings,
    (entry) => entry.interface_param
  );

  return (
    <Space
      className="mcp-input-mapping-editor__stack"
      orientation="vertical"
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
          aria-label="添加"
          autoInsertSpace={false}
          disabled={!pendingInterfaceParam}
          onClick={() => onAddMapping(pendingInterfaceParam)}
        >
          添加
        </Button>
        <Button
          aria-label="全部"
          autoInsertSpace={false}
          disabled={addableOptions.length === 0}
          onClick={onAddAllMappings}
        >
          <span>全部</span>
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
          {rows.map((row) => {
            if (row.kind === 'group') {
              return <ParameterGroupRow key={row.key} row={row} />;
            }

            const entry = row.item;
            const index = row.index;

            return (
              <div
                className="mcp-input-mapping-editor__mapping-row"
                key={row.key}
              >
                <ParameterNameCell
                  ariaLabel={`interface_param ${entry.interface_param}`}
                  name={entry.interface_param}
                  readOnly
                />
                <Input
                  aria-label={`mcp_param ${entry.interface_param}`}
                  value={entry.mcp_param}
                  readOnly={entry.source?.kind === 'mcp_call'}
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
                  disabled={entry.source?.kind === 'mcp_call'}
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
            );
          })}
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
    <Space orientation="vertical" style={{ width: '100%' }}>
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
  showCallParameters = false,
  onChange,
  onValidityChange
}: {
  value: unknown;
  resetKey?: string | number | null;
  showCallParameters?: boolean;
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
      ...mapping,
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
      ...mapping,
      interface_parameters: mapping.interface_parameters.filter(
        (_, entryIndex) => entryIndex !== index
      ),
      mappings: mapping.mappings.filter(
        (entry) => entry.interface_param !== parameter.name
      )
    });
  }

  function addMapping(interfaceParam: string | undefined) {
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

  function addAllMappings() {
    const nextMappedParameters = new Set(mappedParameters);
    const nextMappings = [...mapping.mappings];

    for (const parameter of mapping.interface_parameters) {
      if (parameter.source?.kind === 'mcp_call' && !showCallParameters) {
        continue;
      }
      if (!parameter.name || nextMappedParameters.has(parameter.name)) {
        continue;
      }

      nextMappings.push(mappingFromInterfaceParameter(parameter));
      nextMappedParameters.add(parameter.name);
    }

    emit({
      ...mapping,
      interface_parameters: mapping.interface_parameters,
      mappings: nextMappings
    });
    setPendingInterfaceParam(undefined);
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
  const addableOptions = mapping.interface_parameters
    .filter(
      (entry) =>
        entry.name &&
        !mappedParameters.has(entry.name) &&
        (showCallParameters || entry.source?.kind !== 'mcp_call')
    )
    .map((entry) => ({
      label: parameterOptionLabel(entry.name),
      value: entry.name
    }));

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
                showCallParameters={showCallParameters}
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
                onAddAllMappings={addAllMappings}
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
